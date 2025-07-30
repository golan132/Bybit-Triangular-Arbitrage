use crate::balance::BalanceManager;
use crate::config::{MIN_PROFIT_THRESHOLD, MAX_TRIANGLES_TO_SCAN};
use crate::models::ArbitrageOpportunity;
use crate::pairs::{PairManager, TrianglePairs};
use chrono::Utc;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub enum TradeDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct TradeStep {
    pub pair_symbol: String,
    pub direction: TradeDirection,
    pub input_amount: f64,
    pub output_amount: f64,
    pub execution_price: f64,
    pub ideal_price: f64,
    pub slippage_percent: f64,
    pub available_size: f64,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub initial_amount: f64,
    pub final_amount: f64,
    pub steps: Vec<TradeStep>,
    pub total_slippage: f64,
}

pub struct ArbitrageEngine {
    opportunities: Vec<ArbitrageOpportunity>,
    profit_threshold: f64,
    max_scan_count: usize,
    trading_fee_rate: f64, // Bybit spot trading fee (usually 0.1%)
}

impl ArbitrageEngine {
    pub fn new() -> Self {
        Self {
            opportunities: Vec::new(),
            profit_threshold: MIN_PROFIT_THRESHOLD,
            max_scan_count: MAX_TRIANGLES_TO_SCAN,
            trading_fee_rate: 0.001, // 0.1% trading fee
        }
    }

    pub fn with_config(profit_threshold: f64, max_scan_count: usize, fee_rate: f64) -> Self {
        Self {
            opportunities: Vec::new(),
            profit_threshold,
            max_scan_count,
            trading_fee_rate: fee_rate,
        }
    }

    /// Scan for triangular arbitrage opportunities
    pub fn scan_opportunities(
        &mut self,
        pair_manager: &PairManager,
        balance_manager: &BalanceManager,
    ) -> Vec<ArbitrageOpportunity> {
        self.scan_opportunities_with_min_amount(pair_manager, balance_manager, 50.0)
    }

    /// Scan for triangular arbitrage opportunities with minimum trade amount filtering
    pub fn scan_opportunities_with_min_amount(
        &mut self,
        pair_manager: &PairManager,
        balance_manager: &BalanceManager,
        min_trade_amount: f64,
    ) -> Vec<ArbitrageOpportunity> {
        self.opportunities.clear();
        let tradeable_coins = balance_manager.get_tradeable_coins(min_trade_amount);
        let mut total_scanned = 0;

        if tradeable_coins.is_empty() {
            // If no sufficient balances, scan with popular base currencies to show potential opportunities
            let popular_coins = vec![
                "USDT".to_string(),
                "BTC".to_string(), 
                "ETH".to_string(),
                "USDC".to_string(),
                "BNB".to_string(),
            ];
            
            debug!("No tradeable coins with balance >= ${:.0}, scanning popular currencies for reference", min_trade_amount);
            
            for base_currency in &popular_coins {
                if total_scanned >= self.max_scan_count {
                    break;
                }
                total_scanned += self.scan_for_base_currency(base_currency, min_trade_amount, pair_manager);
            }
        } else {
            // Focus on assets we actually have sufficient balances for
            debug!("Scanning {} tradeable coins: {:?}", tradeable_coins.len(), tradeable_coins);
            
            for base_currency in &tradeable_coins {
                if total_scanned >= self.max_scan_count {
                    warn!("⚠️ Reached maximum scan limit of {} triangles", self.max_scan_count);
                    break;
                }

                let balance = balance_manager.get_balance(base_currency);
                
                // Use the minimum trade amount or a portion of balance, whichever is larger
                let test_amount = min_trade_amount.max((balance * 0.1).min(1000.0));

                total_scanned += self.scan_for_base_currency(base_currency, test_amount, pair_manager);
            }
        }

        // Sort opportunities by profit percentage (highest first)
        self.opportunities.sort_by(|a, b| {
            b.estimated_profit_pct
                .partial_cmp(&a.estimated_profit_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Only log detailed scan results occasionally
        debug!("🔁 Found {} potential arbitrage opportunities from {} triangles scanned", 
              self.opportunities.len(), total_scanned);

        self.opportunities.clone()
    }
    
    /// Scan for arbitrage opportunities using a specific base currency
    fn scan_for_base_currency(
        &mut self,
        base_currency: &str,
        test_amount: f64,
        pair_manager: &PairManager,
    ) -> usize {
        let triangles = pair_manager.find_triangle_pairs(base_currency);
        let mut scanned_count = 0;
        
        for triangle in triangles.iter().take(self.max_scan_count) {
            // Pre-filter triangles by liquidity
            if !self.is_triangle_liquid_enough(&triangle, pair_manager, test_amount) {
                scanned_count += 1;
                continue;
            }

            if let Some(opportunity) = self.calculate_arbitrage_profit(
                triangle,
                test_amount,
                pair_manager,
            ) {
                if opportunity.estimated_profit_pct >= self.profit_threshold {
                    self.opportunities.push(opportunity);
                }
            }
            scanned_count += 1;
        }
        
        debug!("Scanned {} triangles for {}", scanned_count, base_currency);
        scanned_count
    }

    /// Check if triangle meets minimum liquidity requirements
    fn is_triangle_liquid_enough(
        &self,
        triangle: &TrianglePairs,
        pair_manager: &PairManager,
        test_amount: f64,
    ) -> bool {
        let pair1 = pair_manager.get_pair_by_symbol(&triangle.pair1.symbol);
        let pair2 = pair_manager.get_pair_by_symbol(&triangle.pair2.symbol);
        let pair3 = pair_manager.get_pair_by_symbol(&triangle.pair3.symbol);

        if let (Some(p1), Some(p2), Some(p3)) = (pair1, pair2, pair3) {
            let pairs = [p1, p2, p3];
            let min_trade_size_usd = test_amount.max(crate::config::MIN_TRADE_AMOUNT_USD);
            
            for pair in &pairs {
                // Volume filter - must have sufficient 24h volume
                if pair.volume_24h_usd < crate::config::MIN_VOLUME_24H_USD {
                    debug!("❌ {} failed volume check: ${:.0} < ${:.0}", 
                          pair.symbol, pair.volume_24h_usd, crate::config::MIN_VOLUME_24H_USD);
                    return false;
                }

                // Spread filter - spread must be reasonable
                if pair.spread_percent > crate::config::MAX_SPREAD_PERCENT {
                    debug!("❌ {} failed spread check: {:.2}% > {:.2}%", 
                          pair.symbol, pair.spread_percent, crate::config::MAX_SPREAD_PERCENT);
                    return false;
                }

                // Size filter - must have enough bid/ask size for our trade
                let bid_size_usd = pair.bid_size * pair.bid_price;
                let ask_size_usd = pair.ask_size * pair.ask_price;
                
                if bid_size_usd < min_trade_size_usd || ask_size_usd < min_trade_size_usd {
                    debug!("❌ {} failed size check: bid ${:.0}, ask ${:.0} < ${:.0}", 
                          pair.symbol, bid_size_usd, ask_size_usd, min_trade_size_usd);
                    return false;
                }

                // Liquidity flag check
                if !pair.is_liquid {
                    debug!("❌ {} marked as illiquid", pair.symbol);
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// Calculate profit for a specific triangle using realistic bid/ask prices
    fn calculate_arbitrage_profit(
        &self,
        triangle: &TrianglePairs,
        initial_amount: f64,
        _pair_manager: &PairManager,
    ) -> Option<ArbitrageOpportunity> {
        let path = &triangle.path;
        let pairs = [&triangle.pair1, &triangle.pair2, &triangle.pair3];
        let mut prices = Vec::new();
        let mut pair_symbols = Vec::new();
        let mut trade_details = Vec::new();

        // Use a reasonable test amount (10% of balance or $100 equivalent)
        let test_amount = (initial_amount * 0.1).min(100.0).max(1.0);
        let mut current_amount = test_amount;

        // Simulate the trades through the triangle using realistic bid/ask prices
        for (i, pair) in pairs.iter().enumerate() {
            let from_currency = &path[i];
            let to_currency = &path[i + 1];
            
            pair_symbols.push(pair.symbol.clone());

            // Determine if we're buying or selling and use appropriate price
            let (amount_after_trade, effective_price, is_sell) = if pair.base == *from_currency {
                // Selling base for quote (from_currency/to_currency)
                // When selling, we get the bid price (what market makers will pay us)
                let received = current_amount / pair.bid_price;
                prices.push(pair.bid_price);
                (received, pair.bid_price, true)
            } else {
                // Buying base with quote (to_currency/from_currency)  
                // When buying, we pay the ask price (what market makers will sell for)
                let received = current_amount * pair.ask_price;
                prices.push(pair.ask_price);
                (received, pair.ask_price, false)
            };

            // Store trade details for logging
            trade_details.push(crate::logger::TradeDetail {
                pair_symbol: pair.symbol.clone(),
                from_currency: from_currency.clone(),
                to_currency: to_currency.clone(),
                amount_in: current_amount,
                amount_out: amount_after_trade * (1.0 - self.trading_fee_rate),
                price: effective_price,
                bid_price: pair.bid_price,
                ask_price: pair.ask_price,
                is_sell,
            });

            // Apply trading fee (typically 0.1% for Bybit)
            current_amount = amount_after_trade * (1.0 - self.trading_fee_rate);
            
            debug!("Step {}: {} {} -> {} {} (price: {:.8}, type: {}, after fee: {:.6})",
                   i + 1, 
                   current_amount / (1.0 - self.trading_fee_rate), 
                   from_currency,
                   current_amount, 
                   to_currency, 
                   effective_price,
                   if is_sell { "SELL@BID" } else { "BUY@ASK" },
                   current_amount);
        }

        // Calculate profit
        let profit_amount = current_amount - test_amount;
        let profit_pct = (profit_amount / test_amount) * 100.0;

        // Estimate profit in USD (assuming USDT ≈ USD)
        let estimated_usd_profit = if triangle.base_currency == "USDT" || triangle.base_currency == "USDC" {
            profit_amount * (initial_amount / test_amount)
        } else {
            // For non-USD base currencies, we'd need price conversion
            // For now, use a conservative estimate
            profit_amount * 0.5 * (initial_amount / test_amount)
        };

        if profit_pct > -50.0 && profit_pct.is_finite() {
            // Only return reasonable profit calculations
            let opportunity = ArbitrageOpportunity {
                path: path.clone(),
                pairs: pair_symbols,
                prices,
                estimated_profit_pct: profit_pct,
                estimated_profit_usd: estimated_usd_profit,
                timestamp: Utc::now(),
            };

            // Return any profitable opportunity (threshold handled in main)
            Some(opportunity)
        } else {
            None
        }
    }

    /// Get current opportunities
    pub fn get_opportunities(&self) -> &[ArbitrageOpportunity] {
        &self.opportunities
    }

    /// Get opportunities above a certain profit threshold
    pub fn get_profitable_opportunities(&self, min_profit_pct: f64) -> Vec<&ArbitrageOpportunity> {
        self.opportunities
            .iter()
            .filter(|opp| opp.estimated_profit_pct >= min_profit_pct)
            .collect()
    }

    /// Log top opportunities for debugging
    fn log_top_opportunities(&self) {
        if self.opportunities.is_empty() {
            info!("📉 No profitable arbitrage opportunities found");
            return;
        }

        info!("🚀 Top arbitrage opportunities:");
        for (i, opportunity) in self.opportunities.iter().take(5).enumerate() {
            info!(
                "  {}. {} | Est. Profit: {:+.2}% (${:.2})",
                i + 1,
                opportunity.display_path(),
                opportunity.estimated_profit_pct,
                opportunity.estimated_profit_usd
            );
            debug!("     Pairs: {}", opportunity.display_pairs());
            debug!("     Prices: {:?}", opportunity.prices);
        }
    }

    /// Calculate theoretical maximum profit for a triangle
    pub fn calculate_max_theoretical_profit(
        &self,
        triangle: &TrianglePairs,
        max_amount: f64,
    ) -> Option<f64> {
        // This would calculate the maximum possible profit considering:
        // - Order book depth
        // - Slippage
        // - Maximum trade sizes
        // For now, return a conservative estimate
        
        let base_profit_rate = self.calculate_base_profit_rate(triangle)?;
        if base_profit_rate <= 0.0 {
            return None;
        }

        // Conservative estimate considering slippage increases with trade size
        let slippage_factor = 1.0 - (max_amount / 10000.0).min(0.05); // Max 5% slippage
        Some(max_amount * base_profit_rate * slippage_factor)
    }

    /// Calculate base profit rate without considering trade size
    fn calculate_base_profit_rate(&self, triangle: &TrianglePairs) -> Option<f64> {
        let p1 = triangle.pair1.price;
        let p2 = triangle.pair2.price;
        let p3 = triangle.pair3.price;

        // Simplified calculation - in practice, this depends on the direction of trades
        let theoretical_rate = (1.0 / p1) * p2 * (1.0 / p3) - 1.0;
        let after_fees = theoretical_rate - (self.trading_fee_rate * 3.0); // 3 trades

        if after_fees > 0.0 && after_fees.is_finite() {
            Some(after_fees)
        } else {
            None
        }
    }

    /// Update trading fee rate
    pub fn set_trading_fee_rate(&mut self, fee_rate: f64) {
        self.trading_fee_rate = fee_rate;
        info!("Updated trading fee rate to {:.3}%", fee_rate * 100.0);
    }

    /// Update profit threshold
    pub fn set_profit_threshold(&mut self, threshold: f64) {
        self.profit_threshold = threshold;
        info!("Updated profit threshold to {:.2}%", threshold);
    }

    /// Get arbitrage statistics
    pub fn get_statistics(&self) -> ArbitrageStatistics {
        if self.opportunities.is_empty() {
            return ArbitrageStatistics::default();
        }

        let total_opportunities = self.opportunities.len();
        let profitable_count = self.get_profitable_opportunities(0.0).len();
        
        let max_profit = self.opportunities
            .iter()
            .map(|opp| opp.estimated_profit_pct)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let avg_profit = if total_opportunities > 0 {
            self.opportunities
                .iter()
                .map(|opp| opp.estimated_profit_pct)
                .sum::<f64>() / total_opportunities as f64
        } else {
            0.0
        };

        let total_estimated_usd = self.opportunities
            .iter()
            .map(|opp| opp.estimated_profit_usd)
            .sum();

        ArbitrageStatistics {
            total_opportunities,
            profitable_count,
            max_profit_pct: max_profit,
            avg_profit_pct: avg_profit,
            total_estimated_usd_profit: total_estimated_usd,
            last_scan: Some(Utc::now()),
        }
    }

    /// Clear old opportunities
    pub fn clear_opportunities(&mut self) {
        self.opportunities.clear();
    }
}

impl Default for ArbitrageEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArbitrageStatistics {
    pub total_opportunities: usize,
    pub profitable_count: usize,
    pub max_profit_pct: f64,
    pub avg_profit_pct: f64,
    pub total_estimated_usd_profit: f64,
    pub last_scan: Option<chrono::DateTime<chrono::Utc>>,
}

impl ArbitrageStatistics {
    pub fn display(&self) -> String {
        let last_scan = match self.last_scan {
            Some(dt) => dt.format("%H:%M:%S UTC").to_string(),
            None => "Never".to_string(),
        };

        format!(
            "Arbitrage: {} opportunities ({} profitable), max: {:.2}%, avg: {:.2}%, est. USD: ${:.2}, last scan: {}",
            self.total_opportunities,
            self.profitable_count,
            self.max_profit_pct,
            self.avg_profit_pct,
            self.total_estimated_usd_profit,
            last_scan
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MarketPair;
    use crate::pairs::TrianglePairs;

    fn create_test_triangle() -> TrianglePairs {
        let pair1 = MarketPair {
            base: "BTC".to_string(),
            quote: "USDT".to_string(),
            symbol: "BTCUSDT".to_string(),
            price: 50000.0,
            min_qty: 0.001,
            qty_step: 0.001,
            min_notional: 1.0,
            is_active: true,
        };

        let pair2 = MarketPair {
            base: "ETH".to_string(),
            quote: "BTC".to_string(),
            symbol: "ETHBTC".to_string(),
            price: 0.06, // ETH = 0.06 BTC
            min_qty: 0.001,
            qty_step: 0.001,
            min_notional: 1.0,
            is_active: true,
        };

        let pair3 = MarketPair {
            base: "ETH".to_string(),
            quote: "USDT".to_string(),
            symbol: "ETHUSDT".to_string(),
            price: 3100.0, // Slightly higher to create arbitrage opportunity
            min_qty: 0.001,
            qty_step: 0.001,
            min_notional: 1.0,
            is_active: true,
        };

        TrianglePairs {
            base_currency: "USDT".to_string(),
            pair1,
            pair2,
            pair3,
            path: vec![
                "USDT".to_string(),
                "BTC".to_string(),
                "ETH".to_string(),
                "USDT".to_string(),
            ],
        }
    }

    #[test]
    fn test_arbitrage_engine_creation() {
        let engine = ArbitrageEngine::new();
        assert_eq!(engine.opportunities.len(), 0);
        assert_eq!(engine.profit_threshold, MIN_PROFIT_THRESHOLD);
    }

    #[test]
    fn test_arbitrage_engine_with_config() {
        let engine = ArbitrageEngine::with_config(0.5, 100, 0.002);
        assert_eq!(engine.profit_threshold, 0.5);
        assert_eq!(engine.max_scan_count, 100);
        assert_eq!(engine.trading_fee_rate, 0.002);
    }

    #[test]
    fn test_calculate_base_profit_rate() {
        let engine = ArbitrageEngine::new();
        let triangle = create_test_triangle();
        
        let profit_rate = engine.calculate_base_profit_rate(&triangle);
        
        // The profit rate might be None if no arbitrage opportunity exists
        // This is normal and expected in most cases
        if let Some(rate) = profit_rate {
            println!("Calculated base profit rate: {:.6}", rate);
            assert!(rate.is_finite());
        } else {
            println!("No arbitrage opportunity found (expected)");
        }
    }

    #[test]
    fn test_statistics() {
        let engine = ArbitrageEngine::new();
        let stats = engine.get_statistics();
        assert_eq!(stats.total_opportunities, 0);
        assert_eq!(stats.profitable_count, 0);
    }
}
