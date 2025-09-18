#[cfg(test)]
use crate::config::{Config, RawConfig, TradingPair};
#[cfg(test)]
use crate::output::OutputFormat;

/// Common test utilities for creating test configurations and mock data
#[cfg(test)]
pub mod config {
    use super::*;

    /// Create a default test configuration with standard settings
    pub fn create_test_config() -> Config {
        create_test_config_with_threshold(0.5)
    }

    /// Create a test configuration with custom profit threshold
    pub fn create_test_config_with_threshold(threshold: f64) -> Config {
        let raw = RawConfig {
            pair: TradingPair::SolUsdt,
            threshold,
            max_price_age_ms: 5000,
            rpc_url: None,
            helius_api_key: None,
            alchemy_api_key: None,
            genesisgo_api_key: None,
            output_format: OutputFormat::Table,
            min_price: 1.0,
            max_price: 10000.0,
            enable_performance_monitor: false,
        };

        Config::new(&raw).expect("Valid test configuration")
    }

    /// Create a test configuration for high-threshold arbitrage detection
    pub fn create_high_threshold_test_config() -> Config {
        create_test_config_with_threshold(1.0)
    }

    /// Create a test configuration with very low threshold for testing opportunity detection
    pub fn create_low_threshold_test_config() -> Config {
        create_test_config_with_threshold(0.01)
    }
}
