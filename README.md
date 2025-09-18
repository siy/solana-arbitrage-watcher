# Solana DEX-CEX Arbitrage Watcher

A real-time cryptocurrency arbitrage detection system that monitors price differences between Solana DEXes (Raydium) and centralized exchanges (Binance) to identify profitable trading opportunities.

This software is provided for research and educational purposes only and does not constitute financial advice.

## Features

- **Real-time monitoring** of SOL/USDT and SOL/USDC prices
- **Dual WebSocket connections** with automatic reconnection
- **Comprehensive fee calculation** including trading fees and gas costs
- **Performance monitoring** with detailed metrics
- **Multiple output formats** (table, JSON, compact)
- **Premium RPC provider support** with API key authentication

## Prerequisites

- **Rust 1.70+** (MSRV; see `rust-version` in Cargo.toml) - Install from [rustup.rs](https://rustup.rs/)
- **Git** - For cloning the repository
- **Internet connection** - For WebSocket connections to exchanges and RPC providers

## Installation

Clone and build from source:

```bash
# Clone the repository
git clone https://github.com/siy/solana-arbitrage-watcher.git
cd solana-arbitrage-watcher

# Build the project
cargo build --release

# Run tests (optional)
cargo test
```

## Quick Start

**Important**: Public Solana RPC endpoints often throttle or limit WebSocket account/program subscriptions. Connections may succeed but updates can be delayed, showing "No fresh price data available". For reliable operation, prefer premium RPC providers with API keys.

### Basic Usage (Testing Only - Limited Functionality)

```bash
# Test application startup and WebSocket connections
cargo run --release -- --pair sol-usdt --threshold 0.5

# Test with Solana mainnet endpoint (connections work; data may be limited)
cargo run --release -- --pair sol-usdt --threshold 0.5 --rpc-url wss://api.mainnet-beta.solana.com

# Optional: devnet fallback
cargo run --release -- --pair sol-usdt --threshold 0.5 --rpc-url wss://api.devnet.solana.com
```

**Note**: These commands will establish WebSocket connections but likely show "No fresh price data available" due to public RPC account subscription limitations.

### Recommended Usage (With API Keys)

For better performance and reliability, use premium RPC providers:

#### Method 1: Environment Variables

Set environment variables for your API keys:

```bash
export HELIUS_API_KEY="your-helius-api-key-here"
export ALCHEMY_API_KEY="your-alchemy-api-key-here"
export GENESISGO_API_KEY="your-genesisgo-token-here"
#
# Note: QuickNode requires a full endpoint URL; pass it via --rpc-url (see below).
```

Tip: To avoid storing secrets in shell history, prefer exporting from a `.env` file (with your shell's dotenv loader) or use an interactive prompt:
`read -s HELIUS_API_KEY && export HELIUS_API_KEY`

Then run the application normally:

```bash
cargo run -- --pair sol-usdt --threshold 1.0
```

#### Method 2: CLI Arguments

Pass API keys directly via command line:

```bash
cargo run --release -- --pair sol-usdt --threshold 1.0 \
  --helius-api-key "your-helius-key" \
  --alchemy-api-key "your-alchemy-key"

# QuickNode example (requires full endpoint URL)
cargo run --release -- --pair sol-usdt --threshold 1.0 \
  --rpc-url "wss://<your-qn-endpoint>/<token>/"
```

## Configuration Options

### Required Parameters

- `--pair <PAIR>` - Trading pair to monitor (`sol-usdt` or `sol-usdc`)
- `--threshold <PERCENT>` - Minimum profit threshold (0.0-100.0)

### Optional Parameters

- `--output-format <FORMAT>` - Output format (`table`, `json`, `compact`) [default: `table`]
- `--max-price-age-ms <MS>` - Maximum price staleness in milliseconds [default: `5000`]
- `--min-price <PRICE>` - Minimum valid SOL price [default: `1.0`]
- `--max-price <PRICE>` - Maximum valid SOL price [default: `10000.0`]
- `--rpc-url <URL>` - Custom Solana RPC WebSocket URL

### API Key Options

- `--helius-api-key <KEY>` - Helius API key (or set `HELIUS_API_KEY`)
- QuickNode: pass your full endpoint via `--rpc-url wss://<your-endpoint>/<token>/` (env var not used)
- `--alchemy-api-key <KEY>` - Alchemy API key (or set `ALCHEMY_API_KEY`)
- `--genesisgo-api-key <KEY>` - GenesisGo API key (or set `GENESISGO_API_KEY`)

## Output Examples

### Table Format (Default)
```text
ARBITRAGE OPPORTUNITY DETECTED
==================================================
Buy Source:       Solana @ $195.4500
Sell Source:      Binance @ $197.2300
Raw Profit:       $1.7800 per unit
Net Profit:       $0.8900 per unit
Profit Margin:    0.45%
Total Fees:      $0.8900 per unit
Recommended Amount: 10.0000 SOL
Est. Total Profit: $8.9000
Detected at:      2024-01-15 14:30:22 UTC
==================================================
```

### JSON Format
```json
{
  "type": "arbitrage_opportunity",
  "trading_pair": "sol/usdt",
  "buy_source": "solana",
  "sell_source": "binance",
  "buy_price": 195.45,
  "sell_price": 197.23,
  "raw_profit_per_unit": 1.78,
  "net_profit_per_unit": 0.89,
  "profit_percentage": 0.45,
  "total_fees_per_unit": 0.89,
  "recommended_amount": 100.0,
  "estimated_total_profit": 89.0,
  "timestamp": "2024-01-15T14:30:22Z"
}
```

### Compact Format
```text
[14:30:22] SOL/USDT | Solana: $195.45 | Binance: $197.23 | Spread: 0.91% | Profit: $0.89 (0.45%)
```

## Supported RPC Providers

- **Helius** - High-performance Solana RPC with enhanced features
- **QuickNode** - Enterprise-grade blockchain infrastructure (requires full endpoint URL)
- **Alchemy** - Scalable Web3 infrastructure platform
- **GenesisGo (Triton)** - Premium Solana RPC services

## Logging

Set the `RUST_LOG` environment variable to control logging level:

```bash
# Info level (recommended)
export RUST_LOG=info
cargo run --release -- --pair sol-usdt --threshold 0.5

# Debug level (verbose)
export RUST_LOG=debug
cargo run --release -- --pair sol-usdt --threshold 0.5
```

## Troubleshooting

### Common Issues

1. **"No fresh price data available"**: Public RPC endpoints have limitations. Use premium API keys for reliable data access.
2. **Connection failures**: Ensure internet connectivity and try different RPC providers.
3. **API rate limits**: Use premium API keys for higher rate limits.
4. **Compilation errors**: Ensure Rust 1.70+ is installed (`rustc --version`).
5. **No opportunities found**: Lower the threshold or wait for market conditions.

### Performance Tips

- Use `--release` flag for optimal performance
- Set API keys for better RPC reliability
- Use `table` format for human reading, `json` for automation
- Monitor logs with `RUST_LOG=info` for system health

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
