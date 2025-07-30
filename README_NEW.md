# Bybit Triangular Arbitrage Bot 🚀

A high-performance Rust-based triangular arbitrage bot for Bybit cryptocurrency exchange. This bot automatically detects and executes profitable arbitrage opportunities using triangular trading patterns.

## ⚠️ Important Disclaimer

**This software is for educational and research purposes only. Trading cryptocurrencies involves substantial risk of loss and is not suitable for all investors. The authors assume no responsibility for your trading results.**

- Start with small amounts
- Use dry-run mode first
- Never invest more than you can afford to lose
- Past performance does not guarantee future results

## 🎯 Features

### Core Functionality
- **Real-time Arbitrage Detection**: Continuously scans for profitable triangular arbitrage opportunities
- **Live Trading Execution**: Automated order placement and execution via Bybit API
- **Intelligent Filtering**: Liquidity-based filtering to ensure executable opportunities
- **Risk Management**: Configurable profit thresholds and trade size limits

### Trading Features
- **Dry Run Mode**: Safe simulation mode for testing strategies
- **Live Trading Mode**: Actual order execution with real funds
- **Market Orders**: Immediate execution using market orders with IOC (Immediate or Cancel)
- **Order Management**: Real-time order tracking and status monitoring
- **Balance Integration**: Automatic balance checks and updates

### Technical Features
- **High Performance**: 100ms scan cycles for rapid opportunity detection
- **Realistic Constraints**: Volume filtering ($10K min), spread limits (5% max)
- **Geographical Compliance**: Excludes problematic tokens (USDR, BUSD, UST, etc.)
- **Comprehensive Logging**: Detailed execution logs and performance metrics

## 📋 Prerequisites

- **Rust 1.70+**: [Install Rust](https://rustup.rs/)
- **Bybit Account**: [Create account](https://www.bybit.com/)
- **API Keys**: Generate API keys with trading permissions

### API Permissions Required
- **Spot Trading**: Place and cancel orders
- **Wallet**: Read balance information
- **Read**: Access market data

## ⚙️ Installation

1. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/bybit-arbitrage-bot.git
   cd bybit-arbitrage-bot
   ```

2. **Install dependencies**:
   ```bash
   cargo build --release
   ```

3. **Configure environment**:
   ```bash
   cp .env.sample .env
   # Edit .env with your API credentials
   ```

## 🔧 Configuration

### Environment Variables

Create a `.env` file with your configuration:

```env
# Required: Your Bybit API credentials
BYBIT_API_KEY=your_api_key_here
BYBIT_API_SECRET=your_api_secret_here

# Trading Mode (IMPORTANT!)
DRY_RUN=true  # Set to false for live trading

# Optional settings
BYBIT_TESTNET=false
REQUEST_TIMEOUT_SECS=30
MAX_RETRIES=3
RUST_LOG=info
```

### Trading Parameters

The bot uses the following default parameters (configurable in `src/config.rs`):

```rust
// Profit thresholds
MIN_PROFIT_THRESHOLD: 0.01%    // Minimum profit to execute trade

// Liquidity filters
MIN_VOLUME_24H_USD: $10,000    // Minimum 24h volume
MAX_SPREAD_PERCENT: 5.0%       // Maximum bid-ask spread
MIN_ORDER_SIZE_USD: $100       // Minimum order size

// Execution settings
MIN_TRADE_AMOUNT: $100         // Minimum trade amount
```

## 🚀 Usage

### Dry Run Mode (Recommended First)

Start with simulation mode to test the bot safely:

```bash
# Set DRY_RUN=true in .env file
cargo run --release
```

The bot will:
- Detect arbitrage opportunities
- Simulate trade execution
- Show potential profits
- No real trades executed

### Live Trading Mode ⚡

**WARNING: This mode uses real money!**

```bash
# Set DRY_RUN=false in .env file
cargo run --release
```

The bot will:
- Execute real trades on Bybit
- Use your actual account balance
- Generate real profits/losses

### Example Output

```
🔍 Account Scanning: Found 3 assets in account: ["USDT", "BTC", "ETH"]
   USDT (balance: 1000.000000, test amount: 100.000000)

💰 Arbitrage Opportunity #1:
   Path: USDT → USDC → BCH → USDT
   Estimated Profit: 0.012% ($0.12 on $1000)
   Pairs: USDCUSDT, BCHUSDC, BCHUSDT

🚀 LIVE EXECUTION: Starting arbitrage trade with $100.00
✅ ARBITRAGE SUCCESS!
   Profit: $0.120000 (0.12%)
   Execution time: 2847ms
   Total fees: $0.300000
```

## 📊 Trading Logic

### Arbitrage Detection

1. **Triangle Identification**: Finds currency triangles (e.g., USDT → BTC → ETH → USDT)
2. **Price Analysis**: Compares prices across different trading pairs
3. **Profit Calculation**: Calculates potential profit after fees
4. **Liquidity Verification**: Ensures sufficient volume for execution

### Execution Process

1. **Opportunity Detection**: Identifies profitable triangle (>0.01% profit)
2. **Balance Check**: Verifies sufficient USDT balance
3. **Order Execution**: Places market orders sequentially
4. **Order Monitoring**: Tracks order status until completion
5. **Balance Update**: Refreshes account balances

### Risk Management

- **Volume Filtering**: Only trades high-volume pairs
- **Spread Limits**: Avoids pairs with excessive spreads
- **Size Restrictions**: Minimum order sizes for efficiency
- **Timeout Protection**: Cancels stuck orders after 30 seconds

## 📁 Project Structure

```
src/
├── main.rs          # Application entry point
├── arbitrage.rs     # Core arbitrage detection logic
├── trader.rs        # Trade execution engine
├── client.rs        # Bybit API client
├── models.rs        # Data structures
├── pairs.rs         # Trading pair management
├── balance.rs       # Account balance management
├── config.rs        # Configuration constants
└── logger.rs        # Logging utilities
```

## 🛡️ Safety Features

### Built-in Protections
- **Dry Run Default**: Starts in simulation mode
- **Balance Checks**: Verifies funds before trading
- **Order Timeouts**: Prevents stuck orders
- **Error Recovery**: Continues operation after errors
- **Comprehensive Logging**: Full audit trail

### Monitoring
- **Real-time Status**: Live updates on opportunities and trades
- **Performance Metrics**: Execution times and success rates
- **Balance Tracking**: Automatic balance updates
- **Error Reporting**: Detailed error messages and recovery

## ⚠️ Risk Warnings

### Market Risks
- **Volatility**: Prices can change rapidly during execution
- **Slippage**: Actual execution prices may differ from expected
- **Network Latency**: Delays can affect profitability
- **Exchange Issues**: API or exchange problems can cause losses

### Technical Risks
- **API Limits**: Rate limiting may affect performance
- **Network Connectivity**: Internet issues can disrupt trading
- **Software Bugs**: Always test thoroughly before live trading

### Financial Risks
- **Capital Loss**: You can lose money, potentially all of it
- **Fee Accumulation**: Trading fees reduce profitability
- **Insufficient Liquidity**: Large orders may not fill completely

## 🔧 Troubleshooting

### Common Issues

1. **No Opportunities Found**:
   - Check market conditions (low volatility periods have fewer opportunities)
   - Verify profit threshold settings
   - Ensure sufficient account balance

2. **API Errors**:
   - Verify API key permissions
   - Check API key/secret in .env file
   - Ensure stable internet connection

3. **Order Execution Failures**:
   - Check account balance
   - Verify trading permissions
   - Monitor for exchange maintenance

### Debug Mode

Enable detailed logging for troubleshooting:

```env
RUST_LOG=debug
```

## 📈 Performance Optimization

### Recommended Settings
- **VPS Hosting**: Use a server close to Bybit's location
- **Stable Connection**: Ensure reliable, low-latency internet
- **Sufficient Balance**: Maintain adequate funds for opportunities
- **Conservative Thresholds**: Start with higher profit requirements

### Hardware Requirements
- **CPU**: Moderate (single core sufficient)
- **Memory**: 512MB+ RAM
- **Network**: Low latency, stable connection
- **Storage**: Minimal (< 100MB)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚡ Support

- **Issues**: [GitHub Issues](https://github.com/yourusername/bybit-arbitrage-bot/issues)
- **Documentation**: This README and inline code comments
- **Community**: Discussions welcome in GitHub Discussions

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

---

**Remember: Always start with dry-run mode and small amounts. Trading involves risk!** 🛡️
