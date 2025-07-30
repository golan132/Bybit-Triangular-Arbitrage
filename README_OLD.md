# 📈 Bybit Triangular Arbitrage Bot

A high-performance triangular arbitrage detection bot built in Rust for the Bybit cryptocurrency exchange. This bot analyzes real-time market data to identify profitable arbitrage opportunities across trading pairs.

## 🎯 Features

- **Real-time Analysis**: Continuously scans Bybit spot markets for arbitrage opportunities
- **High Performance**: Built in Rust for maximum speed and efficiency
- **Comprehensive Logging**: Detailed logs of all operations and opportunities found
- **Account Integration**: Fetches real account balances to determine available capital
- **Risk Aware**: Considers trading fees, minimum order sizes, and balance constraints
- **Production Ready**: Robust error handling and retry mechanisms

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ installed ([rustup.rs](https://rustup.rs/))
- Bybit account with API access
- API key with spot trading permissions

### Installation

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd bybit-test2
   ```

2. **Set up environment variables**:
   ```bash
   cp .env.sample .env
   ```
   
   Edit `.env` with your Bybit API credentials:
   ```env
   BYBIT_API_KEY=your_api_key_here
   BYBIT_API_SECRET=your_api_secret_here
   BYBIT_TESTNET=false
   RUST_LOG=info
   ```

3. **Run the bot**:
   ```bash
   cargo run
   ```

## 📋 Configuration

The bot can be configured through environment variables:

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `BYBIT_API_KEY` | Your Bybit API key | - | ✅ |
| `BYBIT_API_SECRET` | Your Bybit API secret | - | ✅ |
| `BYBIT_TESTNET` | Use testnet environment | `false` | ❌ |
| `REQUEST_TIMEOUT_SECS` | API request timeout | `30` | ❌ |
| `MAX_RETRIES` | Max retry attempts | `3` | ❌ |
| `RUST_LOG` | Logging level | `info` | ❌ |

## 🔧 How It Works

### 1. **Data Collection**
- Fetches account balances from Bybit API
- Retrieves all active spot trading pairs
- Gets real-time price data for all pairs

### 2. **Triangle Detection**
- Identifies potential triangular arbitrage paths
- Example: USDT → BTC → ETH → USDT
- Validates each step has sufficient liquidity

### 3. **Profit Calculation**
- Simulates trades through the triangle
- Accounts for trading fees (0.1% per trade)
- Calculates net profit percentage and USD value

### 4. **Opportunity Logging**
- Logs all profitable opportunities found
- Ranks by estimated profit percentage
- Provides detailed trade path information

## 📊 Output Example

```
🚀 Bybit Triangular Arbitrage Bot Starting...
✅ Loaded balances: 3 assets
✅ Fetched 250 trading pairs
🔍 Scanning for triangular loops...
🔁 Found 12 potential loops

[OPPORTUNITY #1] USDT → BTC → ETH → USDT | Est. Profit: +0.24% ($2.40)
[OPPORTUNITY #2] USDT → ETH → BNB → USDT | Est. Profit: +0.18% ($1.80)
[OPPORTUNITY #3] USDT → BTC → ADA → USDT | Est. Profit: +0.15% ($1.50)
```

## 🏗️ Project Structure

```
src/
├── main.rs          # Application entry point and main loop
├── config.rs        # Configuration management
├── client.rs        # Bybit API client implementation
├── models.rs        # Data structures for API responses
├── balance.rs       # Account balance management
├── pairs.rs         # Trading pair and price management
├── arbitrage.rs     # Triangular arbitrage detection engine
└── logger.rs        # Logging and output formatting
```

## 🔒 Security Notes

- **API Permissions**: Only requires spot trading read permissions
- **No Auto-Trading**: This bot only analyzes and logs opportunities
- **Rate Limiting**: Respects Bybit API rate limits
- **Secure Storage**: Keep your `.env` file private and never commit it

## 📈 Performance

- **Scanning Speed**: Analyzes 1000+ triangular combinations per second
- **Memory Usage**: Low memory footprint (~10-50MB)
- **API Efficiency**: Minimizes API calls through intelligent caching
- **Real-time Updates**: 10-second price refresh intervals

## 🛠️ Development

### Running Tests
```bash
cargo test
```

### Building for Release
```bash
cargo build --release
```

### Logging Levels
Set `RUST_LOG` to control verbosity:
- `error`: Only errors
- `warn`: Warnings and errors
- `info`: General information (recommended)
- `debug`: Detailed debugging info
- `trace`: Very verbose output

## 📚 API Documentation

This bot uses the following Bybit API endpoints:

- `GET /v5/account/wallet-balance` - Account balances
- `GET /v5/market/instruments-info` - Trading pair information
- `GET /v5/market/tickers` - Real-time price data

## 🔮 Future Enhancements

- **WebSocket Integration**: Real-time price streaming
- **Auto-Execution**: Automatic trade placement for profitable opportunities
- **Portfolio Optimization**: Advanced risk management and position sizing
- **Historical Analysis**: Backtesting and performance analytics
- **Multi-Exchange**: Support for additional cryptocurrency exchanges

## ⚠️ Disclaimer

This software is for educational and research purposes only. Cryptocurrency trading carries significant financial risk. The authors are not responsible for any financial losses incurred through the use of this software.

**Important Notes:**
- Always test thoroughly before using with real funds
- Monitor for changes in exchange APIs and fee structures
- Consider market volatility and slippage in real trading scenarios
- Arbitrage opportunities are often short-lived and may disappear quickly

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📧 Support

If you encounter any issues or have questions, please open an issue on GitHub.

---

**Built with ❤️ in Rust** | **Powered by Bybit API** | **For Educational Use** 
