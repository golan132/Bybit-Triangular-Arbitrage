use crate::client::BybitClient;
use crate::models::InstrumentsInfoResult;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug, error};

#[derive(Debug, Clone)]
pub struct PrecisionInfo {
    pub symbol: String,
    pub base_coin: String,
    pub quote_coin: String,
    pub qty_precision: u32,
    pub price_precision: u32,
    pub min_order_qty: f64,
    pub max_order_qty: f64,
    pub qty_step: f64,
    pub tick_size: f64,
}

#[derive(Debug, Clone)]
pub struct PrecisionManager {
    // Map of symbol -> precision info
    symbol_precision: HashMap<String, PrecisionInfo>,
    // Map of coin -> default precision for quantity formatting
    coin_precision: HashMap<String, u32>,
    // Cache of working decimal places for each symbol (learned from successful trades)
    working_decimals_cache: HashMap<String, u32>,
}

impl PrecisionManager {
    pub fn new() -> Self {
        Self {
            symbol_precision: HashMap::new(),
            coin_precision: HashMap::new(),
            working_decimals_cache: HashMap::new(),
        }
    }

    /// Initialize precision data by fetching from Bybit API
    pub async fn initialize(&mut self, client: &BybitClient) -> Result<()> {
        info!("🔍 Fetching precision information for all trading pairs...");
        
        // Fetch spot instruments info
        let instruments = client.get_instruments_info("spot", Some(1000)).await
            .context("Failed to fetch instruments info")?;
        
        self.process_instruments_info(instruments)?;
        
        info!("✅ Precision data loaded for {} symbols and {} coins", 
              self.symbol_precision.len(), self.coin_precision.len());
        
        Ok(())
    }

    /// Process instruments info and extract precision data
    fn process_instruments_info(&mut self, instruments: InstrumentsInfoResult) -> Result<()> {
        for instrument in instruments.list {
            // Skip non-active instruments
            if instrument.status != "Trading" {
                continue;
            }

            let qty_precision = self.extract_precision_from_step(&instrument.lot_size_filter.as_ref()
                .and_then(|f| f.qty_step.as_ref()))
                .unwrap_or(8); // Default to 8 decimals if not found

            let price_precision = self.extract_precision_from_step(&instrument.price_filter.as_ref()
                .and_then(|f| f.tick_size.as_ref()))
                .unwrap_or(8); // Default to 8 decimals if not found

            let min_order_qty = instrument.lot_size_filter.as_ref()
                .map(|f| f.min_order_qty.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);

            let max_order_qty = instrument.lot_size_filter.as_ref()
                .map(|f| f.max_order_qty.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);

            let qty_step = instrument.lot_size_filter.as_ref()
                .and_then(|f| f.qty_step.as_ref())
                .map(|s| s.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);

            let tick_size = instrument.price_filter.as_ref()
                .and_then(|f| f.tick_size.as_ref())
                .map(|s| s.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);

            let precision_info = PrecisionInfo {
                symbol: instrument.symbol.clone(),
                base_coin: instrument.base_coin.clone(),
                quote_coin: instrument.quote_coin.clone(),
                qty_precision,
                price_precision,
                min_order_qty,
                max_order_qty,
                qty_step,
                tick_size,
            };

            debug!("📊 {} precision: qty={} decimals, price={} decimals, step={:.8}", 
                   instrument.symbol, qty_precision, price_precision, qty_step);

            // Store symbol precision
            self.symbol_precision.insert(instrument.symbol.clone(), precision_info);

            // Update coin precision (use the most restrictive precision found)
            let existing_base_precision = self.coin_precision.get(&instrument.base_coin).copied().unwrap_or(8);
            let existing_quote_precision = self.coin_precision.get(&instrument.quote_coin).copied().unwrap_or(8);
            
            self.coin_precision.insert(
                instrument.base_coin.clone(), 
                existing_base_precision.min(qty_precision)
            );
            self.coin_precision.insert(
                instrument.quote_coin.clone(), 
                existing_quote_precision.min(qty_precision)
            );
        }

        Ok(())
    }

    /// Extract decimal precision from step size string
    fn extract_precision_from_step(&self, step_str: &Option<&String>) -> Option<u32> {
        if let Some(step) = step_str {
            if let Ok(step_value) = step.parse::<f64>() {
                if step_value > 0.0 {
                    // Count decimal places
                    let step_str = format!("{:.10}", step_value);
                    if let Some(decimal_pos) = step_str.find('.') {
                        let decimal_part = &step_str[decimal_pos + 1..];
                        let precision = decimal_part.trim_end_matches('0').len() as u32;
                        return Some(precision);
                    }
                }
            }
        }
        None
    }

    /// Get precision info for a specific symbol
    pub fn get_symbol_precision(&self, symbol: &str) -> Option<&PrecisionInfo> {
        self.symbol_precision.get(symbol)
    }

    /// Get quantity precision for a specific coin
    pub fn get_coin_precision(&self, coin: &str) -> u32 {
        self.coin_precision.get(coin).copied().unwrap_or_else(|| {
            // Fallback to hardcoded values for known coins
            match coin {
                "NEAR" => 2,
                "BCH" => 4,
                "BTC" => 5,
                "ETH" => 6,
                "USDT" | "USDC" | "BUSD" => 8,
                _ => {
                    warn!("⚠️ Unknown coin precision for {}, using default 8 decimals", coin);
                    8
                }
            }
        })
    }

    /// Format quantity with appropriate precision for a symbol
    pub fn format_quantity_for_symbol(&self, symbol: &str, quantity: f64) -> String {
        if let Some(precision_info) = self.get_symbol_precision(symbol) {
            // Use the symbol's specific quantity precision
            format!("{:.prec$}", quantity, prec = precision_info.qty_precision as usize)
        } else {
            // Fallback to coin-based precision
            let base_coin = self.extract_base_coin_from_symbol(symbol);
            let precision = self.get_coin_precision(&base_coin);
            format!("{:.prec$}", quantity, prec = precision as usize)
        }
    }

    /// Format quantity with appropriate precision for a coin
    pub fn format_quantity_for_coin(&self, coin: &str, quantity: f64) -> String {
        let precision = self.get_coin_precision(coin);
        format!("{:.prec$}", quantity, prec = precision as usize)
    }

    /// Extract base coin from symbol (rough estimation for fallback)
    fn extract_base_coin_from_symbol(&self, symbol: &str) -> String {
        // Try to match known patterns
        if symbol.ends_with("USDT") {
            symbol.trim_end_matches("USDT").to_string()
        } else if symbol.ends_with("USDC") {
            symbol.trim_end_matches("USDC").to_string()
        } else if symbol.ends_with("BTC") {
            symbol.trim_end_matches("BTC").to_string()
        } else if symbol.ends_with("ETH") {
            symbol.trim_end_matches("ETH").to_string()
        } else {
            // Return the first part if we can't determine
            symbol.chars().take(3).collect()
        }
    }

    /// Validate if quantity meets minimum requirements for symbol
    pub fn validate_quantity(&self, symbol: &str, quantity: f64) -> Result<()> {
        if let Some(precision_info) = self.get_symbol_precision(symbol) {
            if quantity < precision_info.min_order_qty {
                return Err(anyhow::anyhow!(
                    "Quantity {:.8} is below minimum {:.8} for symbol {}",
                    quantity, precision_info.min_order_qty, symbol
                ));
            }
            
            if quantity > precision_info.max_order_qty {
                return Err(anyhow::anyhow!(
                    "Quantity {:.8} exceeds maximum {:.8} for symbol {}",
                    quantity, precision_info.max_order_qty, symbol
                ));
            }
        }
        Ok(())
    }

    /// Validate if order value meets minimum requirements for symbol
    pub fn validate_order_value(&self, symbol: &str, quantity: f64, price: f64) -> Result<()> {
        let order_value = quantity * price;
        
        // Common minimum order values by quote currency
        let min_order_value = if symbol.ends_with("USDT") || symbol.ends_with("USDC") {
            5.0 // $5 minimum for USDT/USDC pairs
        } else if symbol.ends_with("BTC") {
            0.0001 // 0.0001 BTC minimum
        } else {
            1.0 // Default $1 minimum
        };
        
        if order_value < min_order_value {
            return Err(anyhow::anyhow!(
                "Order value {:.8} is below minimum {:.8} for symbol {} (qty: {:.8}, price: {:.8})",
                order_value, min_order_value, symbol, quantity, price
            ));
        }
        
        Ok(())
    }

    /// Get all loaded symbols
    pub fn get_loaded_symbols(&self) -> Vec<String> {
        self.symbol_precision.keys().cloned().collect()
    }

    /// Print precision summary for debugging
    pub fn print_precision_summary(&self) {
        info!("📊 Precision Summary:");
        info!("   Symbols loaded: {}", self.symbol_precision.len());
        info!("   Coins with precision data: {}", self.coin_precision.len());
        
        for (coin, precision) in &self.coin_precision {
            debug!("   {}: {} decimals", coin, precision);
        }
    }

    /// Format quantity with automatic precision reduction for API compatibility
    /// Starts with 6 decimals max, then reduces based on retry count
    pub fn format_quantity_with_retry(&self, symbol: &str, quantity: f64, retry_count: u32) -> String {
        // Start with maximum 6 decimals, then reduce based on retry count
        let max_decimals = (6_i32 - retry_count as i32).max(0) as u32;
        
        // For insufficient balance retries, also reduce the quantity slightly to ensure we don't hit balance limits
        let adjusted_quantity = if retry_count > 3 {
            // After 3 precision retries, start reducing quantity by 0.1% per retry to avoid balance issues
            let reduction_factor = 1.0 - (retry_count as f64 - 3.0) * 0.001; // 0.1% reduction per retry after retry 3
            let new_quantity = quantity * reduction_factor;
            tracing::info!("🔽 Reducing quantity due to balance issues: {:.8} → {:.8} ({}% reduction)", 
                         quantity, new_quantity, (1.0 - reduction_factor) * 100.0);
            new_quantity
        } else {
            quantity
        };
        
        if let Some(precision_info) = self.symbol_precision.get(symbol) {
            // Use the smaller of our calculated max_decimals or the symbol's qty_precision
            let actual_decimals = max_decimals.min(precision_info.qty_precision);
            let factor = 10_f64.powi(actual_decimals as i32);
            let truncated = (adjusted_quantity * factor).floor() / factor;
            let formatted = format!("{:.prec$}", truncated, prec = actual_decimals as usize);
            
            if retry_count > 0 {
                tracing::info!("📏 Precision retry #{} for {}: {} decimals, {:.8} → {} (factor: {})", 
                             retry_count, symbol, actual_decimals, adjusted_quantity, formatted, factor);
            }
            
            formatted
        } else {
            // Fallback: use max_decimals for unknown symbols
            let factor = 10_f64.powi(max_decimals as i32);
            let truncated = (adjusted_quantity * factor).floor() / factor;
            let formatted = format!("{:.prec$}", truncated, prec = max_decimals as usize);
            
            if retry_count > 0 {
                tracing::info!("📏 Precision retry #{} for {} (unknown symbol): {} decimals, {:.8} → {}", 
                             retry_count, symbol, max_decimals, adjusted_quantity, formatted);
            }
            
            formatted
        }
    }

    /// Cache the working decimal places for a symbol after successful trade
    pub fn cache_working_decimals(&mut self, symbol: &str, decimals: u32) {
        info!("💾 Caching working decimals for {}: {} decimals", symbol, decimals);
        self.working_decimals_cache.insert(symbol.to_string(), decimals);
    }

    /// Get cached working decimal places for a symbol
    pub fn get_cached_decimals(&self, symbol: &str) -> Option<u32> {
        self.working_decimals_cache.get(symbol).copied()
    }

    /// Format quantity using cached decimals if available, otherwise use API precision
    pub fn format_quantity_smart(&self, symbol: &str, quantity: f64) -> String {
        // First try to use cached working decimals
        if let Some(cached_decimals) = self.get_cached_decimals(symbol) {
            debug!("🎯 Using cached decimals for {}: {} decimals", symbol, cached_decimals);
            let factor = 10_f64.powi(cached_decimals as i32);
            let truncated = (quantity * factor).floor() / factor;
            return format!("{:.prec$}", truncated, prec = cached_decimals as usize);
        }

        // Fallback to regular precision logic
        if let Some(info) = self.symbol_precision.get(symbol) {
            let adjusted_quantity = quantity.max(info.min_order_qty);
            let max_decimals = info.qty_precision.min(8);
            let factor = 10_f64.powi(max_decimals as i32);
            let truncated = (adjusted_quantity * factor).floor() / factor;
            format!("{:.prec$}", truncated, prec = max_decimals as usize)
        } else {
            // Ultimate fallback
            format!("{:.6}", quantity)
        }
    }

    /// Get cache statistics for debugging
    pub fn get_cache_stats(&self) -> (usize, Vec<(String, u32)>) {
        let total_cached = self.working_decimals_cache.len();
        let mut cached_symbols: Vec<(String, u32)> = self.working_decimals_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        cached_symbols.sort_by(|a, b| a.0.cmp(&b.0));
        (total_cached, cached_symbols)
    }

    /// Save precision cache to file
    pub fn save_cache_to_file(&self, file_path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.working_decimals_cache)
            .context("Failed to serialize precision cache")?;
        fs::write(file_path, json)
            .context("Failed to write precision cache to file")?;
        info!("💾 Saved precision cache ({} symbols) to {}", self.working_decimals_cache.len(), file_path);
        Ok(())
    }

    /// Load precision cache from file
    pub fn load_cache_from_file(&mut self, file_path: &str) -> Result<()> {
        if !Path::new(file_path).exists() {
            info!("📁 No precision cache file found at {}, starting with empty cache", file_path);
            return Ok(());
        }

        let json = fs::read_to_string(file_path)
            .context("Failed to read precision cache file")?;
        let cache: HashMap<String, u32> = serde_json::from_str(&json)
            .context("Failed to deserialize precision cache")?;
        
        let loaded_count = cache.len();
        self.working_decimals_cache = cache;
        info!("📂 Loaded precision cache ({} symbols) from {}", loaded_count, file_path);
        Ok(())
    }

    /// Auto-save cache periodically or on program exit
    pub fn auto_save_cache(&self) -> Result<()> {
        self.save_cache_to_file("precision_cache.json")
    }
}
