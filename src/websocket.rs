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
    _topic: Option<String>,
    #[serde(rename = "type")]
    _msg_type: Option<String>,
    data: Option<TickerInfo>,
    success: Option<bool>,
    ret_msg: Option<String>,
    _op: Option<String>,
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

                    // Subscribe to tickers
                    // Bybit allows max 10 args per request. We need to chunk subscriptions.
                    let mut subscribed_count = 0;
                    for chunk in self.symbols.chunks(10) {
                        let args: Vec<String> =
                            chunk.iter().map(|s| format!("tickers.{}", s)).collect();
                        let subscribe_msg = serde_json::json!({
                            "op": "subscribe",
                            "args": args
                        });

                        if let Err(e) = write
                            .send(Message::Text(subscribe_msg.to_string().into()))
                            .await
                        {
                            error!("Failed to send subscription: {}", e);
                            break;
                        }
                        subscribed_count += chunk.len();
                    }
                    info!(
                        "[Conn #{}] Subscribed to {} symbols",
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
                                    error!("Failed to send ping: {}", e);
                                    break;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        match serde_json::from_str::<WsResponse>(&text) {
                                            Ok(response) => {
                                                if let Some(data) = response.data {
                                                    if let Err(e) = self.sender.send(data).await {
                                                        error!("Failed to send ticker update: {}", e);
                                                        break;
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
                                                    warn!("Failed to parse WS message: {} | Text: {}", e, text);
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        warn!("WebSocket connection closed");
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        error!("WebSocket error: {}", e);
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
                    error!("Failed to connect to WebSocket: {}", e);
                }
            }

            warn!("Reconnecting in 5 seconds...");
            sleep(Duration::from_secs(5)).await;
        }
    }
}
