use crate::config::{Config, MaxPriceAge};
use crate::price::{PriceCache, PriceSource, SourcePrice};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::time::{interval, sleep};

/// Errors that can occur during price processing
#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("No fresh price data available")]
    NoFreshData,
    #[error("Price data is stale: age={age_ms}ms, max={max_age_ms}ms")]
    StaleData {
        age_ms: u64,
        max_age_ms: u64,
    },
    #[error("Invalid price detected: {price}")]
    InvalidPrice { price: f64 },
    #[error("Price cache lock error")]
    CacheLockError,
}

/// Validated price pair with freshness guarantee
#[derive(Debug, Clone)]
pub struct ValidatedPricePair {
    pub solana_price: SourcePrice,
    pub binance_price: SourcePrice,
    pub price_spread: f64,
    pub price_spread_percentage: f64,
}

impl ValidatedPricePair {
    /// Create new validated price pair
    pub fn new(solana_price: SourcePrice, binance_price: SourcePrice) -> Self {
        let price_spread = (solana_price.price - binance_price.price).abs();
        let price_spread_percentage = (price_spread / binance_price.price) * 100.0;

        Self {
            solana_price,
            binance_price,
            price_spread,
            price_spread_percentage,
        }
    }

    /// Get the higher price source
    pub fn higher_price_source(&self) -> PriceSource {
        if self.solana_price.price > self.binance_price.price {
            PriceSource::Solana
        } else {
            PriceSource::Binance
        }
    }

    /// Get the lower price source
    pub fn lower_price_source(&self) -> PriceSource {
        if self.solana_price.price < self.binance_price.price {
            PriceSource::Solana
        } else {
            PriceSource::Binance
        }
    }

    /// Get price for specific source
    pub fn get_price(&self, source: PriceSource) -> &SourcePrice {
        match source {
            PriceSource::Solana => &self.solana_price,
            PriceSource::Binance => &self.binance_price,
        }
    }

    /// Check if prices are inverted (Solana higher than Binance)
    pub fn is_inverted(&self) -> bool {
        self.solana_price.price > self.binance_price.price
    }

    /// Get maximum age of either price
    pub fn max_age_ms(&self) -> u64 {
        self.solana_price.age_ms().max(self.binance_price.age_ms())
    }
}

/// Price processor that validates and processes price data from cache
pub struct PriceProcessor {
    price_cache: Arc<PriceCache>,
    max_price_age: MaxPriceAge,
    validation_enabled: bool,
}

impl PriceProcessor {
    /// Create new price processor
    pub fn new(price_cache: Arc<PriceCache>, config: &Config) -> Self {
        Self {
            price_cache,
            max_price_age: config.max_price_age_ms,
            validation_enabled: true,
        }
    }

    /// Create processor with custom settings
    #[allow(dead_code)]
    pub fn with_custom_settings(
        price_cache: Arc<PriceCache>,
        max_price_age: MaxPriceAge,
        validation_enabled: bool,
    ) -> Self {
        Self {
            price_cache,
            max_price_age,
            validation_enabled,
        }
    }

    /// Get validated price pair if available and fresh
    pub fn get_validated_prices(&self) -> Result<ValidatedPricePair, ProcessorError> {
        let (solana_price, binance_price) = self
            .price_cache
            .get_both_prices()
            .ok_or(ProcessorError::NoFreshData)?;

        // Validate freshness
        self.validate_price_freshness(&solana_price)?;
        self.validate_price_freshness(&binance_price)?;

        // Validate price values if enabled
        if self.validation_enabled {
            self.validate_price_value(&solana_price)?;
            self.validate_price_value(&binance_price)?;
        }

        Ok(ValidatedPricePair::new(solana_price, binance_price))
    }

    /// Wait for fresh price data to become available
    pub async fn wait_for_fresh_prices(&self, timeout: Duration) -> Result<ValidatedPricePair, ProcessorError> {
        let start = tokio::time::Instant::now();
        let mut check_interval = interval(Duration::from_millis(100));

        loop {
            if let Ok(prices) = self.get_validated_prices() {
                return Ok(prices);
            }

            if start.elapsed() >= timeout {
                return Err(ProcessorError::NoFreshData);
            }

            check_interval.tick().await;
        }
    }

    /// Start background cleanup task for stale prices
    #[allow(dead_code)]
    pub async fn start_cleanup_task(&self, cleanup_interval: Duration) {
        let cache = Arc::clone(&self.price_cache);
        let max_age = self.max_price_age.value();

        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);
            loop {
                interval.tick().await;
                cache.clear_stale_prices(max_age);
            }
        });
    }

    /// Check if fresh prices are available without validation
    pub fn has_fresh_prices(&self) -> bool {
        self.price_cache.has_fresh_prices(self.max_price_age.value())
    }

    /// Get current price age statistics
    #[allow(dead_code)]
    pub fn get_price_age_stats(&self) -> Option<(u64, u64)> {
        let (solana, binance) = self.price_cache.get_both_prices()?;
        Some((solana.age_ms(), binance.age_ms()))
    }

    /// Validate that price is not stale
    fn validate_price_freshness(&self, price: &SourcePrice) -> Result<(), ProcessorError> {
        let age_ms = price.age_ms();
        let max_age_ms = self.max_price_age.value();

        if age_ms > max_age_ms {
            return Err(ProcessorError::StaleData {
                age_ms,
                max_age_ms,
            });
        }

        Ok(())
    }

    /// Validate that price value is reasonable
    fn validate_price_value(&self, price: &SourcePrice) -> Result<(), ProcessorError> {
        if !price.price.is_finite() || price.price <= 0.0 {
            return Err(ProcessorError::InvalidPrice {
                price: price.price,
            });
        }

        // Additional validation: reasonable price ranges for SOL
        // This prevents obviously incorrect data from being processed
        if price.price < 1.0 || price.price > 10000.0 {
            return Err(ProcessorError::InvalidPrice {
                price: price.price,
            });
        }

        Ok(())
    }

    /// Get maximum allowed price age
    #[allow(dead_code)]
    pub fn max_price_age(&self) -> u64 {
        self.max_price_age.value()
    }

    /// Check if validation is enabled
    #[allow(dead_code)]
    pub fn is_validation_enabled(&self) -> bool {
        self.validation_enabled
    }

    /// Enable or disable price validation
    #[allow(dead_code)]
    pub fn set_validation_enabled(&mut self, enabled: bool) {
        self.validation_enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, RawConfig, TradingPair};
    use crate::price::{PriceCache, PriceSource, PriceUpdate, SourcePrice};

    fn create_test_config() -> Config {
        let raw = RawConfig {
            pair: TradingPair::SolUsdt,
            threshold: 0.5,
            max_price_age_ms: 5000,
            rpc_url: None,
        };
        Config::new(&raw).unwrap()
    }

    fn create_test_price_cache() -> Arc<PriceCache> {
        let cache = Arc::new(PriceCache::new());

        // Add some test data
        let solana_update = PriceUpdate::new(PriceSource::Solana, TradingPair::SolUsdt, 195.5);
        let binance_update = PriceUpdate::new(PriceSource::Binance, TradingPair::SolUsdt, 195.0);

        cache.update(&solana_update);
        cache.update(&binance_update);

        cache
    }

    #[test]
    fn test_processor_creation() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new());
        let processor = PriceProcessor::new(cache, &config);

        assert_eq!(processor.max_price_age(), 5000);
        assert!(processor.is_validation_enabled());
    }

    #[test]
    fn test_custom_processor_settings() {
        let cache = Arc::new(PriceCache::new());
        let max_age = MaxPriceAge::new(1000);
        let processor = PriceProcessor::with_custom_settings(cache, max_age, false);

        assert_eq!(processor.max_price_age(), 1000);
        assert!(!processor.is_validation_enabled());
    }

    #[test]
    fn test_validated_price_pair_creation() {
        let solana_price = SourcePrice::new(195.5, PriceSource::Solana);
        let binance_price = SourcePrice::new(195.0, PriceSource::Binance);

        let pair = ValidatedPricePair::new(solana_price, binance_price);

        assert_eq!(pair.price_spread, 0.5);
        assert!((pair.price_spread_percentage - 0.256).abs() < 0.001);
        assert_eq!(pair.higher_price_source(), PriceSource::Solana);
        assert_eq!(pair.lower_price_source(), PriceSource::Binance);
        assert!(pair.is_inverted());
    }

    #[test]
    fn test_get_validated_prices_success() {
        let config = create_test_config();
        let cache = create_test_price_cache();
        let processor = PriceProcessor::new(cache, &config);

        let result = processor.get_validated_prices();
        assert!(result.is_ok());

        let pair = result.unwrap();
        assert_eq!(pair.solana_price.source, PriceSource::Solana);
        assert_eq!(pair.binance_price.source, PriceSource::Binance);
    }

    #[test]
    fn test_get_validated_prices_no_data() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new()); // Empty cache
        let processor = PriceProcessor::new(cache, &config);

        let result = processor.get_validated_prices();
        assert!(matches!(result, Err(ProcessorError::NoFreshData)));
    }

    #[test]
    fn test_price_validation_disabled() {
        let cache = Arc::new(PriceCache::new());
        let max_age = MaxPriceAge::new(5000);
        let mut processor = PriceProcessor::with_custom_settings(cache.clone(), max_age, true);

        // Test setting validation
        processor.set_validation_enabled(false);
        assert!(!processor.is_validation_enabled());

        processor.set_validation_enabled(true);
        assert!(processor.is_validation_enabled());
    }

    #[test]
    fn test_invalid_price_detection() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new());
        let processor = PriceProcessor::new(cache.clone(), &config);

        // Add invalid price data
        let invalid_update = PriceUpdate::new(PriceSource::Solana, TradingPair::SolUsdt, -1.0);
        cache.update(&invalid_update);

        let valid_update = PriceUpdate::new(PriceSource::Binance, TradingPair::SolUsdt, 195.0);
        cache.update(&valid_update);

        let result = processor.get_validated_prices();
        assert!(matches!(result, Err(ProcessorError::InvalidPrice { .. })));
    }

    #[test]
    fn test_price_freshness_check() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new());
        let processor = PriceProcessor::new(cache, &config);

        assert!(!processor.has_fresh_prices());
    }

    #[test]
    fn test_price_spread_calculation() {
        let solana_price = SourcePrice::new(200.0, PriceSource::Solana);
        let binance_price = SourcePrice::new(190.0, PriceSource::Binance);

        let pair = ValidatedPricePair::new(solana_price, binance_price);

        assert_eq!(pair.price_spread, 10.0);
        assert!((pair.price_spread_percentage - 5.263).abs() < 0.001);
    }

    #[test]
    fn test_price_source_identification() {
        let solana_price = SourcePrice::new(190.0, PriceSource::Solana);
        let binance_price = SourcePrice::new(200.0, PriceSource::Binance);

        let pair = ValidatedPricePair::new(solana_price, binance_price);

        assert_eq!(pair.higher_price_source(), PriceSource::Binance);
        assert_eq!(pair.lower_price_source(), PriceSource::Solana);
        assert!(!pair.is_inverted()); // Binance higher than Solana
    }

    #[tokio::test]
    async fn test_wait_for_fresh_prices_timeout() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new()); // Empty cache
        let processor = PriceProcessor::new(cache, &config);

        let result = processor.wait_for_fresh_prices(Duration::from_millis(50)).await;
        assert!(matches!(result, Err(ProcessorError::NoFreshData)));
    }

    #[tokio::test]
    async fn test_wait_for_fresh_prices_success() {
        let config = create_test_config();
        let cache = Arc::new(PriceCache::new());
        let processor = PriceProcessor::new(cache.clone(), &config);

        // Add data after a short delay
        let cache_clone = Arc::clone(&cache);
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            let solana_update = PriceUpdate::new(PriceSource::Solana, TradingPair::SolUsdt, 195.5);
            let binance_update = PriceUpdate::new(PriceSource::Binance, TradingPair::SolUsdt, 195.0);
            cache_clone.update(&solana_update);
            cache_clone.update(&binance_update);
        });

        let result = processor.wait_for_fresh_prices(Duration::from_millis(100)).await;
        assert!(result.is_ok());
    }
}