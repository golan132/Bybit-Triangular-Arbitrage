# 🚀 Getting Started with Bybit Triangular Arbitrage Bot

## Prerequisites

1. **Rust Installation**: Install Rust from [rustup.rs](https://rustup.rs/)
2. **Bybit Account**: Sign up at [bybit.com](https://www.bybit.com/)
3. **API Access**: Create API keys with spot trading permissions

## Step-by-Step Setup

### 1. API Key Configuration

1. Log in to your Bybit account
2. Go to **API Management** (User Profile → API Management)
3. Create a new API key with these permissions:
   - ✅ **Spot Trading** (Read)
   - ✅ **Wallet** (Read)
   - ❌ **Derivatives Trading** (Not needed)
   - ❌ **Contract Trading** (Not needed)

### 2. Environment Setup

1. Copy the sample environment file:
   ```bash
   cp .env.sample .env
   ```

2. Edit `.env` with your real API credentials:
   ```env
   BYBIT_API_KEY=your_actual_api_key_here
   BYBIT_API_SECRET=your_actual_api_secret_here
   BYBIT_TESTNET=false
   RUST_LOG=info
   ```

   **⚠️ Security Warning**: Never commit your `.env` file to version control!

### 3. Build and Run

```bash
# Build the project
cargo build --release

# Run the bot
cargo run --release
```

Or use VS Code tasks:
- Press `Ctrl+Shift+P` (Cmd+Shift+P on Mac)
- Type "Tasks: Run Task"
- Select "Build and Run Bybit Arbitrage Bot"

## Expected Output

When running successfully, you should see:

```
🚀 Bybit Triangular Arbitrage Bot Starting...
📈 Bybit Triangular Arbitrage Bot v0.1.0
⚡ Powered by Rust for high-performance trading analysis
🎯 Mode: Real Trading Analysis (No Testnet)

🔧 INIT: Loading configuration
✅ Initialization: Bybit client created successfully

🔄 Starting new arbitrage analysis cycle
💰 BALANCE: Fetching account balances
✅ Updated balances for 3 assets
💰 Balances: 3 total coins, 3 significant, largest: 1000.000000, updated: 14:32:15 UTC

📊 PAIRS: Fetching trading pairs and prices
✅ Updated 250 trading pairs with current prices
📊 Pairs: 250 total (250 active), 125 currencies, avg price: 1.234567, updated: 14:32:18 UTC

🔍 ARBITRAGE: Scanning for triangular arbitrage opportunities
🔁 Found 12 potential arbitrage opportunities from 500 triangles scanned

🚀 Found 3 profitable opportunities:
[OPPORTUNITY #1] USDT → BTC → ETH → USDT | Est. Profit: +0.24% ($2.40)
[OPPORTUNITY #2] USDT → ETH → BNB → USDT | Est. Profit: +0.18% ($1.80)
[OPPORTUNITY #3] USDT → BTC → ADA → USDT | Est. Profit: +0.15% ($1.50)
```

## Configuration Options

| Setting | Description | Default |
|---------|-------------|---------|
| `MIN_PROFIT_THRESHOLD` | Minimum profit % to log | 0.1% |
| `MAX_TRIANGLES_TO_SCAN` | Maximum triangles per scan | 1000 |
| `BALANCE_REFRESH_INTERVAL_SECS` | Balance update frequency | 300s (5min) |
| `PRICE_REFRESH_INTERVAL_SECS` | Price update frequency | 10s |

## Troubleshooting

### ❌ "Failed to load configuration"
- Check that `.env` file exists and contains valid API keys
- Verify API key permissions in Bybit dashboard

### ❌ "HTTP error 403"
- Check API key is active and not expired
- Verify IP whitelist settings in Bybit (if configured)
- Ensure spot trading permissions are enabled

### ❌ "No profitable opportunities found"
- This is normal! Arbitrage opportunities are rare
- Markets are generally efficient
- Try running during high volatility periods

### ❌ Build errors
- Ensure Rust is up to date: `rustup update`
- Clean build cache: `cargo clean && cargo build`

## Next Steps

1. **Monitor Performance**: Watch the logs to understand market patterns
2. **Analyze Results**: Use the profit estimates to understand market efficiency
3. **Extend Functionality**: Consider adding WebSocket for real-time data
4. **Risk Management**: Implement position sizing and risk controls

## Important Disclaimers

- **Paper Trading Only**: This bot only analyzes and logs opportunities
- **No Guarantees**: Past arbitrage opportunities don't guarantee future profits
- **Market Risk**: Cryptocurrency markets are highly volatile
- **Technical Risk**: Use at your own risk; thoroughly test before any modifications

## Support

If you encounter issues:

1. Check the troubleshooting section above
2. Review logs for specific error messages
3. Open an issue on GitHub with:
   - Operating system
   - Rust version (`rustc --version`)
   - Error logs (remove API keys!)

Happy arbitrage hunting! 🎯📈
