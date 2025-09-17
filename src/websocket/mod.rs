pub mod binance;
pub mod reconnect;
pub mod solana;

use crate::config::{Config, TradingPair};
use crate::price::PriceCache;
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinHandle;

/// Type alias for complex startup return type
type StartupResult = (
    Arc<PriceCache>,
    JoinHandle<Result<(), BinanceError>>,
    JoinHandle<Result<(), SolanaError>>,
);

pub use binance::{BinanceClient, BinanceConfig, BinanceError};
// ReconnectHandler is available but not currently used in public API
#[allow(unused_imports)]
pub use solana::{SolanaClient, SolanaConfig, SolanaError};

/// Errors that can occur in the WebSocket connection manager
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ConnectionManagerError {
    #[error("Binance connection error: {0}")]
    BinanceError(#[from] BinanceError),
    #[error("Solana connection error: {0}")]
    SolanaError(#[from] SolanaError),
    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("All connections failed")]
    AllConnectionsFailed,
}

/// WebSocket connection manager that coordinates multiple price sources
#[allow(dead_code)]
pub struct ConnectionManager {
    binance_client: BinanceClient,
    solana_client: SolanaClient,
    price_cache: Arc<PriceCache>,
    trading_pair: TradingPair,
}

impl ConnectionManager {
    /// Create new connection manager from configuration
    #[allow(dead_code)]
    pub fn new(config: &Config) -> Result<Self, ConnectionManagerError> {
        // Create Binance client with default configuration
        let binance_client = BinanceClient::with_default(config.pair)?;

        // Create Solana client from RPC providers in config
        let solana_client =
            SolanaClient::from_providers(config.rpc_providers.clone(), config.pair)?;

        let price_cache = Arc::new(PriceCache::new());

        Ok(Self {
            binance_client,
            solana_client,
            price_cache,
            trading_pair: config.pair,
        })
    }

    /// Start all WebSocket connections and return price cache and shutdown handles
    #[allow(dead_code)]
    pub fn start_with_handles(mut self) -> StartupResult {
        let price_cache = Arc::clone(&self.price_cache);

        // Start Binance connection
        let binance_cache = Arc::clone(&price_cache);
        let binance_handle: JoinHandle<Result<(), BinanceError>> = tokio::spawn(async move {
            self.binance_client
                .start(move |price_update| {
                    binance_cache.update(&price_update);
                })
                .await
        });

        // Start Solana connection
        let solana_cache = Arc::clone(&price_cache);
        let solana_handle: JoinHandle<Result<(), SolanaError>> = tokio::spawn(async move {
            self.solana_client
                .start(move |price_update| {
                    solana_cache.update(&price_update);
                })
                .await
        });

        (price_cache, binance_handle, solana_handle)
    }

    /// Start all WebSocket connections and return price cache (legacy method)
    #[allow(dead_code)]
    pub async fn start(self) -> Result<Arc<PriceCache>, ConnectionManagerError> {
        let (price_cache, binance_handle, solana_handle) = self.start_with_handles();

        // Monitor connections
        tokio::spawn(async move {
            let binance_result = binance_handle.await;
            let solana_result = solana_handle.await;

            match (binance_result, solana_result) {
                (Ok(Ok(())), Ok(Ok(()))) => {
                    log::info!("Both connections completed successfully");
                }
                (Ok(Err(e)), _) => {
                    log::error!("Binance connection failed: {}", e);
                }
                (_, Ok(Err(e))) => {
                    log::error!("Solana connection failed: {}", e);
                }
                (Err(e), _) => {
                    log::error!("Binance task panicked: {}", e);
                }
                (_, Err(e)) => {
                    log::error!("Solana task panicked: {}", e);
                }
            }
        });

        Ok(price_cache)
    }

    /// Create connection manager with custom WebSocket configurations
    #[allow(dead_code)]
    pub fn with_custom_configs(
        config: &Config,
        binance_config: BinanceConfig,
        solana_config: SolanaConfig,
    ) -> Result<Self, ConnectionManagerError> {
        let binance_client = BinanceClient::new(binance_config, config.pair)?;
        let solana_client = SolanaClient::new(solana_config, config.pair)?;
        let price_cache = Arc::new(PriceCache::new());

        Ok(Self {
            binance_client,
            solana_client,
            price_cache,
            trading_pair: config.pair,
        })
    }

    /// Get the price cache reference
    #[allow(dead_code)]
    pub fn price_cache(&self) -> &Arc<PriceCache> {
        &self.price_cache
    }

    /// Get trading pair
    #[allow(dead_code)]
    pub fn trading_pair(&self) -> TradingPair {
        self.trading_pair
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, RawConfig, TradingPair};

    fn create_test_config() -> Config {
        let raw = RawConfig {
            pair: TradingPair::SolUsdt,
            threshold: 0.5,
            max_price_age_ms: 5000,
            rpc_url: None,
            helius_api_key: None,
            quicknode_api_key: None,
            alchemy_api_key: None,
            genesisgo_api_key: None,
            output_format: crate::output::OutputFormat::Table,
            min_price: 1.0,
            max_price: 10000.0,
        };
        Config::new(&raw).unwrap()
    }

    #[test]
    fn test_connection_manager_creation() {
        let config = create_test_config();
        let manager = ConnectionManager::new(&config);
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.trading_pair(), TradingPair::SolUsdt);
    }

    #[test]
    fn test_connection_manager_with_custom_configs() {
        let config = create_test_config();
        let binance_config = BinanceConfig::default();
        let solana_config = SolanaConfig::default();

        let manager =
            ConnectionManager::with_custom_configs(&config, binance_config, solana_config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_price_cache_access() {
        let config = create_test_config();
        let manager = ConnectionManager::new(&config).unwrap();
        let cache = manager.price_cache();

        // Cache should be empty initially
        assert!(cache.get_both_prices().is_none());
    }
}
