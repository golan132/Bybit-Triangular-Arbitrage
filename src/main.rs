mod arbitrage;
mod balance;
mod client;
mod config;
mod logger;
mod models;
mod pairs;
mod precision;
mod trader;
mod websocket;

use anyhow::{Context, Result};
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use arbitrage::ArbitrageEngine;
use balance::BalanceManager;
use client::BybitClient;
use config::Config;
use logger::*;
use pairs::PairManager;
use precision::PrecisionManager;
use trader::ArbitrageTrader;
use websocket::BybitWebsocket;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file first so RUST_LOG is available for logger initialization
    dotenv::dotenv().ok();

    // Initialize logging
    init_logger().context("Failed to initialize logger")?;
    // Load configuration
    log_phase("init", "Loading configuration");
    let config = Config::from_env().context("Failed to load configuration")?;
    log_startup_info(config.min_profit_threshold, config.trading_fee_rate);

    // Create Bybit client
    let client = BybitClient::new(config.clone()).context("Failed to create Bybit client")?;
    log_success("Initialization", "Bybit client created successfully");

    // Initialize managers and trader
    let mut balance_manager = BalanceManager::new();
    let mut pair_manager = PairManager::new();
    let mut arbitrage_engine = ArbitrageEngine::with_config(
        config.min_profit_threshold,
        1000, // max_scan_count
        config.trading_fee_rate,
    );

    // Initialize precision manager with dynamic data from Bybit
    log_phase("init", "Fetching precision data from Bybit API");
    let mut precision_manager = PrecisionManager::new();

    // Load cached precision data if available
    if let Err(e) = precision_manager.load_cache_from_file("precision_cache.json") {
        warn!("⚠️ Failed to load precision cache: {e}");
    }

    precision_manager
        .initialize(&client)
        .await
        .context("Failed to initialize precision manager")?;
    precision_manager.print_precision_summary();

    // Display precision cache statistics
    let (total_cached, _) = precision_manager.get_cache_stats();
    info!("📊 Precision Cache: {total_cached} symbols cached");

    log_success("Initialization", "Precision data loaded successfully");

    // Create arbitrage trader (set dry_run to false for live trading)
    let dry_run = std::env::var("DRY_RUN").unwrap_or_else(|_| "true".to_string()) == "true";
    let max_trades = std::env::var("MAX_TRADES")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .unwrap_or(1);
    let min_trade_amount = config.order_size; // Order size from .env file
    let mut trader = ArbitrageTrader::new(client.clone(), dry_run, precision_manager.clone());

    if dry_run {
        info!("🧪 Running in DRY RUN mode - no actual trades will be executed");
        info!("🎯 TRADE LIMIT: Bot will execute {max_trades} trade(s) and then stop");
    } else {
        info!("🚀 Running in LIVE TRADING mode - real trades will be executed!");
        info!("🎯 TRADE LIMIT: Bot will execute {max_trades} trade(s) and then stop");
    }

    // Initial pair fetch to populate symbols
    log_phase("init", "Fetching initial trading pairs");
    pair_manager
        .update_pairs_and_prices(&client)
        .await
        .context("Failed to fetch initial pairs")?;

    // Setup WebSocket
    let (tx, mut rx) = tokio::sync::mpsc::channel(10000);

    // Optimization: Only subscribe to liquid symbols to save bandwidth and connections
    let all_symbols_count = pair_manager.get_pairs().len();
    let symbols = pair_manager.get_liquid_symbols();

    info!(
        "🔌 Optimizing WebSocket: Selected {} liquid symbols out of {} total",
        symbols.len(),
        all_symbols_count
    );

    if symbols.is_empty() {
        warn!("⚠️ No liquid symbols found! WebSocket will not subscribe to any pairs.");
    } else {
        info!(
            "🔌 Connecting to WebSocket for {} liquid symbols...",
            symbols.len()
        );

        // Split symbols into chunks of 100 to respect Bybit's connection limit
        // Bybit allows max 100 topics per connection
        const MAX_TOPICS_PER_CONNECTION: usize = 100;
        let chunks: Vec<Vec<String>> = symbols
            .chunks(MAX_TOPICS_PER_CONNECTION)
            .map(|chunk| chunk.to_vec())
            .collect();

        info!(
            "🔌 Spawning {} WebSocket connections to handle liquid symbols",
            chunks.len()
        );

        for (i, chunk) in chunks.into_iter().enumerate() {
            let tx_clone = tx.clone();
            let conn_id = i + 1;
            info!("🔌 Connection #{conn_id}: Managing {} symbols", chunk.len());
            tokio::spawn(BybitWebsocket::new(conn_id, chunk, tx_clone).run());
            // Add a small delay between connections to avoid rate limits
            sleep(Duration::from_millis(100)).await;
        }
    }

    let mut cycle_count = 0;
    let mut initial_scan_logged = false;
    let _trade_executed = false;
    let mut trades_completed = 0u32;
    let start_time = Instant::now();

    info!("🚀 Bot started. Press Ctrl+C to stop.");

    // Main application loop - will exit after reaching max trades
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!(); // Newline
                info!("🛑 Received Ctrl+C signal. Shutting down...");

                let duration = start_time.elapsed();
                info!("📊 Session Summary:");
                info!("   • Runtime: {duration:.2?}");
                info!("   • Total Cycles: {cycle_count}");
                info!("   • Trades Executed: {trades_completed}/{max_trades}");

                break;
            }
            res = run_arbitrage_cycle(&client, &mut balance_manager, &mut pair_manager, &mut arbitrage_engine, &mut trader, &precision_manager, cycle_count + 1, &mut initial_scan_logged, &mut trades_completed, max_trades, min_trade_amount, &mut rx) => {
                cycle_count += 1;
                match res {
                    Ok(should_exit) => {
                        if should_exit {
                            warn!("🎯 TRADE LIMIT REACHED ({trades_completed}/{max_trades}) - Bot stopping as requested");
                            break;
                        }
                        // Only log every 100 cycles to reduce spam
                        if cycle_count % 100 == 0 {
                            log_success("Status", &format!("Completed {cycle_count} cycles successfully (Trades: {trades_completed}/{max_trades})"));
                        }
                    }
                    Err(e) => {
                        log_error_with_context("Arbitrage Cycle", &*e);
                        log_warning("Recovery", "Continuing to next cycle after error");
                    }
                }
                // Run continuously without delay for live arbitrage scanning
                // Add a small delay to prevent overwhelming the API
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // Save precision cache on exit
    if let Err(e) = trader.get_precision_manager().auto_save_cache() {
        warn!("⚠️ Failed to save precision cache on exit: {e}");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_arbitrage_cycle(
    client: &BybitClient,
    balance_manager: &mut BalanceManager,
    pair_manager: &mut PairManager,
    arbitrage_engine: &mut ArbitrageEngine,
    trader: &mut ArbitrageTrader,
    _precision_manager: &PrecisionManager,
    cycle_count: u64,
    initial_scan_logged: &mut bool,
    trades_completed: &mut u32,
    max_trades: u32,
    min_trade_amount: f64,
    rx: &mut tokio::sync::mpsc::Receiver<crate::models::TickerInfo>,
) -> Result<bool> {
    let cycle_start = Instant::now();

    // Only log cycle start every 50 cycles to reduce spam
    if cycle_count.is_multiple_of(50) {
        info!("🔄 Cycle #{cycle_count} - Scanning for arbitrage opportunities");
    }

    // Phase 1: Update account balances
    if balance_manager.needs_refresh(config::BALANCE_REFRESH_INTERVAL_SECS) {
        if cycle_count.is_multiple_of(10) {
            log_phase("balance", "Refreshing account balances");
        }
        let balance_start = Instant::now();

        balance_manager
            .update_balances(client)
            .await
            .context("Failed to update balances")?;

        // Log initial scanning info only once after first balance update
        if !*initial_scan_logged {
            balance_manager.log_initial_scanning_info_with_min_amount(min_trade_amount);
            *initial_scan_logged = true;
        }

        if cycle_count.is_multiple_of(10) {
            log_performance_metrics(
                "Balance fetch",
                balance_start.elapsed().as_millis() as u64,
                Some(balance_manager.get_all_balances().len()),
            );

            log_balance_summary(&balance_manager.get_balance_summary());
        }
    }

    // Phase 2: Update trading pairs and prices
    // Full refresh (instruments + prices) every 2000 cycles or if empty
    let needs_full_refresh = pair_manager.get_pairs().is_empty() || cycle_count.is_multiple_of(2000);

    if needs_full_refresh {
        log_phase(
            "pairs",
            "Performing FULL refresh of trading pairs and prices (Instruments + Tickers)",
        );
        let pairs_start = Instant::now();

        pair_manager
            .update_pairs_and_prices(client)
            .await
            .context("Failed to update pairs and prices")?;

        log_performance_metrics(
            "Full pairs refresh",
            pairs_start.elapsed().as_millis() as u64,
            Some(pair_manager.get_pairs().len()),
        );

        log_pair_statistics(&pair_manager.get_statistics());
    }
    // Process WebSocket updates for prices
    else {
        let mut updates_count = 0;
        while let Ok(ticker) = rx.try_recv() {
            pair_manager.update_from_ticker(&ticker);
            updates_count += 1;
        }

        if updates_count > 0 {
            if cycle_count.is_multiple_of(10) {
                info!("⚡ Processed {updates_count} WebSocket ticker updates");
            }
        } else if cycle_count.is_multiple_of(10) {
            warn!("⚠️ No WebSocket updates received in this cycle (Check connection/subscription)");
        }
    }

    // Phase 3: Scan for arbitrage opportunities
    let arbitrage_start = Instant::now();

    let opportunities = arbitrage_engine.scan_opportunities_with_min_amount(
        pair_manager,
        balance_manager,
        min_trade_amount,
    );

    // Execute profitable opportunities (only the most profitable one per cycle)
    if let Some(best_opportunity) = opportunities.first() {
        // Only log periodically to avoid spam, even for profitable ones (since we log execution separately)
        if cycle_count.is_multiple_of(10) {
            log_arbitrage_opportunity(best_opportunity, 1);
        }

        // Execute the trade if profit is above threshold and we have sufficient balance
        // AND we haven't reached the maximum number of trades yet
        if *trades_completed < max_trades && best_opportunity.estimated_profit_pct > 0.01 {
            // More than 0.01% profit
            let usdt_balance = balance_manager.get_balance("USDT");
            if usdt_balance >= min_trade_amount {
                warn!(
                    "💰 EXECUTING TRADE #{}: Found profitable opportunity {:.2}% - executing!",
                    *trades_completed + 1,
                    best_opportunity.estimated_profit_pct
                );

                match trader
                    .execute_arbitrage(best_opportunity, min_trade_amount)
                    .await
                {
                    Ok(result) => {
                        if result.success {
                            *trades_completed += 1; // Only increment on successful trades
                            warn!("✅ TRADE #{} SUCCESS!", *trades_completed);
                            warn!("✅ TRADE #{trades_completed} SUCCESS!");
                            warn!(
                                "   Realized Profit: ${:.6} ({:.2}%)",
                                result.actual_profit, result.actual_profit_pct
                            );
                            if result.dust_value_usd > 0.0 {
                                warn!("   Dust Value: ${:.6}", result.dust_value_usd);
                                let total_profit = result.actual_profit + result.dust_value_usd;
                                let total_pct = (total_profit / result.initial_amount) * 100.0;
                                warn!(
                                    "   Total Profit (inc. Dust): ${:.6} ({:.2}%)",
                                    total_profit, total_pct
                                );
                            }
                            warn!("   Execution time: {}ms", result.execution_time_ms);
                            warn!("   Total fees: ${:.6}", result.total_fees);

                            // Force balance refresh after successful trade
                            balance_manager.force_refresh();

                            // Save precision cache after successful trade
                            if let Err(e) = trader.get_precision_manager().auto_save_cache() {
                                warn!("⚠️ Failed to save precision cache: {e}");
                            }

                            if *trades_completed >= max_trades {
                                warn!(
                                    "🏁 All {max_trades} trade(s) completed successfully - stopping bot"
                                );
                                return Ok(true); // Signal to exit the main loop
                            } else {
                                warn!("⏳ Trade {trades_completed}/{max_trades} completed, continuing to look for next opportunity...");
                            }
                            let error_msg = result
                                .error_message
                                .unwrap_or_else(|| "Unknown error".to_string());
                            warn!("❌ TRADE FAILED: {error_msg}");

                            // Check if it's a recoverable error (API restrictions, etc.)
                            if error_msg.contains("170348")
                                || error_msg.contains("geographical")
                                || error_msg.contains("restricted")
                            {
                                warn!("🚫 Trade failed due to geographical/API restrictions - continuing to scan for other opportunities");
                            } else {
                                warn!("⚠️ Trade failed with different error - continuing to scan");
                            }

                            // Don't increment trade counter for failed trades - keep looking for opportunities
                            info!("🔄 Continuing to scan for other profitable opportunities...");
                        }
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        warn!("❌ Trade execution error: {error_str}");

                        // Check if it's a recoverable error
                        if error_str.contains("170348")
                            || error_str.contains("geographical")
                            || error_str.contains("restricted")
                        {
                            warn!("🚫 Trade failed due to geographical/API restrictions - continuing to scan for other opportunities");
                        } else {
                            warn!("⚠️ Trade failed with different error - continuing to scan");
                        }

                        // Don't increment trade counter for errors - keep looking for opportunities
                        info!("🔄 Continuing to scan for other profitable opportunities...");
                    }
                }
            } else if cycle_count.is_multiple_of(100) {
                info!("💸 Insufficient USDT balance for trade: ${usdt_balance:.2} (need ${min_trade_amount:.2}) - Trades: {trades_completed}/{max_trades}");
            }
        } else if *trades_completed >= max_trades {
            // All trades completed, just continue scanning until we exit
            if cycle_count.is_multiple_of(100) {
                info!("⏳ All {max_trades} trades completed - scanning for exit condition...");
            }
        }
    }

    // Show top 5 opportunities for monitoring - REMOVED to reduce spam
    // for (i, opportunity) in opportunities.iter().take(5).enumerate() {
    //     if i > 0 { // We already logged the first one above
    //         log_arbitrage_opportunity(opportunity, i + 1);
    //     }
    // }

    // Only log cycle summary every 300 cycles
    if cycle_count.is_multiple_of(crate::config::CYCLE_SUMMARY_INTERVAL as u64) {
        let cycle_duration = cycle_start.elapsed();
        log_performance_metrics(
            "Arbitrage scan",
            arbitrage_start.elapsed().as_millis() as u64,
            Some(opportunities.len()),
        );

        log_arbitrage_statistics(&arbitrage_engine.get_statistics());

        info!("📊 Cycle #{} Summary:", cycle_count);
        info!("  • Trading pairs: {}", pair_manager.get_pairs().len());
        info!("  • Total opportunities: {}", opportunities.len());
        info!("  • Cycle time: {:.2}ms", cycle_duration.as_millis());
    }

    Ok(false) // Continue running unless trade was executed
}

/// Create a sample .env file for configuration
pub fn create_sample_env_file() -> Result<()> {
    use std::fs;

    let sample_content = r#"# Bybit API Configuration
# Get your API keys from: https://www.bybit.com/app/user/api-management

# Required: Your Bybit API credentials
BYBIT_API_KEY=your_api_key_here
BYBIT_API_SECRET=your_api_secret_here

# Optional: Use testnet (default: false)
BYBIT_TESTNET=false

# Optional: Request timeout in seconds (default: 30)
REQUEST_TIMEOUT_SECS=30

# Optional: Maximum retries for failed requests (default: 3)
MAX_RETRIES=3

# Optional: Logging level (default: info)
# Options: error, warn, info, debug, trace
RUST_LOG=info
"#;

    fs::write(".env.sample", sample_content).context("Failed to create .env.sample file")?;

    info!("📋 Created .env.sample file with configuration template");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_main_modules() {
        // Test that all modules can be instantiated
        let balance_manager = BalanceManager::new();
        let pair_manager = PairManager::new();
        let arbitrage_engine = ArbitrageEngine::new();

        assert_eq!(balance_manager.get_all_balances().len(), 0);
        assert_eq!(pair_manager.get_pairs().len(), 0);
        assert_eq!(arbitrage_engine.get_opportunities().len(), 0);
    }

    #[test]
    fn test_create_sample_env() {
        let result = create_sample_env_file();
        assert!(result.is_ok());

        // Clean up
        std::fs::remove_file(".env.sample").ok();
    }
}
