use crate::client::BybitClient;
use crate::models::{PlaceOrderRequest, PlaceOrderResult, OrderInfo, ArbitrageOpportunity};
use crate::precision::PrecisionManager;
use anyhow::{Result, Context};
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TradeExecution {
    pub step: usize,
    pub symbol: String,
    pub side: String,
    pub quantity: String,
    pub price: Option<String>,
    pub order_id: String,
    pub executed_price: f64,
    pub executed_quantity: f64,
    pub fee: f64,
}

#[derive(Debug, Clone)]
pub struct ArbitrageExecutionResult {
    pub success: bool,
    pub initial_amount: f64,
    pub final_amount: f64,
    pub actual_profit: f64,
    pub actual_profit_pct: f64,
    pub executions: Vec<TradeExecution>,
    pub total_fees: f64,
    pub execution_time_ms: u64,
    pub error_message: Option<String>,
}

pub struct ArbitrageTrader {
    client: BybitClient,
    dry_run: bool,
    max_order_wait_time: Duration,
    precision_manager: PrecisionManager,
}

impl ArbitrageTrader {
    pub fn new(client: BybitClient, dry_run: bool, precision_manager: PrecisionManager) -> Self {
        Self {
            client,
            dry_run,
            max_order_wait_time: Duration::from_secs(30),
            precision_manager,
        }
    }

    /// Execute a complete arbitrage opportunity
    pub async fn execute_arbitrage(&mut self, opportunity: &ArbitrageOpportunity, amount: f64) -> Result<ArbitrageExecutionResult> {
        let start_time = std::time::Instant::now();
        
        if self.dry_run {
            info!("🧪 DRY RUN: Simulating arbitrage execution");
            return self.simulate_execution(opportunity, amount);
        }

        info!("🚀 LIVE EXECUTION: Starting arbitrage trade with ${:.2}", amount);
        info!("📊 Path: {} → {} → {} → {}", 
              opportunity.path[0], opportunity.path[1], opportunity.path[2], opportunity.path[3]);

        let mut executions = Vec::new();
        let mut current_amount = amount;
        let mut total_fees = 0.0;

        // Execute each step of the arbitrage
        for (step, pair_symbol) in opportunity.pairs.iter().enumerate() {
            // For steps 2 and 3, verify we have the balance from the previous step
            if step > 0 {
                self.wait_for_balance_settlement(step + 1, opportunity).await?;
            }
            
            // Use the actual amount we have from the previous step
            let trade_amount = current_amount;
            
            match self.execute_trade_step(step + 1, pair_symbol, trade_amount, &opportunity).await {
                Ok(execution) => {
                    // For each step, calculate what amount we actually have in the target currency
                    let new_amount = match step + 1 {
                        1 => {
                            // Step 1: USDT → ICP, execution.executed_quantity is ICP we bought
                            // Account for potential small rounding differences
                            let actual_received = execution.executed_quantity * 0.999; // 0.1% buffer for fees/slippage
                            info!("💰 Step 1: Received {:.8} {}, using {:.8} for next step", 
                                  execution.executed_quantity, &opportunity.path[1], actual_received);
                            actual_received
                        },
                        2 => {
                            // Step 2: ICP → USDC, execution.executed_quantity is USDC we bought
                            let actual_received = execution.executed_quantity * 0.999; // 0.1% buffer
                            info!("💰 Step 2: Received {:.8} {}, using {:.8} for next step", 
                                  execution.executed_quantity, &opportunity.path[2], actual_received);
                            actual_received
                        },
                        3 => {
                            // Step 3: Converting back to start currency (usually USDT)
                            // For triangular arbitrage, this should give us USDT back
                            // The executed_quantity is the amount we received in the start currency
                            let final_amount = execution.executed_quantity;
                            info!("💰 Step 3: Final {} received: {:.8}", &opportunity.path[3], final_amount);
                            final_amount
                        },
                        _ => execution.executed_quantity
                    };
                    
                    info!("✅ Step {} completed: {:.6} → {:.6}", 
                          step + 1, current_amount, new_amount);
                    
                    current_amount = new_amount;
                    total_fees += execution.fee;
                    executions.push(execution);
                },
                Err(e) => {
                    let error_str = e.to_string();
                    error!("❌ Step {} failed: {}", step + 1, error_str);
                    
                    // Categorize the error for better handling
                    let error_category = if error_str.contains("170348") {
                        "Geographical/API restriction"
                    } else if error_str.contains("insufficient") || error_str.contains("balance") {
                        "Insufficient balance"
                    } else if error_str.contains("Order quantity has too many decimals") {
                        "Precision error"
                    } else if error_str.contains("timeout") {
                        "Timeout error"
                    } else {
                        "Unknown error"
                    };
                    
                    info!("🔍 Error category: {}", error_category);
                    
                    // Try to rollback previous trades if possible
                    if !executions.is_empty() {
                        warn!("🔄 Attempting to rollback previous trades...");
                        // TODO: Implement rollback logic
                    }
                    
                    return Ok(ArbitrageExecutionResult {
                        success: false,
                        initial_amount: amount,
                        final_amount: current_amount,
                        actual_profit: current_amount - amount,
                        actual_profit_pct: ((current_amount - amount) / amount) * 100.0,
                        executions,
                        total_fees,
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        error_message: Some(format!("{}: {}", error_category, error_str)),
                    });
                }
            }
        }

        let execution_time = start_time.elapsed().as_millis() as u64;
        let actual_profit = current_amount - amount;
        let actual_profit_pct = (actual_profit / amount) * 100.0;

        info!("🎯 ARBITRAGE COMPLETED!");
        info!("   Initial: ${:.6} → Final: ${:.6}", amount, current_amount);
        info!("   Profit: ${:.6} ({:.2}%)", actual_profit, actual_profit_pct);
        info!("   Total fees: ${:.6}", total_fees);
        info!("   Execution time: {}ms", execution_time);

        Ok(ArbitrageExecutionResult {
            success: true,
            initial_amount: amount,
            final_amount: current_amount,
            actual_profit,
            actual_profit_pct,
            executions,
            total_fees,
            execution_time_ms: execution_time,
            error_message: None,
        })
    }

    /// Wait for balance to be settled after previous trade
    async fn wait_for_balance_settlement(&self, step: usize, opportunity: &ArbitrageOpportunity) -> Result<()> {
        let required_currency = match step {
            2 => &opportunity.path[1], // Step 2 needs currency from step 1 (USDC)
            3 => &opportunity.path[2], // Step 3 needs currency from step 2 (ENS)
            _ => return Ok(()), // Step 1 doesn't need previous balance
        };

        let start_time = std::time::Instant::now();
        let max_wait = Duration::from_millis(5000); // Increased to 5 seconds for better settlement

        loop {
            if start_time.elapsed() > max_wait {
                warn!("⚠️ Balance settlement timeout for {} - proceeding anyway", required_currency);
                return Ok(()); // Continue anyway, let the order fail if needed
            }

            // Check if we have any balance of the required currency
            match self.client.get_wallet_balance(Some("UNIFIED")).await {
                Ok(balance_result) => {
                    if let Some(account) = balance_result.list.first() {
                        if let Some(coin_balance) = account.coin.iter().find(|c| &c.coin == required_currency) {
                            let available_balance: f64 = coin_balance.wallet_balance.parse().unwrap_or(0.0);
                            
                            if available_balance > 0.0 {
                                debug!("✅ Balance settled: {} {} available", available_balance, required_currency);
                                return Ok(());
                            }
                        }
                    }
                }
                Err(_) => {
                    // If balance check fails, just continue
                    return Ok(());
                }
            }

            sleep(Duration::from_millis(100)).await; // Check every 100ms (reduced API calls)
        }
    }

    /// Execute a single trade step
    async fn execute_trade_step(
        &mut self, 
        step: usize, 
        symbol: &str, 
        amount: f64, 
        opportunity: &ArbitrageOpportunity
    ) -> Result<TradeExecution> {
        info!("📈 Step {}: Executing trade on {}", step, symbol);

        // Determine trade direction and calculate quantity
        let (side, quantity) = self.calculate_trade_parameters(step, symbol, amount, opportunity).await?;

        // Verify we have sufficient balance before placing the order
        self.verify_balance_for_trade(step, &side, symbol, quantity, opportunity).await?;

        // Use precision manager to format quantity with automatic retry logic
        let order_result = self.place_order_with_precision_retry(symbol, &side, quantity, step).await?;

        // Wait for order execution
        let executed_order = self.wait_for_order_execution(&order_result.order_id, symbol).await
            .context("Order execution failed or timed out")?;

        let executed_price: f64 = executed_order.avg_price.parse()
            .context("Failed to parse executed price")?;
        let executed_quantity: f64 = executed_order.cum_exec_qty.parse()
            .context("Failed to parse executed quantity")?;
        let fee: f64 = executed_order.cum_exec_fee.parse()
            .context("Failed to parse execution fee")?;

        Ok(TradeExecution {
            step,
            symbol: symbol.to_string(),
            side,
            quantity: format!("{:.8}", executed_quantity), // Use executed quantity
            price: Some(format!("{:.8}", executed_price)),
            order_id: order_result.order_id,
            executed_price,
            executed_quantity,
            fee,
        })
    }

    /// Verify we have sufficient balance for the trade
    async fn verify_balance_for_trade(
        &self,
        step: usize,
        side: &str,
        symbol: &str,
        quantity: f64,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<()> {
        // Determine which currency we need to have balance for
        let required_currency = match (step, side) {
            (1, "Buy") => &opportunity.path[0],  // Step 1 Buy: need base currency (USDT)
            (1, "Sell") => &opportunity.path[1], // Step 1 Sell: need quote currency
            (2, "Buy") => &opportunity.path[1],  // Step 2 Buy: need quote currency to buy
            (2, "Sell") => &opportunity.path[1], // Step 2 Sell: need the asset we're selling
            (3, "Buy") => &opportunity.path[2],  // Step 3 Buy: need BRL to buy USDT
            (3, "Sell") => &opportunity.path[2], // Step 3 Sell: need the asset we're selling
            _ => return Err(anyhow::anyhow!("Invalid step/side combination: {}/{}", step, side)),
        };

        // Check current balance
        match self.client.get_wallet_balance(Some("UNIFIED")).await {
            Ok(balance_result) => {
                if let Some(account) = balance_result.list.first() {
                    if let Some(coin_balance) = account.coin.iter().find(|c| &c.coin == required_currency) {
                        let available_balance: f64 = coin_balance.wallet_balance.parse()
                            .unwrap_or(0.0);
                        
        // For sell orders, we need the exact quantity of the asset
        // For buy orders, we need enough of the paying currency
        let required_amount = if side == "Sell" {
            quantity
        } else {
            quantity // For market orders, this should be the spending amount
        };                        if available_balance >= required_amount {
                            info!("✅ Balance check passed: {} {} available (need {:.6})", 
                                  available_balance, required_currency, required_amount);
                            return Ok(());
                        } else {
                            return Err(anyhow::anyhow!(
                                "Insufficient {} balance: have {:.6}, need {:.6} for step {} {} on {}", 
                                required_currency, available_balance, required_amount, step, side, symbol
                            ));
                        }
                    }
                }
                
                return Err(anyhow::anyhow!(
                    "Could not find {} balance in wallet", required_currency
                ));
            },
            Err(e) => {
                warn!("Failed to check balance (continuing anyway): {}", e);
                // Continue without balance check if API fails
                return Ok(());
            }
        }
    }

    /// Calculate trade parameters for a specific step
    async fn calculate_trade_parameters(
        &self,
        step: usize,
        symbol: &str,
        amount: f64,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<(String, f64)> {
        info!("🔍 Calculating trade parameters for Step {}: {} with amount {:.6}", step, symbol, amount);
        
        // Parse the triangle path to understand trade directions
        let path = &opportunity.path;
        info!("📍 Triangle path: {} → {} → {} → {}", path[0], path[1], path[2], path[3]);
        
        // For triangular arbitrage, we need to understand each step:
        // Step 1: Convert start currency to intermediate currency 1
        // Step 2: Convert intermediate currency 1 to intermediate currency 2  
        // Step 3: Convert intermediate currency 2 back to start currency
        
        let (side, quantity) = match step {
            1 => {
                // Step 1: Convert start currency to first intermediate currency
                let from = &path[0];
                let to = &path[1];
                info!("Step 1: Converting {} to {} via {}", from, to, symbol);
                
                let (action, qty) = self.determine_trade_action(symbol, from, to, amount).await?;
                (action, qty)
            },
            2 => {
                // Step 2: Convert first intermediate to second intermediate currency
                let from = &path[1];
                let to = &path[2];
                info!("Step 2: Converting {} to {} via {}", from, to, symbol);
                
                // Use actual available balance with a more conservative buffer
                let actual_balance = self.get_actual_balance(from).await?;
                let safe_quantity = (actual_balance * 0.99).min(amount); // Use 99% of available (more conservative)
                
                info!("💰 Available {} balance: {:.8}, using: {:.8}", from, actual_balance, safe_quantity);
                
                let (action, _) = self.determine_trade_action(symbol, from, to, safe_quantity).await?;
                (action, safe_quantity)
            },
            3 => {
                // Step 3: Convert second intermediate back to start currency
                let from = &path[2];
                let to = &path[3];
                info!("Step 3: Converting {} to {} via {}", from, to, symbol);
                
                // Use actual available balance
                let actual_balance = self.get_actual_balance(from).await?;
                let safe_quantity = actual_balance * 0.99; // Use 99% of available (more conservative)
                
                info!("💰 Available {} balance: {:.8}, using: {:.8} for next step", from, actual_balance, safe_quantity);
                
                let (action, _) = self.determine_trade_action(symbol, from, to, safe_quantity).await?;
                (action, safe_quantity)
            },
            _ => {
                return Err(anyhow::anyhow!("Invalid step number: {}", step));
            }
        };
        
        info!("💡 Trade decision: {} {:.6} on {}", side, quantity, symbol);
        Ok((side, quantity))
    }

    /// Determine the correct trade action (Buy/Sell) for converting from one currency to another
    /// Based on Bybit's symbol format: ABCXYZ where ABC=base, XYZ=quote
    async fn determine_trade_action(
        &self,
        symbol: &str,
        from_currency: &str,
        to_currency: &str,
        amount: f64,
    ) -> Result<(String, f64)> {
        // Get symbol information to determine base and quote currencies
        let precision_info = self.precision_manager.get_symbol_precision(symbol)
            .ok_or_else(|| anyhow::anyhow!("Symbol {} not found in precision manager", symbol))?;

        let base_coin = &precision_info.base_coin;
        let quote_coin = &precision_info.quote_coin;
        
        info!("🧭 Symbol {}: base={}, quote={} | Converting {} → {}", 
              symbol, base_coin, quote_coin, from_currency, to_currency);

        // Apply the algorithm from the user's requirements:
        // When converting from token A to token B:
        // if exists symbol A + B (AB): action = SELL A (base) → receive B (quote)
        // else if exists symbol B + A (BA): action = BUY B (base) using A (quote)
        
        if base_coin == from_currency && quote_coin == to_currency {
            // Symbol format is FROM+TO (e.g., USDCUSDT for USDC→USDT)
            // Action: SELL from_currency (base) to get to_currency (quote)
            info!("✅ Direct pair {}: SELL {} to get {}", symbol, from_currency, to_currency);
            Ok(("Sell".to_string(), amount))
        } else if base_coin == to_currency && quote_coin == from_currency {
            // Symbol format is TO+FROM (e.g., USDCUSDT for USDT→USDC)  
            // Action: BUY to_currency (base) using from_currency (quote)
            info!("✅ Reverse pair {}: BUY {} using {}", symbol, to_currency, from_currency);
            Ok(("Buy".to_string(), amount))
        } else {
            return Err(anyhow::anyhow!(
                "Cannot convert {} → {} using symbol {} (base: {}, quote: {})", 
                from_currency, to_currency, symbol, base_coin, quote_coin
            ));
        }
    }

    /// Get actual available balance for a currency
    async fn get_actual_balance(&self, currency: &str) -> Result<f64> {
        match self.client.get_wallet_balance(Some("UNIFIED")).await {
            Ok(balance_result) => {
                if let Some(account) = balance_result.list.first() {
                    if let Some(coin_balance) = account.coin.iter().find(|c| &c.coin == currency) {
                        let balance: f64 = coin_balance.wallet_balance.parse().unwrap_or(0.0);
                        Ok(balance)
                    } else {
                        Ok(0.0)
                    }
                } else {
                    Ok(0.0)
                }
            },
            Err(e) => {
                warn!("Failed to get balance for {}: {}", currency, e);
                Ok(0.0)
            }
        }
    }

    /// Get estimated market price for order value validation
    async fn get_estimated_market_price(&self, symbol: &str) -> Option<f64> {
        // Try to get current market price from ticker
        match self.client.get_ticker("spot", symbol).await {
            Ok(ticker_result) => {
                if let Some(ticker) = ticker_result.list.first() {
                    ticker.last_price.parse::<f64>().ok()
                } else {
                    None
                }
            },
            Err(_) => {
                // Fallback: use a reasonable estimate based on common prices
                if symbol.contains("BTC") {
                    Some(50000.0) // Conservative BTC price estimate
                } else if symbol.contains("ETH") {
                    Some(3000.0) // Conservative ETH price estimate
                } else if symbol.contains("USDT") || symbol.contains("USDC") {
                    Some(1.0) // Stablecoin
                } else {
                    Some(10.0) // Default estimate for other tokens
                }
            }
        }
    }

    /// Wait for order to be executed
    async fn wait_for_order_execution(&self, order_id: &str, symbol: &str) -> Result<OrderInfo> {
        let start_time = std::time::Instant::now();
        
        loop {
            if start_time.elapsed() > self.max_order_wait_time {
                return Err(anyhow::anyhow!("Order execution timeout"));
            }

            match self.client.get_order("spot", order_id, symbol).await {
                Ok(order) => {
                    match order.order_status.as_str() {
                        "Filled" => {
                            debug!("✅ Order {} filled", order_id);
                            
                            // Quick balance verification instead of blind delay
                            info!("⚡ Verifying balance settlement...");
                            sleep(Duration::from_millis(200)).await; // Minimal delay
                            
                            return Ok(order);
                        },
                        "PartiallyFilled" => {
                            debug!("🔄 Order {} partially filled, waiting...", order_id);
                        },
                        "Cancelled" | "Rejected" => {
                            return Err(anyhow::anyhow!("Order {} was cancelled/rejected", order_id));
                        },
                        _ => {
                            debug!("⏳ Order {} status: {}", order_id, order.order_status);
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to get order status: {}", e);
                }
            }

            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Simulate execution for dry runs
    fn simulate_execution(&self, opportunity: &ArbitrageOpportunity, amount: f64) -> Result<ArbitrageExecutionResult> {
        info!("🧪 Simulating execution...");
        
        // Simulate execution with some slippage
        let slippage_factor = 0.995; // 0.5% slippage
        let simulated_final = amount * (1.0 + opportunity.estimated_profit_pct / 100.0) * slippage_factor;
        let simulated_fees = amount * 0.003; // 0.3% total fees
        let actual_profit = simulated_final - amount - simulated_fees;
        
        Ok(ArbitrageExecutionResult {
            success: true,
            initial_amount: amount,
            final_amount: simulated_final,
            actual_profit,
            actual_profit_pct: (actual_profit / amount) * 100.0,
            executions: Vec::new(), // Empty for simulation
            total_fees: simulated_fees,
            execution_time_ms: 50, // Simulated execution time
            error_message: None,
        })
    }

    /// Place order with automatic precision retry on API Error 170137 and 170148
    async fn place_order_with_precision_retry(
        &mut self, 
        symbol: &str, 
        side: &str, 
        quantity: f64, 
        step: usize
    ) -> Result<crate::models::PlaceOrderResult> {
        // First try with cached working decimals if available
        if let Some(cached_decimals) = self.precision_manager.get_cached_decimals(symbol) {
            info!("🎯 Using cached decimals for {}: {} decimals", symbol, cached_decimals);
            let formatted_quantity = self.precision_manager.format_quantity_smart(symbol, quantity);
            
            match self.attempt_order_placement(symbol, side, &formatted_quantity, step).await {
                Ok(order_result) => {
                    info!("✅ Order placed successfully using cached precision: {}", order_result.order_id);
                    return Ok(order_result);
                },
                Err(e) => {
                    let error_str = e.to_string();
                    if error_str.contains("170137") || error_str.contains("170148") || error_str.contains("too many decimals") {
                        warn!("⚠️ Cached precision failed for {}, falling back to retry logic", symbol);
                        // Continue to retry logic below
                    } else {
                        // Non-precision error, return immediately
                        return Err(e);
                    }
                }
            }
        }

        // Fallback to traditional retry logic
        const MAX_RETRIES: u32 = 6; // Start with 6 decimals, can reduce to 0
        
        for retry_count in 0..=MAX_RETRIES {
            // Format quantity with reduced precision based on retry count
            let formatted_quantity = self.precision_manager.format_quantity_with_retry(symbol, quantity, retry_count);
            
            // Parse the formatted quantity back to f64 to ensure we use the exact truncated amount
            let actual_quantity: f64 = formatted_quantity.parse().unwrap_or(quantity);
            
            if retry_count > 0 {
                warn!("🔄 Retry #{} for {}: Reducing precision to {} decimals (using {:.8})", 
                      retry_count, symbol, 6 - retry_count, actual_quantity);
            }
            
            // Validate the truncated quantity meets symbol requirements
            if let Err(e) = self.precision_manager.validate_quantity(symbol, actual_quantity) {
                return Err(anyhow::anyhow!("Quantity validation failed: {}", e));
            }

            // For market orders, estimate price for order value validation
            if let Some(market_price) = self.get_estimated_market_price(symbol).await {
                if let Err(e) = self.precision_manager.validate_order_value(symbol, actual_quantity, market_price) {
                    return Err(anyhow::anyhow!("Order value validation failed: {}", e));
                }
            }

            info!("📊 Using precision for {}: {:.8} (formatted: {})", symbol, actual_quantity, formatted_quantity);

            // Attempt to place the order
            match self.attempt_order_placement(symbol, side, &formatted_quantity, step).await {
                Ok(order_result) => {
                    info!("✅ Order placed successfully on attempt #{}: {}", retry_count + 1, order_result.order_id);
                    
                    // Cache the working decimal places for future use
                    let working_decimals = 6 - retry_count;
                    self.precision_manager.cache_working_decimals(symbol, working_decimals);
                    
                    return Ok(order_result);
                },
                Err(e) => {
                    let error_str = e.to_string();
                    
                    // Check if it's the "too many decimals" error
                    if error_str.contains("170137") || error_str.contains("too many decimals") {
                        if retry_count < MAX_RETRIES {
                            warn!("⚠️ API Error 170137 (too many decimals) on attempt #{} - retrying with fewer decimals", retry_count + 1);
                            continue; // Try again with fewer decimals
                        } else {
                            error!("❌ Failed after {} attempts - no more precision reduction possible", MAX_RETRIES + 1);
                            return Err(anyhow::anyhow!("Order placement failed after {} precision reduction attempts: {}", MAX_RETRIES + 1, error_str));
                        }
                    } else if error_str.contains("170148") || error_str.contains("Market order amount decimal too long") {
                        if retry_count < MAX_RETRIES {
                            warn!("⚠️ API Error 170148 (market order decimal too long) on attempt #{} - retrying with fewer decimals", retry_count + 1);
                            continue; // Try again with fewer decimals
                        } else {
                            error!("❌ Failed after {} attempts - no more precision reduction possible for market order", MAX_RETRIES + 1);
                            return Err(anyhow::anyhow!("Market order placement failed after {} precision reduction attempts: {}", MAX_RETRIES + 1, error_str));
                        }
                    } else if error_str.contains("170131") || error_str.contains("Insufficient balance") {
                        // For insufficient balance, try reducing the quantity a bit more
                        if retry_count < MAX_RETRIES {
                            warn!("⚠️ API Error 170131 (insufficient balance) - will retry with reduced quantity/precision");
                            continue; // Try again with more aggressive quantity reduction
                        } else {
                            error!("❌ Insufficient balance even after precision and quantity reduction");
                            return Err(anyhow::anyhow!("Order placement failed due to insufficient balance: {}", error_str));
                        }
                    } else {
                        // Different error, don't retry
                        error!("Failed to place order on {}: {}", symbol, e);
                        return Err(anyhow::anyhow!("Order placement failed: {}", error_str));
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Unexpected end of retry loop"))
    }

    /// Helper method to attempt order placement
    async fn attempt_order_placement(
        &self,
        symbol: &str,
        side: &str,
        formatted_quantity: &str,
        step: usize
    ) -> Result<crate::models::PlaceOrderResult> {
        let order_link_id = format!("arb_{}_{}", Uuid::new_v4().simple(), step);

        // Create market order for immediate execution
        let order_request = PlaceOrderRequest {
            category: "spot".to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            order_type: "Market".to_string(),
            qty: formatted_quantity.to_string(),
            price: None, // Market order
            time_in_force: Some("IOC".to_string()), // Immediate or Cancel
            order_link_id: Some(order_link_id.clone()),
            reduce_only: None,
        };

        info!("Placing {} order: {} {} @ {:?}", 
              side, formatted_quantity, symbol, order_request.price);

        self.client.place_order(order_request).await
    }

    /// Get a reference to the precision manager (for cache access)
    pub fn get_precision_manager(&self) -> &PrecisionManager {
        &self.precision_manager
    }
}
