use crate::models::TickerInfo;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};
use url::Url;

const BYBIT_WS_URL: &str = "wss://stream.bybit.com/v5/public/spot";
const PING_INTERVAL: u64 = 20;

#[derive(Debug, Deserialize)]
struct WsResponse {
    topic: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    msg_type: Option<String>,
    data: Option<serde_json::Value>, // Change to Value to handle both single object and array
    success: Option<bool>,
    ret_msg: Option<String>,
    #[allow(dead_code)]
    op: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrderbookData {
    s: String,
    #[serde(default)]
    b: Vec<Vec<String>>,
    #[serde(default)]
    a: Vec<Vec<String>>,
}

pub struct BybitWebsocket {
    id: usize,
    symbols: Vec<String>,
    sender: mpsc::Sender<TickerInfo>,
}

impl BybitWebsocket {
    pub fn new(id: usize, symbols: Vec<String>, sender: mpsc::Sender<TickerInfo>) -> Self {
        Self {
            id,
            symbols,
            sender,
        }
    }

    pub async fn run(self) {
        let url = Url::parse(BYBIT_WS_URL).expect("Invalid WebSocket URL");

        loop {
            info!("[Conn #{}] Connecting to Bybit WebSocket...", self.id);
            match connect_async(url.to_string()).await {
                Ok((ws_stream, _)) => {
                    info!("[Conn #{}] Connected to Bybit WebSocket", self.id);
                    let (mut write, mut read) = ws_stream.split();

                    // Subscribe to orderbook (depth 1) for best bid/ask
                    // Bybit allows max 10 args per request. We need to chunk subscriptions.
                    let mut subscribed_count = 0;
                    for chunk in self.symbols.chunks(10) {
                        let args: Vec<String> =
                            chunk.iter().map(|s| format!("orderbook.1.{s}")).collect();
                        let subscribe_msg = serde_json::json!({
                            "op": "subscribe",
                            "args": args
                        });

                        if let Err(e) = write
                            .send(Message::Text(subscribe_msg.to_string().into()))
                            .await
                        {
                            error!("Failed to send subscription: {e}");
                            break;
                        }
                        subscribed_count += chunk.len();
                    }
                    info!(
                        "[Conn #{}] Subscribed to {} symbols (Orderbook)",
                        self.id, subscribed_count
                    );

                    // Heartbeat task
                    let mut ping_interval =
                        tokio::time::interval(Duration::from_secs(PING_INTERVAL));

                    loop {
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                let ping_msg = serde_json::json!({ "op": "ping" });
                                if let Err(e) = write.send(Message::Text(ping_msg.to_string().into())).await {
                                    error!("Failed to send ping: {e}");
                                    break;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        match serde_json::from_str::<WsResponse>(&text) {
                                            Ok(response) => {
                                                if let Some(data_val) = response.data {
                                                    // Check topic to decide how to parse
                                                    if let Some(topic) = &response.topic {
                                                        if topic.starts_with("orderbook.1") {
                                                            match serde_json::from_value::<OrderbookData>(data_val.clone()) {
                                                                Ok(ob) => {
                                                                    // Convert to TickerInfo using serde_json::json!
                                                                    // We only care about symbol, bid1, ask1
                                                                    let ticker_json = serde_json::json!({
                                                                        "symbol": ob.s,
                                                                        "bid1Price": ob.b.first().map(|v| v[0].clone()),
                                                                        "bid1Size": ob.b.first().map(|v| v[1].clone()),
                                                                        "ask1Price": ob.a.first().map(|v| v[0].clone()),
                                                                        "ask1Size": ob.a.first().map(|v| v[1].clone())
                                                                    });

                                                                    match serde_json::from_value::<TickerInfo>(ticker_json) {
                                                                        Ok(ticker) => {
                                                                            if let Err(e) = self.sender.send(ticker).await {
                                                                                error!("Failed to send ticker update: {e}");
                                                                                break;
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            warn!("Failed to convert orderbook to ticker: {e}");
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    warn!("Failed to deserialize orderbook data: {e}");
                                                                }
                                                            }
                                                        } else {
                                                            // Fallback for tickers topic if we ever use it
                                                            match serde_json::from_value::<TickerInfo>(data_val.clone()) {
                                                                Ok(ticker) => {
                                                                    if let Err(e) = self.sender.send(ticker).await {
                                                                        error!("Failed to send ticker update: {e}");
                                                                        break;
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    warn!("Failed to deserialize ticker data: {e}. Data: {:?}", data_val);
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else if let Some(success) = response.success {
                                                    if !success {
                                                        warn!("WebSocket operation failed: {:?}", response.ret_msg);
                                                    } else {
                                                        // debug!("WebSocket operation successful: {:?}", response.ret_msg);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                // Only log error if it's not a simple pong or success message we failed to parse fully
                                                if !text.contains("pong") && !text.contains("subscribe") {
                                                    warn!("Failed to parse WS message: {e} | Text: {text}");
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        warn!("WebSocket connection closed");
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        error!("WebSocket error: {e}");
                                        break;
                                    }
                                    None => {
                                        warn!("WebSocket stream ended");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect to WebSocket: {e}");
                }
            }

            warn!("Reconnecting in 5 seconds...");
            sleep(Duration::from_secs(5)).await;
        }
    }
}
