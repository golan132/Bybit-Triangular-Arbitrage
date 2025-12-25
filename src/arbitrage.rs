use crate::balance::BalanceManager;
use crate::config::{MAX_TRIANGLES_TO_SCAN, MIN_PROFIT_THRESHOLD};
use crate::models::ArbitrageOpportunity;
use crate::pairs::{PairManager, TrianglePairs};
use chrono::Utc;
use rayon::prelude::*;
use tracing::debug;

pub struct ArbitrageEngine {
    opportunities: Vec<ArbitrageOpportunity>,
    profit_threshold: f64,
    max_scan_count: usize,
    trading_fee_rate: f64, // Bybit spot trading fee (usually 0.1%)
    pub global_best: Option<ArbitrageOpportunity>,
}

impl ArbitrageEngine {
    pub fn new() -> Self {
        Self {
            opportunities: Vec::new(),
            profit_threshold: MIN_PROFIT_THRESHOLD,
            max_scan_count: MAX_TRIANGLES_TO_SCAN,
            trading_fee_rate: 0.001, // 0.1% trading fee
            global_best: None,
        }
    }

    pub fn with_config(profit_threshold: f64, max_scan_count: usize, fee_rate: f64) -> Self {
        Self {
            opportunities: Vec::new(),
            profit_threshold,
            max_scan_count,
            trading_fee_rate: fee_rate,
            global_best: None,
        }
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

        let coins_to_scan = if tradeable_coins.is_empty() {
            debug!("No tradeable coins with balance >= ${:.0}, scanning popular currencies for reference", min_trade_amount);
            vec![
                "USDT".to_string(),
                "BTC".to_string(),
                "ETH".to_string(),
                "USDC".to_string(),
                "BNB".to_string(),
            ]
        } else {
            debug!(
                "Scanning {} tradeable coins: {:?}",
                tradeable_coins.len(),
                tradeable_coins
            );
            tradeable_coins
        };

        // Use Rayon for parallel scanning
        let results: Vec<(
            usize,
            Vec<ArbitrageOpportunity>,
            Option<ArbitrageOpportunity>,
        )> = coins_to_scan
            .par_iter()
            .map(|base_currency| {
                let balance = balance_manager.get_balance(base_currency);
                // Use the minimum trade amount or a portion of balance, whichever is larger
                let test_amount = min_trade_amount.max((balance * 0.1).min(1000.0));

                self.scan_for_base_currency(base_currency, test_amount, pair_manager)
            })
            .collect();

        let mut total_scanned = 0;
        let mut cycle_best: Option<ArbitrageOpportunity> = None;

        for (scanned, opps, best_in_coin) in results {
            total_scanned += scanned;
            self.opportunities.extend(opps);

            if let Some(best) = best_in_coin {
                if cycle_best
                    .as_ref()
                    .map_or(true, |o| best.estimated_profit_pct > o.estimated_profit_pct)
                {
                    cycle_best = Some(best);
                }
            }
        }

        // Update global best
        if let Some(ref current) = cycle_best {
            if self.global_best.as_ref().map_or(true, |g| {
                current.estimated_profit_pct > g.estimated_profit_pct
            }) {
                self.global_best = Some(current.clone());
            }
        }

        // Log best opportunities
        if let Some(best) = &cycle_best {
            debug!(
                "📉 Cycle Best: {:.4}% via {} (Prices: {:?})",
                best.estimated_profit_pct,
                best.display_pairs(),
                best.prices
            );
        }
        if let Some(global) = &self.global_best {
            debug!(
                "🏆 Global Best: {:.4}% via {} (Prices: {:?})",
                global.estimated_profit_pct,
                global.display_pairs(),
                global.prices
            );
        }

        // Sort opportunities by profit percentage (highest first)
        self.opportunities.sort_by(|a, b| {
            b.estimated_profit_pct
                .partial_cmp(&a.estimated_profit_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Only log detailed scan results occasionally
        debug!(
            "🔁 Found {} potential arbitrage opportunities from {} triangles scanned",
            self.opportunities.len(),
            total_scanned
        );

        self.opportunities.clone()
    }

    /// Scan for arbitrage opportunities using a specific base currency
    fn scan_for_base_currency(
        &self,
        base_currency: &str,
        test_amount: f64,
        pair_manager: &PairManager,
    ) -> (
        usize,
        Vec<ArbitrageOpportunity>,
        Option<ArbitrageOpportunity>,
    ) {
        let triangles = pair_manager.find_triangle_pairs(base_currency);
        let mut scanned_count = 0;
        let mut found_opportunities = Vec::new();
        let mut best_opp: Option<ArbitrageOpportunity> = None;

        for triangle in triangles.iter().take(self.max_scan_count) {
            // Pre-filter triangles by liquidity
            if !self.is_triangle_liquid_enough(&triangle, pair_manager, test_amount) {
                scanned_count += 1;
                continue;
            }

            if let Some(opportunity) =
                self.calculate_arbitrage_profit(triangle, test_amount, pair_manager)
            {
                if best_opp.as_ref().map_or(true, |o| {
                    opportunity.estimated_profit_pct > o.estimated_profit_pct
                }) {
                    best_opp = Some(opportunity.clone());
                }

                if opportunity.estimated_profit_pct >= self.profit_threshold {
                    found_opportunities.push(opportunity);
                }
            }
            scanned_count += 1;
        }

        debug!("Scanned {} triangles for {}", scanned_count, base_currency);
        (scanned_count, found_opportunities, best_opp)
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
                    debug!(
                        "❌ {} failed volume check: ${:.0} < ${:.0}",
                        pair.symbol,
                        pair.volume_24h_usd,
                        crate::config::MIN_VOLUME_24H_USD
                    );
                    return false;
                }

                // Spread filter - spread must be reasonable
                if pair.spread_percent > crate::config::MAX_SPREAD_PERCENT {
                    debug!(
                        "❌ {} failed spread check: {:.2}% > {:.2}%",
                        pair.symbol,
                        pair.spread_percent,
                        crate::config::MAX_SPREAD_PERCENT
                    );
                    return false;
                }

                // Size filter - must have enough bid/ask size for our trade
                let bid_size_usd = pair.bid_size * pair.bid_price;
                let ask_size_usd = pair.ask_size * pair.ask_price;

                if bid_size_usd < min_trade_size_usd || ask_size_usd < min_trade_size_usd {
                    debug!(
                        "❌ {} failed size check: bid ${:.0}, ask ${:.0} < ${:.0}",
                        pair.symbol, bid_size_usd, ask_size_usd, min_trade_size_usd
                    );
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
        let mut prices = Vec::with_capacity(3);

        // Use a reasonable test amount (10% of balance or $100 equivalent)
        let test_amount = (initial_amount * 0.1).min(100.0).max(1.0);
        let mut current_amount = test_amount;

        // Simulate the trades through the triangle using realistic bid/ask prices
        for (i, pair) in pairs.iter().enumerate() {
            let from_currency = &path[i];

            // Determine if we're buying or selling and use appropriate price
            let (amount_after_trade, _effective_price) = if pair.base == *from_currency {
                // Selling base for quote (from_currency/to_currency)
                // When selling, we get the bid price (what market makers will pay us)
                if pair.bid_price <= 0.0 {
                    return None; // Invalid price
                }
                let received = current_amount * pair.bid_price;
                prices.push(pair.bid_price);
                (received, pair.bid_price)
            } else {
                // Buying base with quote (to_currency/from_currency)
                // When buying, we pay the ask price (what market makers will sell for)
                if pair.ask_price <= 0.0 {
                    return None; // Invalid price
                }
                let received = current_amount / pair.ask_price;
                prices.push(pair.ask_price);
                (received, pair.ask_price)
            };

            // Apply trading fee (typically 0.1% for Bybit)
            current_amount = amount_after_trade * (1.0 - self.trading_fee_rate);
        }

        // Calculate profit with additional slippage buffer
        let profit_amount = current_amount - test_amount;
        let profit_pct = (profit_amount / test_amount) * 100.0;

        // Apply realistic slippage penalty (0.05% per trade = 0.15% total for 3 trades)
        let slippage_penalty = 0.15;
        let profit_pct_with_slippage = profit_pct - slippage_penalty;

        // Estimate profit in USD (assuming USDT ≈ USD)
        let estimated_usd_profit =
            if triangle.base_currency == "USDT" || triangle.base_currency == "USDC" {
                (profit_amount - (test_amount * slippage_penalty / 100.0))
                    * (initial_amount / test_amount)
            } else {
                // For non-USD base currencies, we'd need price conversion
                // For now, use a conservative estimate
                (profit_amount - (test_amount * slippage_penalty / 100.0))
                    * 0.5
                    * (initial_amount / test_amount)
            };

        if profit_pct_with_slippage > -50.0 && profit_pct_with_slippage.is_finite() {
            // Sanity check: Filter out unrealistic profits (> 100%) which usually indicate bad data
            if profit_pct_with_slippage > 100.0 {
                debug!(
                    "⚠️ Filtered out unrealistic profit: {:.2}% (Path: {})",
                    profit_pct_with_slippage,
                    path.join("->")
                );
                return None;
            }

            // Only return reasonable profit calculations
            // Optimization: Only clone strings if we are actually returning an opportunity
            let pair_symbols = vec![
                triangle.pair1.symbol.clone(),
                triangle.pair2.symbol.clone(),
                triangle.pair3.symbol.clone(),
            ];

            let opportunity = ArbitrageOpportunity {
                path: path.clone(),
                pairs: pair_symbols,
                prices,
                estimated_profit_pct: profit_pct_with_slippage,
                estimated_profit_usd: estimated_usd_profit,
                timestamp: Utc::now(),
            };

            // Return any profitable opportunity (threshold handled in main)
            Some(opportunity)
        } else {
            None
        }
    }

    /// Get opportunities above a certain profit threshold
    pub fn get_profitable_opportunities(&self, min_profit_pct: f64) -> Vec<&ArbitrageOpportunity> {
        self.opportunities
            .iter()
            .filter(|opp| opp.estimated_profit_pct >= min_profit_pct)
            .collect()
    }

    /// Get arbitrage statistics
    pub fn get_statistics(&self) -> ArbitrageStatistics {
        if self.opportunities.is_empty() {
            return ArbitrageStatistics::default();
        }

        let total_opportunities = self.opportunities.len();
        let profitable_count = self.get_profitable_opportunities(0.0).len();

        let max_profit = self
            .opportunities
            .iter()
            .map(|opp| opp.estimated_profit_pct)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let avg_profit = if total_opportunities > 0 {
            self.opportunities
                .iter()
                .map(|opp| opp.estimated_profit_pct)
                .sum::<f64>()
                / total_opportunities as f64
        } else {
            0.0
        };

        let total_estimated_usd = self
            .opportunities
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
    fn test_statistics() {
        let engine = ArbitrageEngine::new();
        let stats = engine.get_statistics();
        assert_eq!(stats.total_opportunities, 0);
        assert_eq!(stats.profitable_count, 0);
    }
}
