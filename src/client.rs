use crate::config::Config;
use crate::models::*;
use anyhow::{Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct BybitClient {
    client: Client,
    config: Config,
}

impl BybitClient {
    pub fn new(config: Config) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("X-BAPI-API-KEY", HeaderValue::from_str(&config.api_key)?);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
            .tcp_nodelay(true)
            .tcp_keepalive(std::time::Duration::from_secs(60)) // Keep connections alive
            .pool_idle_timeout(None) // Never close idle connections automatically
            .pool_max_idle_per_host(10) // Keep up to 10 connections open per host
            .default_headers(headers)
            .build()?;

        Ok(BybitClient { client, config })
    }

    /// Generate HMAC SHA256 signature for Bybit API
    fn generate_signature(
        &self,
        timestamp: u64,
        method: &str,
        _path: &str,
        query_params: &str,
        body: &str,
    ) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let recv_window = "5000";

        // For POST requests with body, include the body in the signature
        let param_str = if method == "POST" && !body.is_empty() {
            format!(
                "{}{}{}{}",
                timestamp, &self.config.api_key, recv_window, body
            )
        } else if !query_params.is_empty() {
            format!(
                "{}{}{}{}",
                timestamp, &self.config.api_key, recv_window, query_params
            )
        } else {
            format!("{}{}{}", timestamp, &self.config.api_key, recv_window)
        };

        let mut mac = HmacSha256::new_from_slice(self.config.api_secret.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to create HMAC: {}", e))?;

        mac.update(param_str.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// Get current timestamp in milliseconds
    fn get_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Execute a signed GET request to Bybit API
    async fn signed_request<T>(&self, endpoint: &str, query_params: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let timestamp = Self::get_timestamp_ms();
        let signature = self.generate_signature(timestamp, "GET", endpoint, query_params, "")?;

        let mut url = endpoint.to_string();
        if !query_params.is_empty() {
            url.push('?');
            url.push_str(query_params);
        }

        debug!("Making signed request to: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-BAPI-SIGN", signature)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-RECV-WINDOW", "5000")
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();

        if !status.is_success() {
            let response_text = response.text().await.unwrap_or_default();
            error!("HTTP error {}: {}", status, response_text);
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, response_text));
        }

        // Optimization: Use simd-json for faster parsing and avoid double-parsing
        // We need a mutable buffer for simd-json
        let bytes = response
            .bytes()
            .await
            .context("Failed to get response bytes")?;
        let mut buffer = bytes.to_vec();

        let api_response: ApiResponse<T> =
            simd_json::from_slice(&mut buffer).context("Failed to parse API response structure")?;

        api_response
            .into_result()
            .map_err(|e| anyhow::anyhow!("API error: {}", e))
    }

    /// Execute an unsigned GET request (for public endpoints)
    async fn public_request<T>(&self, endpoint: &str, query_params: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut url = endpoint.to_string();
        if !query_params.is_empty() {
            url.push('?');
            url.push_str(query_params);
        }

        debug!("Making public request to: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();

        if !status.is_success() {
            let response_text = response.text().await.unwrap_or_default();
            error!("HTTP error {}: {}", status, response_text);
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, response_text));
        }

        // Optimization: Use simd-json
        let bytes = response
            .bytes()
            .await
            .context("Failed to get response bytes")?;
        let mut buffer = bytes.to_vec();

        let api_response: ApiResponse<T> =
            simd_json::from_slice(&mut buffer).context("Failed to parse API response structure")?;

        api_response
            .into_result()
            .map_err(|e| anyhow::anyhow!("API error: {}", e))
    }

    /// Fetch account wallet balance
    pub async fn get_wallet_balance(
        &self,
        account_type: Option<&str>,
    ) -> Result<WalletBalanceResult> {
        let acc_type = account_type.unwrap_or("UNIFIED");
        debug!("Fetching wallet balance for account type: {}", acc_type);

        let query_params = format!("accountType={acc_type}");

        let result = self
            .signed_request::<WalletBalanceResult>(
                &self.config.wallet_balance_endpoint(),
                &query_params,
            )
            .await?;

        debug!(
            "Successfully fetched wallet balance for {} accounts (type: {})",
            result.list.len(),
            acc_type
        );
        Ok(result)
    }

    /// Fetch trading instruments info
    pub async fn get_instruments_info(
        &self,
        category: &str,
        limit: Option<u32>,
    ) -> Result<InstrumentsInfoResult> {
        debug!("Fetching instruments info for category: {}", category);

        let mut query_params = format!("category={category}");
        if let Some(lmt) = limit {
            query_params.push_str(&format!("&limit={lmt}"));
        }

        let result = self
            .public_request::<InstrumentsInfoResult>(
                &self.config.instruments_info_endpoint(),
                &query_params,
            )
            .await?;

        debug!(
            "Successfully fetched {} instruments for category {}",
            result.list.len(),
            category
        );
        Ok(result)
    }

    /// Fetch all spot instruments with pagination
    pub async fn get_all_spot_instruments(&self) -> Result<Vec<InstrumentInfo>> {
        debug!("Fetching all spot instruments...");

        let mut all_instruments = Vec::new();
        let mut cursor: Option<String> = None;
        let mut page = 1;

        loop {
            let mut query_params = "category=spot&limit=1000".to_string();
            if let Some(ref c) = cursor {
                query_params.push_str(&format!("&cursor={c}"));
            }

            debug!("Fetching page {} of instruments", page);

            let result = self
                .public_request::<InstrumentsInfoResult>(
                    &self.config.instruments_info_endpoint(),
                    &query_params,
                )
                .await?;

            let instruments_count = result.list.len();
            all_instruments.extend(result.list);

            debug!("Fetched {} instruments on page {}", instruments_count, page);

            if result.next_page_cursor.is_none() || instruments_count == 0 {
                break;
            }

            cursor = result.next_page_cursor;
            page += 1;
        }

        debug!(
            "Successfully fetched {} total spot instruments across {} pages",
            all_instruments.len(),
            page
        );

        Ok(all_instruments)
    }

    /// Fetch ticker prices for all symbols
    pub async fn get_tickers(&self, category: &str) -> Result<TickersResult> {
        debug!("Fetching tickers for category: {}", category);

        let query_params = format!("category={category}");

        let result = self
            .public_request::<TickersResult>(&self.config.tickers_endpoint(), &query_params)
            .await?;

        debug!(
            "Successfully fetched {} tickers for category {}",
            result.list.len(),
            category
        );
        Ok(result)
    }

    /// Get ticker for a specific symbol
    pub async fn get_ticker(&self, category: &str, symbol: &str) -> Result<TickersResult> {
        debug!("Fetching ticker for symbol: {}", symbol);

        let query_params = format!("category={category}&symbol={symbol}");

        let result = self
            .public_request::<TickersResult>(&self.config.tickers_endpoint(), &query_params)
            .await?;

        Ok(result)
    }

    /// Place a new order
    pub async fn place_order(
        &self,
        order_request: crate::models::PlaceOrderRequest,
    ) -> Result<crate::models::PlaceOrderResult> {
        // info!("Placing {} order: {} {} @ {:?}",
        //       order_request.side, order_request.qty, order_request.symbol, order_request.price);

        let endpoint = format!("{}/v5/order/create", self.config.base_url);
        let body = serde_json::to_string(&order_request)?;
        let timestamp = Self::get_timestamp_ms();

        let client = reqwest::Client::new();
        let signature =
            self.generate_signature(timestamp, "POST", "/v5/order/create", "", &body)?;

        let response = client
            .post(&endpoint)
            .header("X-BAPI-API-KEY", &self.config.api_key)
            .header("X-BAPI-SIGN", signature)
            .header("X-BAPI-SIGN-TYPE", "2")
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-RECV-WINDOW", "5000")
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await?;

        let response_text = response.text().await?;
        debug!("Place order response: {}", response_text);

        // First parse as a generic API response to check for errors
        let api_response: crate::models::ApiResponse<serde_json::Value> =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse API response: {}. Response was: {}",
                    e,
                    response_text
                )
            })?;

        if !api_response.is_success() {
            error!("Order placement failed. Request: {}", body);
            error!(
                "API Error {}: {}",
                api_response.ret_code, api_response.ret_msg
            );
            return Err(anyhow::anyhow!(
                "Order placement failed - API Error {}: {}",
                api_response.ret_code,
                api_response.ret_msg
            ));
        }

        // Now parse the successful response as PlaceOrderResult
        let typed_response: crate::models::ApiResponse<crate::models::PlaceOrderResult> =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse successful order response: {}. Response was: {}",
                    e,
                    response_text
                )
            })?;

        let result = typed_response
            .into_result()
            .map_err(|e| anyhow::anyhow!("Failed to parse order result: {}", e))?;

        info!("Order placed successfully: {}", result.order_id);
        Ok(result)
    }

    /// Get order information
    pub async fn get_order(
        &self,
        category: &str,
        order_id: &str,
        symbol: &str,
    ) -> Result<crate::models::OrderInfo> {
        debug!("Getting order info: {}", order_id);

        let query_params = format!("category={category}&orderId={order_id}&symbol={symbol}");

        let endpoint = format!("{}/v5/order/realtime", self.config.base_url);

        // Get the raw response to debug the structure
        let response = self
            .signed_request::<serde_json::Value>(&endpoint, &query_params)
            .await?;

        debug!(
            "Raw order status response: {}",
            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        // Try to parse as the expected OrderListResult structure
        match serde_json::from_value::<crate::models::OrderListResult>(response.clone()) {
            Ok(parsed) => parsed
                .list
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("Order not found in response")),
            Err(parse_err) => {
                error!("Failed to parse order response: {}", parse_err);
                error!(
                    "Raw response was: {}",
                    serde_json::to_string(&response)
                        .unwrap_or_else(|_| "Failed to serialize".to_string())
                );
                Err(anyhow::anyhow!(
                    "Failed to parse order response: {}",
                    parse_err
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Config {
        Config {
            api_key: "test_key".to_string(),
            api_secret: "test_secret".to_string(),
            base_url: "https://api-testnet.bybit.com".to_string(),
            testnet: true,
            request_timeout_secs: 30,
            max_retries: 3,
            order_size: 100.0,
            min_profit_threshold: 0.5,
            trading_fee_rate: 0.001,
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = create_test_config();
        let client = BybitClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_timestamp_generation() {
        let ts1 = BybitClient::get_timestamp_ms();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = BybitClient::get_timestamp_ms();
        assert!(ts2 > ts1);
    }
}
