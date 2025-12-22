use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
    pub testnet: bool,
    pub request_timeout_secs: u64,
    pub max_retries: u32,
    pub order_size: f64,
    pub min_profit_threshold: f64,
    pub trading_fee_rate: f64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok(); // Load .env file if present

        let api_key = env::var("BYBIT_API_KEY")
            .context("BYBIT_API_KEY environment variable is required")?;
        
        let api_secret = env::var("BYBIT_API_SECRET")
            .context("BYBIT_API_SECRET environment variable is required")?;

        let testnet = env::var("BYBIT_TESTNET")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let base_url = if testnet {
            "https://api-testnet.bybit.com".to_string()
        } else {
            "https://api.bybit.com".to_string()
        };

        let request_timeout_secs = env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .unwrap_or(30);

        let max_retries = env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u32>()
            .unwrap_or(3);

        let order_size = env::var("ORDER_SIZE")
            .unwrap_or_else(|_| "4.0".to_string())
            .parse::<f64>()
            .unwrap_or(4.0);

        let min_profit_threshold = env::var("MIN_PROFIT_THRESHOLD")
            .unwrap_or_else(|_| "0.05".to_string())
            .parse::<f64>()
            .unwrap_or(0.05);

        let trading_fee_rate = env::var("TRADING_FEE_RATE")
            .unwrap_or_else(|_| "0.0015".to_string())
            .parse::<f64>()
            .unwrap_or(0.0015);

        Ok(Config {
            api_key,
            api_secret,
            base_url,
            testnet,
            request_timeout_secs,
            max_retries,
            order_size,
            min_profit_threshold,
            trading_fee_rate,
        })
    }

    /// Get the wallet balance endpoint
    pub fn wallet_balance_endpoint(&self) -> String {
        format!("{}/v5/account/wallet-balance", self.base_url)
    }

    /// Get the instruments info endpoint
    pub fn instruments_info_endpoint(&self) -> String {
        format!("{}/v5/market/instruments-info", self.base_url)
    }

    /// Get the ticker endpoint for 24hr price data
    pub fn tickers_endpoint(&self) -> String {
        format!("{}/v5/market/tickers", self.base_url)
    }
}

// Constants for arbitrage calculations
pub const MIN_PROFIT_THRESHOLD: f64 = 0.05; // Show any profit above 0.05%
pub const MAX_TRIANGLES_TO_SCAN: usize = 2000; // Maximum triangles to process
pub const BALANCE_REFRESH_INTERVAL_SECS: u64 = 60; // 1 minute
pub const PRICE_REFRESH_INTERVAL_SECS: u64 = 2; // 2 seconds
pub const CYCLE_SUMMARY_INTERVAL: usize = 100; // Log summary every 100 cycles

// Realistic trading filters
pub const MIN_VOLUME_24H_USD: f64 = 10000.0; // Minimum 24h volume in USD for liquidity (increased for safety)
pub const MIN_BID_SIZE_USD: f64 = 100.0; // Minimum bid size in USD (lowered)
pub const MIN_ASK_SIZE_USD: f64 = 100.0; // Minimum ask size in USD (lowered)
pub const MAX_SPREAD_PERCENT: f64 = 1.0; // Maximum bid/ask spread percentage (decreased for tighter spreads)
pub const MAX_SLIPPAGE_PERCENT: f64 = 0.5; // Maximum acceptable slippage per trade
pub const VWAP_DEPTH_LEVELS: usize = 5; // Number of order book levels for VWAP calculation
pub const MIN_TRADE_AMOUNT_USD: f64 = 10.0; // Minimum trade amount for realistic execution

// Blacklisted tokens that should be excluded from arbitrage (geographical restrictions, etc.)
pub const BLACKLISTED_TOKENS: &[&str] = &[
    "USDR",    // USD Reserve - restricted in Netherlands and other regions
    "BUSD",    // Binance USD - being phased out
    "UST",     // TerraUSD - collapsed stablecoin
    "LUNA",    // Terra Luna - collapsed
    "FTT",     // FTX Token - exchange collapsed
    "CEL",     // Celsius - platform issues
    "LUNC",    // Terra Luna Classic - collapsed
    "USTC",    // TerraUSD Classic - collapsed
    "TRY",     // Turkish Lira - geographical restrictions (Error 170348)
    "BRL",     // Brazilian Real - high slippage and low liquidity
    // ⚠️ Newly added delisted / restricted tokens:
    "RDNT",    // Delisted
    "MOVR",    // Delisted
    "HOOK",    // Delisted
    "TST",     // Delisted
    "5IRE",    // Delisted
    "APTR",    // Delisted
    "ERTHA",   // Delisted
    "GUMMY",   // Delisted
    "PIP",     // Delisted
    "WWY",     // Delisted
    "XETA",    // Delisted
    "VRTX",    // Delisted
    "FAR",     // Delisted
    "TAP",     // Delisted
    "KCAL",    // Delisted
    "VPR",     // Delisted
    "SON",     // Delisted
    "COT",     // Delisted
    "MOJO",    // Delisted
    "TENET",   // Delisted
    "SALD",    // Delisted
    "HVH",     // Delisted
    "BRAWL",   // Delisted
    "THN",     // Delisted
    "PI",      // Pi token – rejected due to scam risk
];

/// Check if a token is blacklisted for arbitrage
pub fn is_token_blacklisted(token: &str) -> bool {
    BLACKLISTED_TOKENS.contains(&token.to_uppercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_endpoints() {
        let config = Config {
            api_key: "test_key".to_string(),
            api_secret: "test_secret".to_string(),
            base_url: "https://api.bybit.com".to_string(),
            testnet: false,
            request_timeout_secs: 30,
            max_retries: 3,
        };

        assert_eq!(
            config.wallet_balance_endpoint(),
            "https://api.bybit.com/v5/account/wallet-balance"
        );
        assert_eq!(
            config.instruments_info_endpoint(),
            "https://api.bybit.com/v5/market/instruments-info"
        );
    }
}
