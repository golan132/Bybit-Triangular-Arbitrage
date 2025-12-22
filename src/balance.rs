use crate::client::BybitClient;
use crate::models::BalanceMap;
use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, warn};

pub struct BalanceManager {
    balances: BalanceMap,
    last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl BalanceManager {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            last_updated: None,
        }
    }

    /// Fetch and update account balances
    pub async fn update_balances(&mut self, client: &BybitClient) -> Result<()> {
        info!("Updating account balances...");

        self.balances.clear();

        // Try different account types to find balances
        let account_types = vec!["UNIFIED", "SPOT", "CONTRACT"];
        
        for account_type in account_types {
            match client.get_wallet_balance(Some(account_type)).await {
                Ok(wallet_result) => {
                    debug!("Checking {} account type", account_type);
                    
                    for account in &wallet_result.list {
                        debug!("Processing account type: {}", account.account_type);
                        
                        for coin_balance in &account.coin {
                            // Try multiple balance fields
                            let balance_sources = vec![
                                &coin_balance.wallet_balance,
                                &coin_balance.available_to_withdraw,
                                &coin_balance.equity,
                            ];
                            
                            let mut found_balance = false;
                            for balance_field in balance_sources {
                                if let Ok(balance) = balance_field.parse::<f64>() {
                                    if balance > 0.0 {
                                        self.balances.insert(coin_balance.coin.clone(), balance);
                                        debug!("Added {} balance: {} = {} (from {})", 
                                               account_type, coin_balance.coin, balance, 
                                               if balance_field == &coin_balance.wallet_balance { "wallet_balance" }
                                               else if balance_field == &coin_balance.available_to_withdraw { "available_to_withdraw" }
                                               else { "equity" });
                                        found_balance = true;
                                        break;
                                    }
                                }
                            }
                            
                            if !found_balance {
                                debug!("No positive balance found for {} in {}", coin_balance.coin, account_type);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch {} balance: {}", account_type, e);
                }
            }
        }

        self.last_updated = Some(chrono::Utc::now());
        
        info!("✅ Updated balances for {} assets", self.balances.len());
        self.log_balances();

        Ok(())
    }

    /// Get balance for a specific coin
    pub fn get_balance(&self, coin: &str) -> f64 {
        self.balances.get(coin).copied().unwrap_or(0.0)
    }

    /// Get all balances
    pub fn get_all_balances(&self) -> &BalanceMap {
        &self.balances
    }

    /// Get the list of coins we have balances for
    pub fn get_available_coins(&self) -> Vec<String> {
        self.balances.keys().cloned().collect()
    }

    /// Check if balances need refresh (based on configured interval)
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

    /// Force a balance refresh on the next update cycle
    pub fn force_refresh(&mut self) {
        self.last_updated = None;
    }

    /// Log current balances for debugging
    pub fn log_balances(&self) {
        if self.balances.is_empty() {
            warn!("No balances available");
            return;
        }

        info!("Current account balances:");
        for (coin, balance) in &self.balances {
            if *balance > 0.001 {
                // Only log significant balances
                info!("  {} = {:.6}", coin, balance);
            }
        }
    }

    /// Log initial account scanning configuration with minimum trade amount filtering
    pub fn log_initial_scanning_info_with_min_amount(&self, min_trade_amount: f64) {
        let all_coins = self.get_available_coins();
        
        if all_coins.is_empty() {
            info!("🔍 Account Scanning: No balances found - will scan popular currencies");
            return;
        }

        // Filter coins that have sufficient balance for trading
        let mut sufficient_coins = Vec::new();
        let mut insufficient_coins = Vec::new();
        
        for coin in &all_coins {
            let balance = self.get_balance(coin);
            let usd_value = if coin == "USDT" || coin == "USDC" || coin == "BUSD" {
                balance // These are already in USD
            } else {
                // For other coins, we'd need price data to convert to USD
                // For now, assume we need the minimum in the coin itself
                balance
            };
            
            if usd_value >= min_trade_amount {
                sufficient_coins.push((coin.clone(), balance, usd_value));
            } else {
                insufficient_coins.push((coin.clone(), balance, usd_value));
            }
        }
        
        info!("🔍 Account Scanning: Found {} total assets, {} with sufficient balance (>${:.0})", 
              all_coins.len(), sufficient_coins.len(), min_trade_amount);
        
        if !sufficient_coins.is_empty() {
            info!("✅ Assets available for trading:");
            for (coin, balance, usd_value) in &sufficient_coins {
                info!("   {} (balance: {:.6}, ~${:.2})", coin, balance, usd_value);
            }
        }
        
        if !insufficient_coins.is_empty() {
            info!("❌ Assets with insufficient balance (below ${:.0}):", min_trade_amount);
            for (coin, balance, usd_value) in &insufficient_coins {
                info!("   {} (balance: {:.6}, ~${:.2})", coin, balance, usd_value);
            }
        }
        
        if sufficient_coins.is_empty() {
            info!("⚠️  No assets have sufficient balance for trading!");
        }
    }

    /// Get coins that have sufficient balance for trading
    pub fn get_tradeable_coins(&self, min_trade_amount: f64) -> Vec<String> {
        self.balances
            .iter()
            .filter_map(|(coin, &balance)| {
                let usd_value = if coin == "USDT" || coin == "USDC" || coin == "BUSD" {
                    balance // These are already in USD
                } else {
                    // For other coins, assume we need the minimum in the coin itself
                    balance
                };
                
                if usd_value >= min_trade_amount {
                    Some(coin.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Filter balances above a minimum threshold
    pub fn get_significant_balances(&self, min_threshold: f64) -> BalanceMap {
        self.balances
            .iter()
            .filter(|(_, &balance)| balance >= min_threshold)
            .map(|(coin, &balance)| (coin.clone(), balance))
            .collect()
    }

    /// Get balance summary statistics
    pub fn get_balance_summary(&self) -> BalanceSummary {
        let total_coins = self.balances.len();
        let significant_balances = self.get_significant_balances(0.001).len();
        let largest_balance = self.balances
            .values()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or(0.0);

        BalanceSummary {
            total_coins,
            significant_balances,
            largest_balance,
            last_updated: self.last_updated,
        }
    }
}

impl Default for BalanceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BalanceSummary {
    pub total_coins: usize,
    pub significant_balances: usize,
    pub largest_balance: f64,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl BalanceSummary {
    pub fn display(&self) -> String {
        let last_update = match self.last_updated {
            Some(dt) => dt.format("%H:%M:%S UTC").to_string(),
            None => "Never".to_string(),
        };

        format!(
            "Balances: {} total coins, {} significant, largest: {:.6}, updated: {}",
            self.total_coins, self.significant_balances, self.largest_balance, last_update
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CoinBalance;

    fn create_test_coin_balance(coin: &str, available: &str) -> CoinBalance {
        CoinBalance {
            available_to_borrow: None,
            bonus: "0".to_string(),
            accrued_interest: "0".to_string(),
            available_to_withdraw: available.to_string(),
            total_order_im: "0".to_string(),
            equity: available.to_string(),
            total_position_mm: "0".to_string(),
            usd_value: "0".to_string(),
            unrealised_pnl: "0".to_string(),
            collateral_switch: None,
            spot_hedging_qty: None,
            borrow_amount: "0".to_string(),
            total_position_im: "0".to_string(),
            wallet_balance: available.to_string(),
            cum_realised_pnl: "0".to_string(),
            locked: "0".to_string(),
            margin_collateral: None,
            coin: coin.to_string(),
        }
    }

    #[test]
    fn test_balance_manager_creation() {
        let manager = BalanceManager::new();
        assert_eq!(manager.balances.len(), 0);
        assert!(manager.last_updated.is_none());
    }

    #[test]
    fn test_balance_operations() {
        let mut manager = BalanceManager::new();
        
        // Manually add balances for testing
        manager.balances.insert("BTC".to_string(), 1.5);
        manager.balances.insert("USDT".to_string(), 1000.0);
        
        assert_eq!(manager.get_balance("BTC"), 1.5);
        assert_eq!(manager.get_balance("ETH"), 0.0);
    }

    #[test]
    fn test_significant_balances() {
        let mut manager = BalanceManager::new();
        manager.balances.insert("BTC".to_string(), 1.5);
        manager.balances.insert("ETH".to_string(), 0.0005); // Below threshold
        manager.balances.insert("USDT".to_string(), 1000.0);

        let significant = manager.get_significant_balances(0.001);
        assert_eq!(significant.len(), 2);
        assert!(significant.contains_key("BTC"));
        assert!(significant.contains_key("USDT"));
        assert!(!significant.contains_key("ETH"));
    }
}
