use clap::Parser;
use url::Url;

/// Application configuration from CLI args and environment
#[derive(Parser, Debug)]
#[command(name = "solana-arbitrage-watcher")]
pub struct Config {
    /// Trading pair to monitor
    #[arg(long, value_enum)]
    pub pair: TradingPair,

    /// Minimum profit threshold percentage
    #[arg(long, default_value = "0.1")]
    pub threshold: f64,

    /// Solana RPC WebSocket URL
    #[arg(long, env = "SOLANA_RPC_URL")]
    pub rpc_url: Option<Url>,
}

/// Supported trading pairs for arbitrage monitoring
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum TradingPair {
    SolUsdt,
    SolUsdc,
}

/// RPC provider configuration with failover support
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RpcProvider {
    pub name: String,
    pub websocket_url: Url,
    pub priority: u8,
}

impl Config {
    /// Get prioritized list of RPC providers based on configuration
    pub fn get_rpc_providers(&self) -> Vec<RpcProvider> {
        if let Some(ref custom_url) = self.rpc_url {
            vec![RpcProvider {
                name: "Custom".to_string(),
                websocket_url: custom_url.clone(),
                priority: 1,
            }]
        } else {
            Self::get_default_providers()
        }
    }

    /// Get default public RPC providers
    fn get_default_providers() -> Vec<RpcProvider> {
        vec![
            RpcProvider {
                name: "Helius".to_string(),
                websocket_url: "wss://mainnet.helius-rpc.com"
                    .parse()
                    .expect("Invalid default RPC URL"),
                priority: 1,
            },
            RpcProvider {
                name: "QuickNode".to_string(),
                websocket_url: "wss://mainnet.solana.com"
                    .parse()
                    .expect("Invalid default RPC URL"),
                priority: 2,
            },
        ]
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.threshold < 0.0 || self.threshold > 100.0 {
            return Err(ConfigError::InvalidThreshold(self.threshold));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid threshold: {0}. Must be between 0.0 and 100.0")]
    InvalidThreshold(f64),
}
