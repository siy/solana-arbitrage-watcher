use clap::Parser;
use url::Url;

/// Raw configuration from CLI args and environment (unvalidated)
#[derive(Parser, Debug)]
#[command(name = "solana-arbitrage-watcher")]
pub struct RawConfig {
    /// Trading pair to monitor
    #[arg(long, value_enum)]
    pub pair: TradingPair,

    /// Minimum profit threshold percentage
    #[arg(long, default_value = "0.1")]
    pub threshold: f64,

    /// Maximum price age in milliseconds before data is considered stale
    #[arg(long, default_value = "5000")]
    pub max_price_age_ms: u64,

    /// Solana RPC WebSocket URL
    #[arg(long, env = "SOLANA_RPC_URL")]
    pub rpc_url: Option<Url>,
}

/// Validated application configuration (always valid)
#[derive(Debug)]
pub struct Config {
    pub pair: TradingPair,
    pub threshold: ProfitThreshold,
    pub max_price_age_ms: MaxPriceAge,
    pub rpc_providers: Vec<RpcProvider>,
}

/// Validated profit threshold percentage
#[derive(Debug, Clone, Copy)]
pub struct ProfitThreshold(f64);

impl ProfitThreshold {
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Validated maximum price age in milliseconds
#[derive(Debug, Clone, Copy)]
pub struct MaxPriceAge(u64);

impl MaxPriceAge {
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Supported trading pairs for arbitrage monitoring
#[derive(Debug, Clone, Copy, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
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
    /// Parse and validate raw configuration, accumulating all errors
    pub fn new(raw: &RawConfig) -> Result<Self, ConfigErrors> {
        let mut errors = Vec::new();

        // Validate threshold
        let threshold = if raw.threshold >= 0.0 && raw.threshold <= 100.0 {
            Some(ProfitThreshold(raw.threshold))
        } else {
            errors.push(ConfigError::InvalidThreshold(raw.threshold));
            None
        };

        // Validate max price age (reasonable range: 100ms to 60s)
        let max_price_age_ms = if raw.max_price_age_ms >= 100 && raw.max_price_age_ms <= 60000 {
            Some(MaxPriceAge(raw.max_price_age_ms))
        } else {
            errors.push(ConfigError::InvalidMaxPriceAge(raw.max_price_age_ms));
            None
        };

        // Create RPC providers (no validation needed for URLs since clap already parsed them)
        let rpc_providers = Self::create_rpc_providers(&raw.rpc_url);

        // Return errors if any, otherwise return valid config
        if !errors.is_empty() {
            return Err(ConfigErrors { errors });
        }

        Ok(Config {
            pair: raw.pair,
            threshold: threshold.unwrap(), // Safe because we checked for errors above
            max_price_age_ms: max_price_age_ms.unwrap(), // Safe because we checked for errors above
            rpc_providers,
        })
    }

    /// Create RPC providers based on configuration
    fn create_rpc_providers(custom_url: &Option<Url>) -> Vec<RpcProvider> {
        if let Some(ref url) = custom_url {
            vec![RpcProvider {
                name: "Custom".to_string(),
                websocket_url: url.clone(),
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
}

/// Collection of configuration validation errors
#[derive(Debug, thiserror::Error)]
#[error("Configuration validation failed:\n{}",
    .errors.iter()
        .enumerate()
        .map(|(i, e)| format!("  {}. {}", i + 1, e))
        .collect::<Vec<_>>()
        .join("\n")
)]
pub struct ConfigErrors {
    pub errors: Vec<ConfigError>,
}

/// Individual configuration validation error
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid threshold: {0}. Must be between 0.0 and 100.0")]
    InvalidThreshold(f64),
    #[error("Invalid max price age: {0}ms. Must be between 100 and 60000 milliseconds")]
    InvalidMaxPriceAge(u64),
}
