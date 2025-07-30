use crate::client::BybitClient;
use crate::config;
use crate::models::{InstrumentInfo, MarketPair, TickerInfo};
use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

pub struct PairManager {
    pairs: Vec<MarketPair>,
    price_map: HashMap<String, f64>,
    symbol_to_pair: HashMap<String, usize>,
    last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl PairManager {
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            price_map: HashMap::new(),
            symbol_to_pair: HashMap::new(),
            last_updated: None,
        }
    }

    /// Fetch all trading pairs and their current prices
    pub async fn update_pairs_and_prices(&mut self, client: &BybitClient) -> Result<()> {
        info!("🔄 Updating trading pairs and prices...");

        // Fetch instruments
        let instruments = client
            .get_all_spot_instruments()
            .await
            .context("Failed to fetch instruments")?;

        // Fetch tickers for prices
        let tickers_result = client
            .get_tickers("spot")
            .await
            .context("Failed to fetch tickers")?;

        // Create ticker map for quick lookup
        let mut ticker_map = HashMap::new();
        for ticker in &tickers_result.list {
            ticker_map.insert(ticker.symbol.clone(), ticker);
        }

        // Create price map from tickers (for backward compatibility)
        let mut price_map = HashMap::new();
        for ticker in &tickers_result.list {
            if let Ok(price) = ticker.last_price.parse::<f64>() {
                price_map.insert(ticker.symbol.clone(), price);
            }
        }

        // Create market pairs with bid/ask data, filtering out blacklisted tokens
        let mut pairs = Vec::new();
        let mut symbol_to_pair = HashMap::new();
        let mut blacklisted_count = 0;

        for (idx, instrument) in instruments.iter().enumerate() {
            // Check if base or quote currency is blacklisted
            if config::is_token_blacklisted(&instrument.base_coin) || 
               config::is_token_blacklisted(&instrument.quote_coin) {
                blacklisted_count += 1;
                continue;
            }

            if let Some(ticker) = ticker_map.get(&instrument.symbol) {
                if let Some(market_pair) = MarketPair::new(instrument, ticker) {
                    symbol_to_pair.insert(market_pair.symbol.clone(), idx);
                    pairs.push(market_pair);
                }
            }
        }

        // Filter out pairs with zero or invalid prices
        pairs.retain(|pair| pair.price > 0.0 && pair.price.is_finite() && 
                           pair.bid_price > 0.0 && pair.ask_price > 0.0 && 
                           pair.bid_price < pair.ask_price);

        if blacklisted_count > 0 {
            info!("🚫 Filtered out {} pairs containing blacklisted tokens", blacklisted_count);
        }

        self.pairs = pairs;
        self.price_map = price_map;
        self.symbol_to_pair = symbol_to_pair;
        self.last_updated = Some(chrono::Utc::now());

        info!("✅ Updated {} trading pairs with current prices", self.pairs.len());
        self.log_pair_statistics();
        self.log_bid_ask_analysis();

        Ok(())
    }

    /// Get all market pairs
    pub fn get_pairs(&self) -> &[MarketPair] {
        &self.pairs
    }

    /// Get pairs filtered by base or quote currency
    pub fn get_pairs_with_currency(&self, currency: &str) -> Vec<&MarketPair> {
        self.pairs
            .iter()
            .filter(|pair| pair.base == currency || pair.quote == currency)
            .collect()
    }

    /// Get all unique currencies from pairs
    pub fn get_all_currencies(&self) -> Vec<String> {
        let mut currencies = std::collections::HashSet::new();
        
        for pair in &self.pairs {
            currencies.insert(pair.base.clone());
            currencies.insert(pair.quote.clone());
        }
        
        let mut result: Vec<String> = currencies.into_iter().collect();
        result.sort();
        result
    }

    /// Find a specific pair by symbol
    pub fn get_pair_by_symbol(&self, symbol: &str) -> Option<&MarketPair> {
        self.symbol_to_pair
            .get(symbol)
            .and_then(|&idx| self.pairs.get(idx))
    }

    /// Get current price for a symbol
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        self.price_map.get(symbol).copied()
    }

    /// Update price for a specific symbol (for real-time updates)
    pub fn update_price(&mut self, symbol: &str, price: f64) {
        if let Some(existing_price) = self.price_map.get_mut(symbol) {
            *existing_price = price;
        }

        // Also update the price in the corresponding MarketPair
        if let Some(&idx) = self.symbol_to_pair.get(symbol) {
            if let Some(pair) = self.pairs.get_mut(idx) {
                pair.price = price;
            }
        }
    }

    /// Find pairs that could form triangular arbitrage with given base currency
    pub fn find_triangle_pairs(&self, base_currency: &str) -> Vec<TrianglePairs> {
        let mut triangles = Vec::new();
        
        // Only consider liquid pairs for arbitrage
        let liquid_pairs: Vec<&MarketPair> = self.pairs
            .iter()
            .filter(|pair| pair.is_liquid && pair.is_active)
            .collect();

        let pairs_with_base: Vec<&MarketPair> = liquid_pairs
            .iter()
            .filter(|pair| pair.base == base_currency || pair.quote == base_currency)
            .cloned()
            .collect();

        debug!("🔍 Looking for triangles with {} liquid pairs containing {}", 
               pairs_with_base.len(), base_currency);

        for pair1 in &pairs_with_base {
            // pair1: base -> intermediate
            let intermediate = if pair1.base == base_currency {
                &pair1.quote
            } else {
                &pair1.base
            };

            if intermediate == base_currency {
                continue; // Skip self-referencing pairs
            }

            let pairs_with_intermediate: Vec<&MarketPair> = liquid_pairs
                .iter()
                .filter(|pair| pair.base == *intermediate || pair.quote == *intermediate)
                .cloned()
                .collect();

            for pair2 in &pairs_with_intermediate {
                if pair2.symbol == pair1.symbol {
                    continue; // Skip same pair
                }

                // pair2: intermediate -> final
                let final_currency = if pair2.base == *intermediate {
                    &pair2.quote
                } else {
                    &pair2.base
                };

                if final_currency == base_currency || final_currency == intermediate {
                    continue; // Skip circular or same currency
                }

                // pair3: final -> base (closing the triangle)
                let pairs_with_final: Vec<&MarketPair> = liquid_pairs
                    .iter()
                    .filter(|pair| pair.base == *final_currency || pair.quote == *final_currency)
                    .cloned()
                    .collect();

                for pair3 in &pairs_with_final {
                    if pair3.symbol == pair1.symbol || pair3.symbol == pair2.symbol {
                        continue; // Skip duplicate pairs
                    }

                    let closes_loop = (pair3.base == *final_currency && pair3.quote == *base_currency)
                        || (pair3.quote == *final_currency && pair3.base == *base_currency);

                    if closes_loop {
                        triangles.push(TrianglePairs {
                            base_currency: base_currency.to_string(),
                            pair1: (*pair1).clone(),
                            pair2: (*pair2).clone(),
                            pair3: (*pair3).clone(),
                            path: vec![
                                base_currency.to_string(),
                                intermediate.to_string(),
                                final_currency.to_string(),
                                base_currency.to_string(),
                            ],
                        });
                    }
                }
            }
        }

        debug!("Found {} potential triangles for base currency {}", 
               triangles.len(), base_currency);
        
        triangles
    }

    /// Get trading statistics
    pub fn get_statistics(&self) -> PairStatistics {
        if self.pairs.is_empty() {
            return PairStatistics::default();
        }

        let currencies = self.get_all_currencies();
        let avg_price = self.pairs.iter().map(|p| p.price).sum::<f64>() / self.pairs.len() as f64;
        
        let min_price = self.pairs
            .iter()
            .map(|p| p.price)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let max_price = self.pairs
            .iter()
            .map(|p| p.price)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        PairStatistics {
            total_pairs: self.pairs.len(),
            total_currencies: currencies.len(),
            active_pairs: self.pairs.iter().filter(|p| p.is_active).count(),
            avg_price,
            min_price,
            max_price,
            last_updated: self.last_updated,
        }
    }

    /// Check if data needs refresh
    pub fn needs_refresh(&self, interval_secs: u64) -> bool {
        match self.last_updated {
            None => true,
            Some(last_update) => {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(last_update);
                duration.num_seconds() as u64 >= interval_secs
            }
        }
    }

    /// Log pair statistics for debugging
    fn log_pair_statistics(&self) {
        let stats = self.get_statistics();
        let liquid_pairs = self.pairs.iter().filter(|p| p.is_liquid).count();
        
        info!("📊 Pair Statistics:");
        info!("  Total pairs: {}", stats.total_pairs);
        info!("  Active pairs: {}", stats.active_pairs);
        info!("  Liquid pairs: {} ({:.1}%)", liquid_pairs, (liquid_pairs as f64 / stats.total_pairs as f64) * 100.0);
        info!("  Total currencies: {}", stats.total_currencies);
        info!("  Price range: {:.8} - {:.8}", stats.min_price, stats.max_price);
        
        // Volume statistics
        let volumes: Vec<f64> = self.pairs.iter().map(|p| p.volume_24h_usd).collect();
        let total_volume: f64 = volumes.iter().sum();
        let avg_volume = if !volumes.is_empty() { total_volume / volumes.len() as f64 } else { 0.0 };
        info!("  Total 24h volume: ${:.0}", total_volume);
        info!("  Average 24h volume: ${:.0}", avg_volume);

        // Show liquidity thresholds
        info!("🧪 Liquidity Filters:");
        info!("  Min 24h volume: ${:.0}", crate::config::MIN_VOLUME_24H_USD);
        info!("  Max spread: {:.1}%", crate::config::MAX_SPREAD_PERCENT);
        info!("  Min trade size: ${:.0}", crate::config::MIN_TRADE_AMOUNT_USD);
        
        // Log some popular currencies
        let popular_currencies = ["USDT", "BTC", "ETH", "BNB", "USDC"];
        for currency in &popular_currencies {
            let count = self.get_pairs_with_currency(currency).len();
            let liquid_count = self.pairs
                .iter()
                .filter(|p| p.is_liquid && (p.base == *currency || p.quote == *currency))
                .count();
            if count > 0 {
                debug!("  {} pairs: {} (liquid: {})", currency, count, liquid_count);
            }
        }
    }

    /// Log bid/ask spread analysis for debugging
    fn log_bid_ask_analysis(&self) {
        if self.pairs.is_empty() {
            return;
        }

        let spreads: Vec<f64> = self.pairs
            .iter()
            .map(|pair| ((pair.ask_price - pair.bid_price) / pair.bid_price) * 100.0)
            .collect();

        let avg_spread = spreads.iter().sum::<f64>() / spreads.len() as f64;
        let min_spread = spreads.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0);
        let max_spread = spreads.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0);

        info!("📈 Bid/Ask Spread Analysis:");
        info!("  Average spread: {:.4}%", avg_spread);
        info!("  Spread range: {:.4}% - {:.4}%", min_spread, max_spread);

        // Show some examples of major pairs
        let major_pairs = ["BTCUSDT", "ETHUSDT", "BNBUSDT"];
        for symbol in &major_pairs {
            if let Some(pair) = self.pairs.iter().find(|p| p.symbol == *symbol) {
                let spread = ((pair.ask_price - pair.bid_price) / pair.bid_price) * 100.0;
                debug!("  {} spread: {:.4}% (bid: {:.4}, ask: {:.4})", 
                       symbol, spread, pair.bid_price, pair.ask_price);
            }
        }
    }

    /// Filter pairs by minimum price and volume requirements
    pub fn filter_liquid_pairs(&self, min_price: f64, _min_volume_24h: f64) -> Vec<&MarketPair> {
        self.pairs
            .iter()
            .filter(|pair| {
                pair.price >= min_price 
                    && pair.is_active
                    // Note: We'd need 24h volume data from tickers for complete filtering
            })
            .collect()
    }

    /// Get pairs suitable for arbitrage (active, reasonable prices)
    pub fn get_arbitrage_suitable_pairs(&self) -> Vec<&MarketPair> {
        self.pairs
            .iter()
            .filter(|pair| {
                pair.is_active 
                    && pair.price > 0.0 
                    && pair.price.is_finite()
                    && pair.min_qty > 0.0
            })
            .collect()
    }
}

impl Default for PairManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TrianglePairs {
    pub base_currency: String,
    pub pair1: MarketPair,
    pub pair2: MarketPair,
    pub pair3: MarketPair,
    pub path: Vec<String>,
}

impl TrianglePairs {
    pub fn get_symbols(&self) -> Vec<String> {
        vec![
            self.pair1.symbol.clone(),
            self.pair2.symbol.clone(),
            self.pair3.symbol.clone(),
        ]
    }

    pub fn display_path(&self) -> String {
        self.path.join(" → ")
    }
}

#[derive(Debug, Clone, Default)]
pub struct PairStatistics {
    pub total_pairs: usize,
    pub total_currencies: usize,
    pub active_pairs: usize,
    pub avg_price: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl PairStatistics {
    pub fn display(&self) -> String {
        let last_update = match self.last_updated {
            Some(dt) => dt.format("%H:%M:%S UTC").to_string(),
            None => "Never".to_string(),
        };

        format!(
            "Pairs: {} total ({} active), {} currencies, avg price: {:.6}, updated: {}",
            self.total_pairs, self.active_pairs, self.total_currencies, 
            self.avg_price, last_update
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MarketPair;

    fn create_test_pair(symbol: &str, base: &str, quote: &str, price: f64) -> MarketPair {
        MarketPair {
            base: base.to_string(),
            quote: quote.to_string(),
            symbol: symbol.to_string(),
            price,
            min_qty: 0.001,
            qty_step: 0.001,
            min_notional: 1.0,
            is_active: true,
        }
    }

    #[test]
    fn test_pair_manager_creation() {
        let manager = PairManager::new();
        assert_eq!(manager.pairs.len(), 0);
        assert!(manager.last_updated.is_none());
    }

    #[test]
    fn test_get_pairs_with_currency() {
        let mut manager = PairManager::new();
        manager.pairs = vec![
            create_test_pair("BTCUSDT", "BTC", "USDT", 50000.0),
            create_test_pair("ETHUSDT", "ETH", "USDT", 3000.0),
            create_test_pair("ETHBTC", "ETH", "BTC", 0.06),
        ];

        let usdt_pairs = manager.get_pairs_with_currency("USDT");
        assert_eq!(usdt_pairs.len(), 2);

        let btc_pairs = manager.get_pairs_with_currency("BTC");
        assert_eq!(btc_pairs.len(), 2);
    }

    #[test]
    fn test_get_all_currencies() {
        let mut manager = PairManager::new();
        manager.pairs = vec![
            create_test_pair("BTCUSDT", "BTC", "USDT", 50000.0),
            create_test_pair("ETHUSDT", "ETH", "USDT", 3000.0),
            create_test_pair("ETHBTC", "ETH", "BTC", 0.06),
        ];

        let currencies = manager.get_all_currencies();
        assert_eq!(currencies.len(), 3);
        assert!(currencies.contains(&"BTC".to_string()));
        assert!(currencies.contains(&"ETH".to_string()));
        assert!(currencies.contains(&"USDT".to_string()));
    }

    #[test]
    fn test_find_triangle_pairs() {
        let mut manager = PairManager::new();
        manager.pairs = vec![
            create_test_pair("BTCUSDT", "BTC", "USDT", 50000.0),
            create_test_pair("ETHUSDT", "ETH", "USDT", 3000.0),
            create_test_pair("ETHBTC", "ETH", "BTC", 0.06),
        ];

        // Rebuild symbol map
        for (idx, pair) in manager.pairs.iter().enumerate() {
            manager.symbol_to_pair.insert(pair.symbol.clone(), idx);
        }

        let triangles = manager.find_triangle_pairs("USDT");
        assert!(!triangles.is_empty());
        
        // Should find USDT -> BTC -> ETH -> USDT or USDT -> ETH -> BTC -> USDT
        let first_triangle = &triangles[0];
        assert_eq!(first_triangle.base_currency, "USDT");
        assert_eq!(first_triangle.path[0], "USDT");
        assert_eq!(first_triangle.path[3], "USDT");
    }
}
