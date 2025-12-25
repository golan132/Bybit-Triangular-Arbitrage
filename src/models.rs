use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(rename = "retCode")]
    pub ret_code: i32,
    #[serde(rename = "retMsg")]
    pub ret_msg: String,
    pub result: Option<T>,
    #[serde(rename = "retExtInfo")]
    pub ret_ext_info: Option<serde_json::Value>,
    pub time: Option<i64>,
}

impl<T> ApiResponse<T> {
    pub fn is_success(&self) -> bool {
        self.ret_code == 0
    }

    pub fn into_result(self) -> Result<T, String> {
        if self.is_success() {
            self.result.ok_or_else(|| "No result data".to_string())
        } else {
            Err(format!("API Error {}: {}", self.ret_code, self.ret_msg))
        }
    }
}

// Wallet Balance Models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalanceResult {
    #[serde(default)]
    pub list: Vec<WalletAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAccount {
    #[serde(rename = "totalEquity")]
    pub total_equity: Option<String>,
    #[serde(rename = "accountIMRate")]
    pub account_im_rate: Option<String>,
    #[serde(rename = "totalMarginBalance")]
    pub total_margin_balance: Option<String>,
    #[serde(rename = "totalInitialMargin")]
    pub total_initial_margin: Option<String>,
    #[serde(rename = "accountType")]
    pub account_type: Option<String>,
    #[serde(rename = "totalAvailableBalance")]
    pub total_available_balance: Option<String>,
    #[serde(rename = "accountMMRate")]
    pub account_mm_rate: Option<String>,
    #[serde(rename = "totalPerpUPL")]
    pub total_perp_upl: Option<String>,
    #[serde(rename = "totalWalletBalance")]
    pub total_wallet_balance: Option<String>,
    #[serde(rename = "accountLTV")]
    pub account_ltv: Option<String>,
    #[serde(rename = "totalMaintenanceMargin")]
    pub total_maintenance_margin: Option<String>,
    #[serde(default)]
    pub coin: Vec<CoinBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinBalance {
    #[serde(rename = "availableToBorrow")]
    pub available_to_borrow: Option<String>,
    #[serde(rename = "bonus")]
    pub bonus: Option<String>,
    #[serde(rename = "accruedInterest")]
    pub accrued_interest: Option<String>,
    #[serde(rename = "availableToWithdraw")]
    pub available_to_withdraw: Option<String>,
    #[serde(rename = "totalOrderIM")]
    pub total_order_im: Option<String>,
    #[serde(rename = "equity")]
    pub equity: Option<String>,
    #[serde(rename = "totalPositionMM")]
    pub total_position_mm: Option<String>,
    #[serde(rename = "usdValue")]
    pub usd_value: Option<String>,
    #[serde(rename = "unrealisedPnl")]
    pub unrealised_pnl: Option<String>,
    #[serde(rename = "collateralSwitch")]
    pub collateral_switch: Option<bool>,
    #[serde(rename = "spotHedgingQty")]
    pub spot_hedging_qty: Option<String>,
    #[serde(rename = "borrowAmount")]
    pub borrow_amount: Option<String>,
    #[serde(rename = "totalPositionIM")]
    pub total_position_im: Option<String>,
    #[serde(rename = "walletBalance")]
    pub wallet_balance: Option<String>,
    #[serde(rename = "cumRealisedPnl")]
    pub cum_realised_pnl: Option<String>,
    #[serde(rename = "locked")]
    pub locked: Option<String>,
    #[serde(rename = "marginCollateral")]
    pub margin_collateral: Option<bool>,
    pub coin: String,
}

// Instruments Info Models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentsInfoResult {
    pub category: String,
    pub list: Vec<InstrumentInfo>,
    #[serde(rename = "nextPageCursor")]
    pub next_page_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentInfo {
    pub symbol: String,
    #[serde(rename = "contractType")]
    pub contract_type: Option<String>,
    pub status: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    #[serde(rename = "launchTime")]
    pub launch_time: Option<String>,
    #[serde(rename = "deliveryTime")]
    pub delivery_time: Option<String>,
    #[serde(rename = "deliveryFeeRate")]
    pub delivery_fee_rate: Option<String>,
    #[serde(rename = "priceScale")]
    pub price_scale: Option<String>,
    #[serde(rename = "leverageFilter")]
    pub leverage_filter: Option<LeverageFilter>,
    #[serde(rename = "priceFilter")]
    pub price_filter: Option<PriceFilter>,
    #[serde(rename = "lotSizeFilter")]
    pub lot_size_filter: Option<LotSizeFilter>,
    #[serde(rename = "unifiedMarginTrade")]
    pub unified_margin_trade: Option<bool>,
    #[serde(rename = "fundingInterval")]
    pub funding_interval: Option<i32>,
    #[serde(rename = "settleCoin")]
    pub settle_coin: Option<String>,
    #[serde(rename = "copyTrading")]
    pub copy_trading: Option<String>,
    #[serde(rename = "upperFundingRate")]
    pub upper_funding_rate: Option<String>,
    #[serde(rename = "lowerFundingRate")]
    pub lower_funding_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeverageFilter {
    #[serde(rename = "minLeverage")]
    pub min_leverage: Option<String>, // Make optional
    #[serde(rename = "maxLeverage")]
    pub max_leverage: Option<String>, // Make optional
    #[serde(rename = "leverageStep")]
    pub leverage_step: Option<String>, // Make optional
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFilter {
    #[serde(rename = "minPrice")]
    pub min_price: Option<String>, // Make optional
    #[serde(rename = "maxPrice")]
    pub max_price: Option<String>, // Make optional
    #[serde(rename = "tickSize")]
    pub tick_size: Option<String>, // Make optional
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotSizeFilter {
    #[serde(rename = "maxOrderQty")]
    pub max_order_qty: String,
    #[serde(rename = "maxMktOrderQty")]
    pub max_mkt_order_qty: Option<String>,
    #[serde(rename = "minOrderQty")]
    pub min_order_qty: String,
    #[serde(rename = "qtyStep")]
    pub qty_step: Option<String>, // Make this optional as some instruments might not have it
    #[serde(rename = "postOnlyMaxOrderQty")]
    pub post_only_max_order_qty: Option<String>,
    #[serde(rename = "minNotionalValue")]
    pub min_notional_value: Option<String>,
}

// Ticker Models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickersResult {
    pub category: String,
    pub list: Vec<TickerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerInfo {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: Option<String>,
    #[serde(rename = "indexPrice")]
    pub index_price: Option<String>,
    #[serde(rename = "markPrice")]
    pub mark_price: Option<String>,
    #[serde(rename = "prevPrice24h")]
    pub prev_price_24h: Option<String>,
    #[serde(rename = "price24hPcnt")]
    pub price_24h_pcnt: Option<String>,
    #[serde(rename = "highPrice24h")]
    pub high_price_24h: Option<String>,
    #[serde(rename = "lowPrice24h")]
    pub low_price_24h: Option<String>,
    #[serde(rename = "prevPrice1h")]
    pub prev_price_1h: Option<String>,
    #[serde(rename = "openInterest")]
    pub open_interest: Option<String>,
    #[serde(rename = "openInterestValue")]
    pub open_interest_value: Option<String>,
    pub turnover24h: Option<String>,
    pub volume24h: Option<String>,
    #[serde(rename = "fundingRate")]
    pub funding_rate: Option<String>,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: Option<String>,
    #[serde(rename = "predictedDeliveryPrice")]
    pub predicted_delivery_price: Option<String>,
    #[serde(rename = "basisRate")]
    pub basis_rate: Option<String>,
    #[serde(rename = "deliveryFeeRate")]
    pub delivery_fee_rate: Option<String>,
    #[serde(rename = "deliveryTime")]
    pub delivery_time: Option<String>,
    #[serde(rename = "ask1Size")]
    pub ask1_size: Option<String>,
    #[serde(rename = "bid1Price")]
    pub bid1_price: Option<String>,
    #[serde(rename = "ask1Price")]
    pub ask1_price: Option<String>,
    #[serde(rename = "bid1Size")]
    pub bid1_size: Option<String>,
    pub basis: Option<String>,
}

// Order placement models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderRequest {
    pub category: String,
    pub symbol: String,
    pub side: String, // "Buy" or "Sell"
    #[serde(rename = "orderType")]
    pub order_type: String, // "Market" or "Limit"
    pub qty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(rename = "timeInForce", skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<String>, // "GTC", "IOC", "FOK"
    #[serde(rename = "orderLinkId", skip_serializing_if = "Option::is_none")]
    pub order_link_id: Option<String>,
    #[serde(rename = "reduceOnly", skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderResult {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "orderLinkId")]
    pub order_link_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListResult {
    pub list: Vec<OrderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderInfo {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "orderLinkId")]
    pub order_link_id: String,
    pub symbol: String,
    #[serde(rename = "orderStatus")]
    pub order_status: String,
    pub side: String,
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub qty: String,
    pub price: String,
    #[serde(rename = "avgPrice")]
    pub avg_price: String,
    #[serde(rename = "cumExecQty")]
    pub cum_exec_qty: String,
    #[serde(rename = "cumExecValue")]
    pub cum_exec_value: String,
    #[serde(rename = "cumExecFee")]
    pub cum_exec_fee: String,
    #[serde(rename = "createdTime")]
    pub created_time: String,
    #[serde(rename = "updatedTime")]
    pub updated_time: String,
}

// Market Pair for internal use
#[derive(Debug, Clone, PartialEq)]
pub struct MarketPair {
    pub base: String,
    pub quote: String,
    pub symbol: String,
    pub price: f64,          // Keep for backwards compatibility (last_price)
    pub bid_price: f64,      // Best bid price
    pub ask_price: f64,      // Best ask price
    pub bid_size: f64,       // Bid quantity
    pub ask_size: f64,       // Ask quantity
    pub volume_24h: f64,     // 24h volume in base currency
    pub volume_24h_usd: f64, // 24h volume in USD
    pub spread_percent: f64, // Bid/ask spread percentage
    pub min_qty: f64,
    pub qty_step: f64,
    pub min_notional: f64,
    pub is_active: bool,
    pub is_liquid: bool, // Meets liquidity requirements
}

impl MarketPair {
    pub fn new(instrument: &InstrumentInfo, ticker: &TickerInfo) -> Option<Self> {
        if instrument.status != "Trading" {
            return None;
        }

        let min_qty = instrument
            .lot_size_filter
            .as_ref()?
            .min_order_qty
            .parse()
            .ok()?;

        let qty_step = instrument
            .lot_size_filter
            .as_ref()?
            .qty_step
            .as_ref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.001); // Default to 0.001 if not available

        let min_notional = instrument
            .lot_size_filter
            .as_ref()
            .and_then(|f| f.min_notional_value.as_ref())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);

        // Parse prices from ticker
        let price = ticker.last_price.as_ref().and_then(|s| s.parse().ok())?;
        let bid_price = ticker.bid1_price.as_ref().and_then(|s| s.parse().ok())?;
        let ask_price = ticker.ask1_price.as_ref().and_then(|s| s.parse().ok())?;
        let bid_size = ticker
            .bid1_size
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let ask_size = ticker
            .ask1_size
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let volume_24h = ticker
            .volume24h
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let turnover_24h = ticker
            .turnover24h
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // Calculate spread percentage
        let spread_percent = if bid_price > 0.0 && ask_price > 0.0 {
            ((ask_price - bid_price) / bid_price) * 100.0
        } else {
            100.0 // Mark as illiquid if prices are invalid
        };

        // Estimate 24h volume in USD (use turnover if available, otherwise estimate)
        let volume_24h_usd = if turnover_24h > 0.0 {
            turnover_24h
        } else {
            volume_24h * price
        };

        // Validate prices
        if price <= 0.0 || bid_price <= 0.0 || ask_price <= 0.0 || bid_price >= ask_price {
            return None;
        }

        // Determine liquidity based on volume and spread
        let is_liquid = volume_24h_usd >= crate::config::MIN_VOLUME_24H_USD
            && spread_percent <= crate::config::MAX_SPREAD_PERCENT
            && bid_size * bid_price >= crate::config::MIN_BID_SIZE_USD
            && ask_size * ask_price >= crate::config::MIN_ASK_SIZE_USD;

        Some(MarketPair {
            base: instrument.base_coin.clone(),
            quote: instrument.quote_coin.clone(),
            symbol: instrument.symbol.clone(),
            price,
            bid_price,
            ask_price,
            bid_size,
            ask_size,
            volume_24h,
            volume_24h_usd,
            spread_percent,
            min_qty,
            qty_step,
            min_notional,
            is_active: true,
            is_liquid,
        })
    }
}

// Triangular Arbitrage Opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub path: Vec<String>,  // [USDT, BTC, ETH, USDT]
    pub pairs: Vec<String>, // [BTCUSDT, ETHBTC, ETHUSDT]
    pub prices: Vec<f64>,
    pub estimated_profit_pct: f64,
    pub estimated_profit_usd: f64,
    pub timestamp: DateTime<Utc>,
}

impl ArbitrageOpportunity {
    pub fn display_path(&self) -> String {
        self.path.join(" → ")
    }

    pub fn display_pairs(&self) -> String {
        self.pairs.join(" → ")
    }
}

// Balance mapping for quick lookups
pub type BalanceMap = HashMap<String, f64>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse {
            ret_code: 0,
            ret_msg: "OK".to_string(),
            result: Some("test_data".to_string()),
            ret_ext_info: None,
            time: Some(1234567890),
        };

        assert!(response.is_success());
        assert_eq!(response.into_result().unwrap(), "test_data");
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse {
            ret_code: 10001,
            ret_msg: "Invalid API key".to_string(),
            result: None,
            ret_ext_info: None,
            time: Some(1234567890),
        };

        assert!(!response.is_success());
        assert!(response.into_result().is_err());
    }
}
