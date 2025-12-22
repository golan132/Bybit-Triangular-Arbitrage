use crate::client::BybitClient;
use crate::models::{PlaceOrderRequest, PlaceOrderResult, OrderInfo, ArbitrageOpportunity};
use crate::precision::PrecisionManager;
use anyhow::{Result, Context};
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use std::collections::HashMap;

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
    pub executed_value: f64,
    pub fee: f64,
}

#[derive(Debug, Clone)]
pub struct ArbitrageExecutionResult {
    pub success: bool,
    pub initial_amount: f64,
    pub final_amount: f64,
    pub actual_profit: f64,
    pub actual_profit_pct: f64,
    pub dust_value_usd: f64,
    pub dust_assets: HashMap<String, f64>,
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
    /// Cache for currency pair mappings: "FROMUPTO" -> (symbol, action)
    /// e.g., "USDCUSDT" -> ("USDCUSDT", "SELL"), "USDTUSDC" -> ("USDCUSDT", "BUY")
    symbol_map: HashMap<String, (String, String)>,
}

impl ArbitrageTrader {
    pub fn new(client: BybitClient, dry_run: bool, precision_manager: PrecisionManager) -> Self {
        let mut trader = Self {
            client,
            dry_run,
            max_order_wait_time: Duration::from_secs(30),
            precision_manager,
            symbol_map: HashMap::new(),
        };
        
        // Initialize symbol mapping cache
        trader.build_symbol_map();
        trader
    }

    /// Build the symbol mapping cache for efficient lookups
    /// Maps "FROM+TO" -> (symbol, action) for all available trading pairs
    fn build_symbol_map(&mut self) {
        info!("🗺️ Building symbol mapping cache...");
        let mut mappings = 0;
        
        // Get all available symbols from precision manager
        for (symbol, precision_info) in self.precision_manager.get_all_symbols() {
            let base = &precision_info.base_coin;
            let quote = &precision_info.quote_coin;
            
            // Example: For symbol ETHUSDT (base=ETH, quote=USDT):
            // - Converting ETH → USDT: key "ETHUSDT" → (ETHUSDT, Sell) - sell ETH to get USDT
            // - Converting USDT → ETH: key "USDTETH" → (ETHUSDT, Buy) - buy ETH using USDT
            
            // Map for direct conversion: FROM(base) -> TO(quote) = Sell base
            let direct_key = format!("{}{}", base, quote);
            self.symbol_map.insert(direct_key.clone(), (symbol.clone(), "Sell".to_string()));
            
            // Map for reverse conversion: FROM(quote) -> TO(base) = Buy base  
            let reverse_key = format!("{}{}", quote, base);
            self.symbol_map.insert(reverse_key.clone(), (symbol.clone(), "Buy".to_string()));
            
            mappings += 2;
            debug!("📊 Mapped {}: {} → Sell {}, {} → Buy {}", 
                   symbol, direct_key, base, reverse_key, base);
        }
        
        info!("✅ Symbol mapping complete: {} mappings for {} symbols", 
              mappings, mappings / 2);
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
        let mut dust_assets: HashMap<String, f64> = HashMap::new();
        let mut dust_value_usd = 0.0;

        // Execute each step of the arbitrage
        for (step, pair_symbol) in opportunity.pairs.iter().enumerate() {
            // Check if execution is taking too long (abort after 10 seconds to prevent stale prices)
            if start_time.elapsed() > Duration::from_secs(10) {
                error!("❌ Aborting arbitrage: execution time exceeded 10 seconds (current: {}ms)", 
                       start_time.elapsed().as_millis());
                return Ok(ArbitrageExecutionResult {
                    success: false,
                    initial_amount: amount,
                    final_amount: current_amount,
                    actual_profit: current_amount - amount,
                    actual_profit_pct: ((current_amount - amount) / amount) * 100.0,
                    dust_value_usd,
                    dust_assets,
                    executions,
                    total_fees,
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    error_message: Some("Execution timeout - market conditions may have changed".to_string()),
                });
            }
            
            // For steps 2 and 3, verify we have the balance from the previous step
            if step > 0 {
                self.wait_for_balance_settlement(step + 1, opportunity).await?;
            }
            
            // Use the actual amount we have from the previous step
            let trade_amount = current_amount;
            
            match self.execute_trade_step(step + 1, pair_symbol, trade_amount, &opportunity).await {
                Ok(execution) => {
                    // Calculate dust (unused balance)
                    let used_amount = if execution.side == "Buy" {
                        execution.executed_value // Quote currency used
                    } else {
                        execution.executed_quantity // Base currency used
                    };
                    
                    let dust = trade_amount - used_amount;
                    if dust > 0.00000001 { // Ignore tiny floating point errors
                        let currency = &opportunity.path[step];
                        *dust_assets.entry(currency.clone()).or_insert(0.0) += dust;
                        
                        // Estimate USD value of dust
                        let estimated_value = if step == 0 {
                            // Dust is in start currency (e.g. USDT)
                            dust
                        } else if step == 2 {
                            // Dust is in 3rd currency (e.g. MET), about to be converted to start (USDT)
                            // Step 3 trade is MET -> USDT. 
                            if execution.side == "Sell" {
                                dust * execution.executed_price
                            } else {
                                dust / execution.executed_price
                            }
                        } else {
                            // Step 2 dust (e.g. USDC).
                            // Use implied price from Step 1 execution to convert to USDT
                            if let Some(prev_exec) = executions.last() {
                                if prev_exec.executed_quantity > 0.0 {
                                    // Implied rate: USDT / USDC
                                    let rate = prev_exec.executed_value / prev_exec.executed_quantity;
                                    dust * rate
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        };
                        dust_value_usd += estimated_value;
                        
                        info!("🧹 Leftover dust: {:.8} {} (≈${:.4})", dust, currency, estimated_value);
                    }

                    // For each step, calculate what amount we actually have in the target currency
                    // If we Bought (Base), we have executed_quantity
                    // If we Sold (Base), we have executed_value (Quote)
                    let received_amount = if execution.side == "Buy" {
                        execution.executed_quantity
                    } else {
                        execution.executed_value
                    };
                    
                    // Account for potential small rounding differences/fees not included in qty
                    // (Bybit fees are usually deducted from received amount)
                    let actual_received = received_amount - execution.fee;
                    
                    info!("💰 Step {}: Received {:.8} {} (Qty: {:.8}, Val: {:.8}, Fee: {:.8})", 
                          step + 1, actual_received, &opportunity.path[step + 1], 
                          execution.executed_quantity, execution.executed_value, execution.fee);
                    
                    current_amount = actual_received;
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
                        dust_value_usd,
                        dust_assets,
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
        let total_profit_with_dust = actual_profit + dust_value_usd;
        let total_profit_pct_with_dust = (total_profit_with_dust / amount) * 100.0;

        info!("🎯 ARBITRAGE COMPLETED!");
        info!("   Initial: ${:.6} → Final: ${:.6}", amount, current_amount);
        info!("   Realized Profit: ${:.6} ({:.2}%)", actual_profit, actual_profit_pct);
        if dust_value_usd > 0.0 {
            info!("   Dust Value: ${:.6}", dust_value_usd);
            info!("   Total Profit (inc. Dust): ${:.6} ({:.2}%)", total_profit_with_dust, total_profit_pct_with_dust);
        }
        info!("   Total fees: ${:.6}", total_fees);
        info!("   Execution time: {}ms", execution_time);

        Ok(ArbitrageExecutionResult {
            success: true,
            initial_amount: amount,
            final_amount: current_amount,
            actual_profit,
            actual_profit_pct,
            dust_value_usd,
            dust_assets,
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
        let executed_value: f64 = executed_order.cum_exec_value.parse()
            .context("Failed to parse executed value")?;
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
            executed_value,
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
                        
                        // Calculate required amount based on order type
                        let required_amount = if side == "Sell" {
                            // For sell orders, we need the exact quantity of the asset
                            quantity
                        } else {
                            // For buy orders, quantity is already the quote currency amount to spend
                            quantity
                        };
                        
                        if available_balance >= required_amount {
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
                
                let (action, converted_quantity) = self.determine_trade_action(symbol, from, to, safe_quantity).await?;
                (action, converted_quantity)
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
                
                let (action, converted_quantity) = self.determine_trade_action(symbol, from, to, safe_quantity).await?;
                (action, converted_quantity)
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
    /// Implements the algorithm: if exists symbol A+B: SELL A → get B, else if exists B+A: BUY B using A
    /// Determine the correct trade action (Buy/Sell) for converting from one currency to another
    /// Uses cached symbol mapping for O(1) lookup performance
    /// Based on Bybit's symbol format: ABCXYZ where ABC=base, XYZ=quote
    async fn determine_trade_action(
        &self,
        symbol: &str,
        from_currency: &str,
        to_currency: &str,
        amount: f64,
    ) -> Result<(String, f64)> {
        info!("🧭 Converting {} → {} via {} (amount: {:.6})", 
              from_currency, to_currency, symbol, amount);

        // First, try the cached mapping approach for speed
        if let Some((mapped_symbol, action)) = self.get_action_for_conversion(from_currency, to_currency) {
            if mapped_symbol == symbol {
                let final_quantity = if action == "Buy" {
                    // For Buy orders, use the quote currency amount (amount to spend)
                    amount
                } else {
                    // For Sell orders, amount is already in base currency
                    amount
                };
                
                info!("✅ Cached mapping: {} {} on {} (final quantity: {:.8})", action, 
                      if action == "Sell" { from_currency } else { to_currency }, symbol, final_quantity);
                return Ok((action, final_quantity));
            } else {
                warn!("⚠️ Symbol mismatch: expected {}, got {} from cache", symbol, mapped_symbol);
            }
        }

        // Fallback: Get symbol information from precision manager
        let precision_info = self.precision_manager.get_symbol_precision(symbol)
            .ok_or_else(|| anyhow::anyhow!("Symbol {} not found in precision manager", symbol))?;

        let base_coin = &precision_info.base_coin;
        let quote_coin = &precision_info.quote_coin;
        
        info!("🔍 Fallback lookup - Symbol {}: base={}, quote={} | Converting {} → {}", 
              symbol, base_coin, quote_coin, from_currency, to_currency);

        // Apply the algorithm from your documentation:
        // When converting from token A to token B:
        // if exists symbol A + B (AB): action = SELL A (base) → receive B (quote)
        // else if exists symbol B + A (BA): action = BUY B (base) using A (quote)
        
        if base_coin == from_currency && quote_coin == to_currency {
            // Symbol format is FROM+TO (e.g., USDCUSDT for USDC→USDT)
            // Action: SELL from_currency (base) to get to_currency (quote)
            info!("✅ Direct pair {}: SELL {} to get {}", symbol, from_currency, to_currency);
            Ok(("Sell".to_string(), amount))
        } else if base_coin == to_currency && quote_coin == from_currency {
            // Symbol format is TO+FROM (e.g., NOTUSDC for USDC→NOT)  
            // Action: BUY to_currency (base) using from_currency (quote)
            // For Buy orders, Bybit expects the quote currency amount (amount to spend)
            info!("✅ Reverse pair {}: BUY {} using {} (spending: {:.6} {})", 
                  symbol, to_currency, from_currency, amount, from_currency);
            Ok(("Buy".to_string(), amount))
        } else {
            return Err(anyhow::anyhow!(
                "Cannot convert {} → {} using symbol {} (base: {}, quote: {})", 
                from_currency, to_currency, symbol, base_coin, quote_coin
            ));
        }
    }

    /// Convert quote currency amount to base currency quantity for Buy orders
    /// Example: Convert 25 USDC to equivalent NOT tokens based on current market price
    async fn convert_quote_to_base_quantity(&self, symbol: &str, quote_amount: f64) -> Result<f64> {
        // Get current market price
        let market_price = self.get_estimated_market_price(symbol).await
            .ok_or_else(|| anyhow::anyhow!("Failed to get market price for {}", symbol))?;
        
        // Calculate how many base tokens we can buy with the quote amount
        let base_quantity = quote_amount / market_price;
        
        // Get precision info to validate against minimum order requirements
        let precision_info = self.precision_manager.get_symbol_precision(symbol)
            .ok_or_else(|| anyhow::anyhow!("Symbol {} not found in precision manager", symbol))?;
        
        // Check if calculated quantity meets minimum order requirements
        if base_quantity < precision_info.min_order_qty {
            return Err(anyhow::anyhow!(
                "Calculated quantity {:.8} {} is below minimum {:.8} for symbol {} (spending {:.6} at price {:.8})",
                base_quantity, precision_info.base_coin, precision_info.min_order_qty, symbol, quote_amount, market_price
            ));
        }
        
        info!("💱 Conversion: {:.6} {} ÷ {:.8} = {:.8} {} (min: {:.8})", 
              quote_amount, precision_info.quote_coin, market_price, 
              base_quantity, precision_info.base_coin, precision_info.min_order_qty);
        
        Ok(base_quantity)
    }

    /// Get action for currency conversion using cached symbol mapping
    /// Returns (symbol, action) where action is "Sell" or "Buy"
    /// O(1) lookup using prebuilt HashMap - much faster than string concatenation + precision manager lookups
    fn get_action_for_conversion(&self, from: &str, to: &str) -> Option<(String, String)> {
        let key = format!("{}{}", from.to_uppercase(), to.to_uppercase());
        
        if let Some((symbol, action)) = self.symbol_map.get(&key) {
            info!("🎯 Found mapping {}: {} {} using {}", 
                  key, action, 
                  if action == "Sell" { from } else { to }, 
                  symbol);
            Some((symbol.clone(), action.clone()))
        } else {
            info!("❌ No mapping found for {} → {} (key: {})", from, to, key);
            None
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
            dust_value_usd: 0.0,
            dust_assets: HashMap::new(),
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
        const MAX_RETRIES: u32 = 4; // 0=6dec, 1=4dec, 2=2dec, 3=1dec, 4=0dec
        
        for retry_count in 0..=MAX_RETRIES {
            // Format quantity with reduced precision based on retry count
            let formatted_quantity = self.precision_manager.format_quantity_with_retry(symbol, quantity, retry_count);
            
            // Parse the formatted quantity back to f64 to ensure we use the exact truncated amount
            let actual_quantity: f64 = formatted_quantity.parse().unwrap_or(quantity);
            
            if retry_count > 0 {
                warn!("🔄 Retry #{} for {}: Reducing precision (using {:.8})", 
                      retry_count, symbol, actual_quantity);
            }
            
            // Validate the truncated quantity meets symbol requirements  
            // For Buy orders, we're using quote currency amounts, so skip base currency validations
            if side == "Sell" {
                if let Err(e) = self.precision_manager.validate_quantity(symbol, actual_quantity) {
                    return Err(anyhow::anyhow!("Quantity validation failed: {}", e));
                }
            }

            // For market orders, estimate price for order value validation
            if let Some(market_price) = self.get_estimated_market_price(symbol).await {
                // For Buy orders, the order value is the quote amount we're spending (already in quantity)
                // For Sell orders, the order value is quantity * price
                let order_value = if side == "Buy" {
                    actual_quantity // For Buy orders, quantity is already the quote currency amount
                } else {
                    actual_quantity * market_price // For Sell orders, calculate value
                };
                
                if let Err(e) = self.precision_manager.validate_order_value(symbol, order_value, 1.0) {
                    return Err(anyhow::anyhow!("Order value validation failed: {}", e));
                }
            }

            info!("📊 Using precision for {}: {:.8} (formatted: {})", symbol, actual_quantity, formatted_quantity);

            // Attempt to place the order
            match self.attempt_order_placement(symbol, side, &formatted_quantity, step).await {
                Ok(order_result) => {
                    info!("✅ Order placed successfully on attempt #{}: {}", retry_count + 1, order_result.order_id);
                    
                    // Cache the working decimal places for future use
                    let working_decimals = if let Some(pos) = formatted_quantity.find('.') {
                        (formatted_quantity.len() - pos - 1) as u32
                    } else {
                        0
                    };
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
