use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    info!("🚀 Bybit Triangular Arbitrage Bot Starting...");

    Ok(())
}

/// Log configuration with runtime values
pub fn log_startup_info(config: &crate::config::Config) {
    info!(
        "📈 Bybit Triangular Arbitrage Bot v{}",
        env!("CARGO_PKG_VERSION")
    );
    info!("⚡ Powered by Rust for high-performance trading analysis");
    info!("🎯 Mode: Real Trading Analysis (No Testnet)");

    // Log some configuration info (without sensitive data)
    info!("📋 Configuration:");
    info!(
        "  • Min Profit Threshold: {:.2}%",
        config.min_profit_threshold
    );
    info!(
        "  • Trading Fee Rate: {:.2}% per trade",
        config.trading_fee_rate * 100.0
    );
    info!(
        "  • Max Triangles to Scan: {}",
        config.max_triangles_to_scan
    );
    info!(
        "  • Balance Refresh: {}s",
        config.balance_refresh_interval_secs
    );
    info!("  • Price Refresh: {}s", config.price_refresh_interval_secs);
}

/// Log arbitrage opportunity in a formatted way
pub fn log_arbitrage_opportunity(opportunity: &crate::models::ArbitrageOpportunity, rank: usize) {
    info!(
        "[OPPORTUNITY #{}] {} | Est. Profit: {:+.2}% (${:.2})",
        rank,
        opportunity.display_path(),
        opportunity.estimated_profit_pct,
        opportunity.estimated_profit_usd
    );

    debug!("  Pairs: {}", opportunity.display_pairs());
    debug!(
        "  Prices: [{}]",
        opportunity
            .prices
            .iter()
            .map(|p| format!("{p:.8}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    debug!(
        "  Timestamp: {}",
        opportunity.timestamp.format("%H:%M:%S%.3f UTC")
    );
}

/// Log detailed arbitrage opportunity with bid/ask prices for manual verification
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

/// Log performance metrics
pub fn log_performance_metrics(operation: &str, duration_ms: u64, items_processed: Option<usize>) {
    let performance_msg = match items_processed {
        Some(count) => {
            let rate = if duration_ms > 0 {
                (count as f64 / duration_ms as f64) * 1000.0
            } else {
                0.0
            };
            format!("{count} items in {duration_ms}ms ({rate:.1} items/sec)")
        }
        None => format!("completed in {duration_ms}ms"),
    };

    debug!("⚡ {}: {}", operation, performance_msg);
}

#[cfg(test)]
mod tests {}
