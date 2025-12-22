use tracing::{info, warn, error, debug};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S"))
    }
}

/// Initialize the logging system
pub fn init_logger() -> Result<(), anyhow::Error> {
    // Create a custom format for logs
    let fmt_layer = fmt::layer()
        .with_timer(LocalTimer)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .compact();

    // Set up environment filter
    // Default to INFO level, but allow override via RUST_LOG env var
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    info!("🚀 Bybit Triangular Arbitrage Bot Starting...");
    
    Ok(())
}

/// Log configuration with runtime values
pub fn log_startup_info(min_profit_threshold: f64, trading_fee_rate: f64) {
    info!("📈 Bybit Triangular Arbitrage Bot v{}", env!("CARGO_PKG_VERSION"));
    info!("⚡ Powered by Rust for high-performance trading analysis");
    info!("🎯 Mode: Real Trading Analysis (No Testnet)");
    
    // Log some configuration info (without sensitive data)
    info!("📋 Configuration:");
    info!("  • Min Profit Threshold: {:.2}%", min_profit_threshold);
    info!("  • Trading Fee Rate: {:.2}% per trade", trading_fee_rate * 100.0);
    info!("  • Max Triangles to Scan: {}", crate::config::MAX_TRIANGLES_TO_SCAN);
    info!("  • Balance Refresh: {}s", crate::config::BALANCE_REFRESH_INTERVAL_SECS);
    info!("  • Price Refresh: {}s", crate::config::PRICE_REFRESH_INTERVAL_SECS);
}

/// Log arbitrage opportunity in a formatted way
pub fn log_arbitrage_opportunity(
    opportunity: &crate::models::ArbitrageOpportunity,
    rank: usize,
) {
    info!(
        "[OPPORTUNITY #{}] {} | Est. Profit: {:+.2}% (${:.2})",
        rank,
        opportunity.display_path(),
        opportunity.estimated_profit_pct,
        opportunity.estimated_profit_usd
    );
    
    debug!("  Pairs: {}", opportunity.display_pairs());
    debug!("  Prices: [{}]", 
           opportunity.prices
               .iter()
               .map(|p| format!("{:.8}", p))
               .collect::<Vec<_>>()
               .join(", "));
    debug!("  Timestamp: {}", opportunity.timestamp.format("%H:%M:%S%.3f UTC"));
}

/// Log detailed arbitrage opportunity with bid/ask prices for manual verification
pub fn log_detailed_arbitrage_opportunity(
    opportunity: &crate::models::ArbitrageOpportunity,
    trade_details: &[TradeDetail],
    rank: usize,
) {
    info!(
        "[OPPORTUNITY #{}] {} | Est. Profit: {:+.2}% (${:.2})",
        rank,
        opportunity.display_path(),
        opportunity.estimated_profit_pct,
        opportunity.estimated_profit_usd
    );
    
    // Log each trade step with detailed pricing
    for (i, detail) in trade_details.iter().enumerate() {
        info!(
            "  Step {}: {} {:.6} {} → {:.6} {} @ {:.8} ({})", 
            i + 1,
            if detail.is_sell { "SELL" } else { "BUY" },
            detail.amount_in,
            detail.from_currency,
            detail.amount_out,
            detail.to_currency,
            detail.price,
            detail.pair_symbol
        );
        info!(
            "    📊 {}: Bid {:.8} | Ask {:.8} | Used: {:.8} | Spread: {:.4}%",
            detail.pair_symbol,
            detail.bid_price,
            detail.ask_price,
            detail.price,
            ((detail.ask_price - detail.bid_price) / detail.bid_price) * 100.0
        );
    }
}

#[derive(Debug)]
pub struct TradeDetail {
    pub pair_symbol: String,
    pub from_currency: String,
    pub to_currency: String,
    pub amount_in: f64,
    pub amount_out: f64,
    pub price: f64,
    pub bid_price: f64,
    pub ask_price: f64,
    pub is_sell: bool,
}

/// Log balance information in a formatted way
pub fn log_balance_summary(summary: &crate::balance::BalanceSummary) {
    info!("💰 {}", summary.display());
}

/// Log pair statistics in a formatted way
pub fn log_pair_statistics(stats: &crate::pairs::PairStatistics) {
    info!("📊 {}", stats.display());
}

/// Log arbitrage statistics in a formatted way
pub fn log_arbitrage_statistics(stats: &crate::arbitrage::ArbitrageStatistics) {
    info!("🔍 {}", stats.display());
}

/// Log application phases with emojis
pub fn log_phase(phase: &str, message: &str) {
    let emoji = match phase {
        "init" => "🔧",
        "balance" => "💰",
        "pairs" => "📊",
        "arbitrage" => "🔍",
        "analysis" => "📈",
        "complete" => "✅",
        "error" => "❌",
        _ => "ℹ️",
    };
    
    info!("{} {}: {}", emoji, phase.to_uppercase(), message);
}

/// Log errors with context
pub fn log_error_with_context(context: &str, error: &dyn std::error::Error) {
    error!("❌ Error in {}: {}", context, error);
    
    // Log the error chain if available
    let mut source = error.source();
    let mut level = 1;
    while let Some(err) = source {
        error!("  └─ Caused by ({}): {}", level, err);
        source = err.source();
        level += 1;
        
        // Prevent infinite loops
        if level > 10 {
            error!("  └─ ... (truncated error chain)");
            break;
        }
    }
}

/// Log warnings with context
pub fn log_warning(context: &str, message: &str) {
    warn!("⚠️ {}: {}", context, message);
}

/// Log successful operations
pub fn log_success(operation: &str, details: &str) {
    info!("✅ {}: {}", operation, details);
}

/// Log rate limiting or API issues
pub fn log_api_issue(endpoint: &str, status_code: Option<u16>, message: &str) {
    match status_code {
        Some(429) => warn!("🚫 Rate limited on {}: {}", endpoint, message),
        Some(code) if code >= 500 => error!("🔥 Server error on {} ({}): {}", endpoint, code, message),
        Some(code) if code >= 400 => warn!("⚠️ Client error on {} ({}): {}", endpoint, code, message),
        _ => warn!("🌐 API issue on {}: {}", endpoint, message),
    }
}

/// Log performance metrics
pub fn log_performance_metrics(
    operation: &str,
    duration_ms: u64,
    items_processed: Option<usize>,
) {
    let performance_msg = match items_processed {
        Some(count) => {
            let rate = if duration_ms > 0 {
                (count as f64 / duration_ms as f64) * 1000.0
            } else {
                0.0
            };
            format!("{} items in {}ms ({:.1} items/sec)", count, duration_ms, rate)
        }
        None => format!("completed in {}ms", duration_ms),
    };
    
    debug!("⚡ {}: {}", operation, performance_msg);
}

/// Log trading simulation results
pub fn log_simulation_result(
    initial_amount: f64,
    final_amount: f64,
    currency: &str,
    steps: &[String],
) {
    let profit = final_amount - initial_amount;
    let profit_pct = (profit / initial_amount) * 100.0;
    
    if profit > 0.0 {
        info!("💹 Simulation: {:.6} {} → {:.6} {} ({:+.2}%)", 
              initial_amount, currency, final_amount, currency, profit_pct);
    } else {
        warn!("📉 Simulation: {:.6} {} → {:.6} {} ({:+.2}%)", 
              initial_amount, currency, final_amount, currency, profit_pct);
    }
    
    debug!("  Steps: {}", steps.join(" → "));
}

/// Log system resource usage (if available)
pub fn log_system_stats() {
    // This could be extended to log memory usage, CPU usage, etc.
    debug!("💻 System stats logging not yet implemented");
}

/// Create a progress indicator for long-running operations
pub struct ProgressLogger {
    operation: String,
    total: usize,
    current: usize,
    last_percent: usize,
}

impl ProgressLogger {
    pub fn new(operation: &str, total: usize) -> Self {
        info!("🔄 Starting {}: 0/{} (0%)", operation, total);
        
        Self {
            operation: operation.to_string(),
            total,
            current: 0,
            last_percent: 0,
        }
    }
    
    pub fn update(&mut self, current: usize) {
        self.current = current;
        let percent = if self.total > 0 {
            (current * 100) / self.total
        } else {
            100
        };
        
        // Only log every 10% to avoid spam
        if percent >= self.last_percent + 10 || current == self.total {
            info!("🔄 {}: {}/{} ({}%)", self.operation, current, self.total, percent);
            self.last_percent = percent;
        }
    }
    
    pub fn finish(&self) {
        info!("✅ Completed {}: {}/{} (100%)", self.operation, self.total, self.total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_logger() {
        let mut progress = ProgressLogger::new("Test Operation", 100);
        progress.update(25);
        progress.update(50);
        progress.update(75);
        progress.update(100);
        progress.finish();
    }
}
