# Solana DEX-CEX Arbitrage Watcher

(Coming soon)...

## Supported Providers

The application supports the following premium RPC providers:

- **Helius** - High-performance Solana RPC with enhanced features
- **QuickNode** - Enterprise-grade blockchain infrastructure
- **Alchemy** - Scalable Web3 infrastructure platform
- **GenesisGo (Triton)** - Premium Solana RPC services

## Configuration Methods

### Method 1: Environment Variables (Recommended)

Set environment variables for your API keys:

```bash
export HELIUS_API_KEY="your-helius-api-key-here"
export QUICKNODE_API_KEY="your-quicknode-token-here"
export ALCHEMY_API_KEY="your-alchemy-api-key-here"
export GENESISGO_API_KEY="your-genesisgo-token-here"
```

Then run the application normally:

```bash
cargo run -- --pair sol-usdt --threshold 1.0
```

### Method 2: CLI Arguments

Pass API keys directly via command line:

```bash
cargo run -- --pair sol-usdt --threshold 1.0 \
  --helius-api-key "your-helius-key" \
  --alchemy-api-key "your-alchemy-key"
```
