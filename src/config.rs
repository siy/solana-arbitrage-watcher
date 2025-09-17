use crate::output::OutputFormat;
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

    /// Helius API key for premium RPC access
    #[arg(long, env = "HELIUS_API_KEY")]
    pub helius_api_key: Option<String>,

    /// QuickNode API key for premium RPC access
    #[arg(long, env = "QUICKNODE_API_KEY")]
    pub quicknode_api_key: Option<String>,

    /// Alchemy API key for premium RPC access
    #[arg(long, env = "ALCHEMY_API_KEY")]
    pub alchemy_api_key: Option<String>,

    /// GenesisGo API key for premium RPC access
    #[arg(long, env = "GENESISGO_API_KEY")]
    pub genesisgo_api_key: Option<String>,

    /// Output format for displaying results
    #[arg(long, value_enum, default_value = "table")]
    pub output_format: OutputFormat,

    /// Minimum valid price for SOL (default: 1.0)
    #[arg(long, default_value = "1.0")]
    pub min_price: f64,

    /// Maximum valid price for SOL (default: 10000.0)
    #[arg(long, default_value = "10000.0")]
    pub max_price: f64,
}

/// Validated application configuration (always valid)
#[derive(Debug)]
pub struct Config {
    pub pair: TradingPair,
    pub threshold: ProfitThreshold,
    pub max_price_age_ms: MaxPriceAge,
    pub rpc_providers: Vec<RpcProvider>,
    pub output_format: OutputFormat,
    pub price_bounds: PriceBounds,
    pub api_keys: ApiKeyConfig,
}

/// Validated price bounds for validation
#[derive(Debug, Clone, Copy)]
pub struct PriceBounds {
    pub min_price: f64,
    pub max_price: f64,
}

impl PriceBounds {
    pub fn new(min_price: f64, max_price: f64) -> Result<Self, ConfigError> {
        if !min_price.is_finite() || !max_price.is_finite() {
            return Err(ConfigError::PriceBound(format!(
                "Prices must be finite numbers, got min={} max={}",
                min_price, max_price
            )));
        }
        if min_price <= 0.0 {
            return Err(ConfigError::PriceBound(format!(
                "Minimum price must be positive, got: {}",
                min_price
            )));
        }
        if max_price <= min_price {
            return Err(ConfigError::PriceBound(format!(
                "Maximum price ({}) must be greater than minimum price ({})",
                max_price, min_price
            )));
        }
        Ok(Self {
            min_price,
            max_price,
        })
    }
}

/// Validated profit threshold percentage
#[derive(Debug, Clone, Copy)]
pub struct ProfitThreshold(f64);

impl ProfitThreshold {
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Create new ProfitThreshold for testing
    #[cfg(test)]
    pub fn new(value: f64) -> Result<Self, ConfigError> {
        if (0.0..=100.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ConfigError::Threshold(value))
        }
    }
}

/// Validated maximum price age in milliseconds
#[derive(Debug, Clone, Copy)]
pub struct MaxPriceAge(u64);

impl MaxPriceAge {
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Create new MaxPriceAge for testing
    #[cfg(test)]
    pub fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Supported trading pairs for arbitrage monitoring
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum TradingPair {
    SolUsdt,
    SolUsdc,
}

/// API key configuration for RPC providers
#[derive(Clone)]
pub struct ApiKeyConfig {
    pub helius: Option<String>,
    pub quicknode: Option<String>,
    pub alchemy: Option<String>,
    pub genesisgo: Option<String>,
}

impl ApiKeyConfig {
    /// Create from raw configuration
    pub fn from_raw(raw: &RawConfig) -> Self {
        Self {
            helius: raw.helius_api_key.clone(),
            quicknode: raw.quicknode_api_key.clone(),
            alchemy: raw.alchemy_api_key.clone(),
            genesisgo: raw.genesisgo_api_key.clone(),
        }
    }

    /// Check if any API keys are configured
    pub fn has_keys(&self) -> bool {
        self.helius.is_some()
            || self.quicknode.is_some()
            || self.alchemy.is_some()
            || self.genesisgo.is_some()
    }
}

impl std::fmt::Debug for ApiKeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyConfig")
            .field("helius", &self.helius.as_ref().map(|_| "***"))
            .field("quicknode", &self.quicknode.as_ref().map(|_| "***"))
            .field("alchemy", &self.alchemy.as_ref().map(|_| "***"))
            .field("genesisgo", &self.genesisgo.as_ref().map(|_| "***"))
            .finish()
    }
}

/// RPC provider configuration with failover support
#[derive(Clone)]
#[allow(dead_code)]
pub struct RpcProvider {
    pub name: String,
    pub websocket_url: Url,
    pub priority: u8,
    pub provider_type: RpcProviderType,
}

/// Types of RPC providers for different authentication methods
#[derive(Debug, Clone, PartialEq)]
pub enum RpcProviderType {
    Helius,
    QuickNode,
    Alchemy,
    GenesisGo,
    Custom,
    Public,
}

impl Config {
    /// Parse and validate raw configuration, accumulating all errors
    pub fn new(raw: &RawConfig) -> Result<Self, ConfigErrors> {
        let mut errors = Vec::new();

        // Validate threshold
        let threshold = if raw.threshold >= 0.0 && raw.threshold <= 100.0 {
            Some(ProfitThreshold(raw.threshold))
        } else {
            errors.push(ConfigError::Threshold(raw.threshold));
            None
        };

        // Validate max price age (reasonable range: 100ms to 60s)
        let max_price_age_ms = if raw.max_price_age_ms >= 100 && raw.max_price_age_ms <= 60000 {
            Some(MaxPriceAge(raw.max_price_age_ms))
        } else {
            errors.push(ConfigError::MaxPriceAge(raw.max_price_age_ms));
            None
        };

        // Validate price bounds
        let price_bounds = match PriceBounds::new(raw.min_price, raw.max_price) {
            Ok(bounds) => Some(bounds),
            Err(e) => {
                errors.push(e);
                None
            }
        };

        // Create API key configuration
        let api_keys = ApiKeyConfig::from_raw(raw);

        // Create RPC providers with API key support
        let rpc_providers = Self::create_rpc_providers(&raw.rpc_url, &api_keys);

        // Return errors if any, otherwise return valid config
        if !errors.is_empty() {
            return Err(ConfigErrors { errors });
        }

        Ok(Config {
            pair: raw.pair,
            threshold: threshold.unwrap(), // Safe because we checked for errors above
            max_price_age_ms: max_price_age_ms.unwrap(), // Safe because we checked for errors above
            rpc_providers,
            output_format: raw.output_format,
            price_bounds: price_bounds.unwrap(), // Safe because we checked for errors above
            api_keys,
        })
    }

    /// Create RPC providers based on configuration with API key support
    fn create_rpc_providers(custom_url: &Option<Url>, api_keys: &ApiKeyConfig) -> Vec<RpcProvider> {
        if let Some(ref url) = custom_url {
            vec![RpcProvider {
                name: "Custom".to_string(),
                websocket_url: url.clone(),
                priority: 1,
                provider_type: RpcProviderType::Custom,
            }]
        } else if api_keys.has_keys() {
            Self::get_authenticated_providers(api_keys)
        } else {
            Self::get_default_providers()
        }
    }

    /// Get authenticated RPC providers using API keys
    fn get_authenticated_providers(api_keys: &ApiKeyConfig) -> Vec<RpcProvider> {
        let mut providers = Vec::new();
        let mut priority = 1;

        // Helius (highest priority if available)
        if let Some(ref api_key) = api_keys.helius {
            if let Ok(url) = format!("wss://mainnet.helius-rpc.com/?api-key={}", api_key).parse() {
                providers.push(RpcProvider {
                    name: "Helius (Authenticated)".to_string(),
                    websocket_url: url,
                    priority,
                    provider_type: RpcProviderType::Helius,
                });
                priority += 1;
            }
        }

        // QuickNode requires a full endpoint URL. Do not synthesize from tokens.
        // Users should pass --rpc-url wss://<their-quicknode-endpoint>/<token>/ to use QuickNode.

        // Alchemy
        if let Some(ref api_key) = api_keys.alchemy {
            if let Ok(url) = format!("wss://solana-mainnet.g.alchemy.com/v2/{}", api_key).parse() {
                providers.push(RpcProvider {
                    name: "Alchemy (Authenticated)".to_string(),
                    websocket_url: url,
                    priority,
                    provider_type: RpcProviderType::Alchemy,
                });
                priority += 1;
            }
        }

        // GenesisGo (Triton/Shadow)
        if let Some(ref api_key) = api_keys.genesisgo {
            if let Ok(url) = format!("wss://triton.genesysgo.net/{}", api_key).parse() {
                providers.push(RpcProvider {
                    name: "GenesisGo Triton (Authenticated)".to_string(),
                    websocket_url: url,
                    priority,
                    provider_type: RpcProviderType::GenesisGo,
                });
            }
        }

        // Fallback to public providers if no authenticated providers were created
        if providers.is_empty() {
            providers = Self::get_default_providers();
        }

        providers
    }

    /// Get default public RPC providers as fallback
    fn get_default_providers() -> Vec<RpcProvider> {
        vec![
            RpcProvider {
                name: "Public Solana (Limited)".to_string(),
                websocket_url: "wss://api.mainnet-beta.solana.com/"
                    .parse()
                    .expect("Invalid default RPC URL"),
                priority: 1,
                provider_type: RpcProviderType::Public,
            },
            RpcProvider {
                name: "Solana Devnet (Fallback)".to_string(),
                websocket_url: "wss://api.devnet.solana.com/"
                    .parse()
                    .expect("Invalid default RPC URL"),
                priority: 2,
                provider_type: RpcProviderType::Public,
            },
        ]
    }
}

impl std::fmt::Debug for RpcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn redact_url(url: &Url, provider_type: &RpcProviderType) -> String {
            let mut s = url.as_str().to_string();
            // Redact api-key query parameters
            if let Some(idx) = s.find("api-key=") {
                let start = idx + "api-key=".len();
                let end = s[start..].find('&').map(|i| start + i).unwrap_or(s.len());
                s.replace_range(start..end, "***");
                return s;
            }
            // Redact path segment after /v2/
            if s.contains("/v2/") {
                if let Some(idx) = s.find("/v2/") {
                    let start = idx + "/v2/".len();
                    let end = s[start..].find('/').map(|i| start + i).unwrap_or(s.len());
                    s.replace_range(start..end, "***");
                    return s;
                }
            }
            // For QuickNode/GenesisGo, redact last path segment
            match provider_type {
                RpcProviderType::QuickNode | RpcProviderType::GenesisGo => {
                    if let Some(mut path_segments) = url.path_segments() {
                        if let Some(last) = path_segments.next_back() {
                            return s.replace(last, "***");
                        }
                    }
                    s
                }
                _ => s,
            }
        }
        f.debug_struct("RpcProvider")
            .field("name", &self.name)
            .field(
                "websocket_url",
                &redact_url(&self.websocket_url, &self.provider_type),
            )
            .field("priority", &self.priority)
            .field("provider_type", &self.provider_type)
            .finish()
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
    Threshold(f64),
    #[error("Invalid max price age: {0}ms. Must be between 100 and 60000 milliseconds")]
    MaxPriceAge(u64),
    #[error("Invalid price bound: {0}")]
    PriceBound(String),
}
