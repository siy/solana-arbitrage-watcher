use crate::config::TradingPair;
use log::error;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Helper function for calculating age in milliseconds
fn calculate_age_ms(timestamp: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(timestamp)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// Custom serde serialization for SystemTime
mod systemtime_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u128::deserialize(deserializer)?;
        let duration = std::time::Duration::from_millis(millis as u64);
        Ok(UNIX_EPOCH + duration)
    }
}

/// Price update from a WebSocket source
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriceUpdate {
    pub source: PriceSource,
    pub pair: TradingPair,
    pub price: f64,
    #[serde(with = "systemtime_serde")]
    pub timestamp: SystemTime,
}

impl PriceUpdate {
    /// Create new price update with current timestamp
    pub fn new(source: PriceSource, pair: TradingPair, price: f64) -> Self {
        Self {
            source,
            pair,
            price,
            timestamp: SystemTime::now(),
        }
    }

    /// Validate price value for financial data integrity
    #[allow(dead_code)]
    pub fn is_valid_price(&self) -> bool {
        self.price.is_finite() && self.price > 0.0
    }

    /// Create price update with specific timestamp
    #[allow(dead_code)]
    pub fn with_timestamp(
        source: PriceSource,
        pair: TradingPair,
        price: f64,
        timestamp: SystemTime,
    ) -> Self {
        Self {
            source,
            pair,
            price,
            timestamp,
        }
    }

    /// Get age of this price update in milliseconds
    #[allow(dead_code)]
    pub fn age_ms(&self) -> u64 {
        calculate_age_ms(self.timestamp)
    }

    /// Check if this price update is stale based on max age
    #[allow(dead_code)]
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        self.age_ms() > max_age_ms
    }
}

/// Price source identifier for arbitrage direction calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceSource {
    Solana,
    Binance,
}

impl PriceSource {
    /// Get display name for this source
    pub fn display_name(&self) -> &'static str {
        match self {
            PriceSource::Solana => "Solana DEX",
            PriceSource::Binance => "Binance",
        }
    }

    /// Check if this is a DEX source
    #[allow(dead_code)]
    pub fn is_dex(&self) -> bool {
        matches!(self, PriceSource::Solana)
    }

    /// Check if this is a CEX source
    #[allow(dead_code)]
    pub fn is_cex(&self) -> bool {
        matches!(self, PriceSource::Binance)
    }
}

/// Price data with source metadata for arbitrage calculations
#[derive(Debug, Clone)]
pub struct SourcePrice {
    pub price: f64,
    #[allow(dead_code)] // Used for debugging and future features
    pub source: PriceSource,
    pub timestamp: SystemTime,
}

impl SourcePrice {
    /// Create new source price with current timestamp
    #[allow(dead_code)]
    pub fn new(price: f64, source: PriceSource) -> Self {
        Self {
            price,
            source,
            timestamp: SystemTime::now(),
        }
    }

    /// Create source price from price update
    pub fn from_update(update: &PriceUpdate) -> Self {
        Self {
            price: update.price,
            source: update.source,
            timestamp: update.timestamp,
        }
    }

    /// Get age of this price data in milliseconds
    pub fn age_ms(&self) -> u64 {
        calculate_age_ms(self.timestamp)
    }

    /// Check if price data is considered stale
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        self.age_ms() > max_age_ms
    }

    /// Get timestamp as milliseconds since Unix epoch
    #[allow(dead_code)]
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Thread-safe cache for latest price data from each source
#[derive(Debug)]
pub struct PriceCache {
    solana_price: Arc<RwLock<Option<SourcePrice>>>,
    binance_price: Arc<RwLock<Option<SourcePrice>>>,
}

impl Default for PriceCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceCache {
    /// Create new empty price cache
    pub fn new() -> Self {
        Self {
            solana_price: Arc::new(RwLock::new(None)),
            binance_price: Arc::new(RwLock::new(None)),
        }
    }

    /// Update price for a specific source
    pub fn update(&self, update: &PriceUpdate) {
        let source_price = SourcePrice::from_update(update);
        match update.source {
            PriceSource::Solana => match self.solana_price.write() {
                Ok(mut price) => *price = Some(source_price),
                Err(_) => error!("Failed to acquire write lock for Solana price"),
            },
            PriceSource::Binance => match self.binance_price.write() {
                Ok(mut price) => *price = Some(source_price),
                Err(_) => error!("Failed to acquire write lock for Binance price"),
            },
        }
    }

    /// Get current prices from both sources if available
    pub fn get_both_prices(&self) -> Option<(SourcePrice, SourcePrice)> {
        let solana_lock = self.solana_price.read().ok()?;
        let binance_lock = self.binance_price.read().ok()?;
        let solana = solana_lock.clone()?;
        let binance = binance_lock.clone()?;
        Some((solana, binance))
    }

    /// Get price for specific source
    #[allow(dead_code)]
    pub fn get_price(&self, source: PriceSource) -> Option<SourcePrice> {
        match source {
            PriceSource::Solana => self.solana_price.read().ok()?.clone(),
            PriceSource::Binance => self.binance_price.read().ok()?.clone(),
        }
    }

    /// Check if both prices are available and fresh
    pub fn has_fresh_prices(&self, max_age_ms: u64) -> bool {
        self.get_both_prices()
            .map(|(solana, binance)| !solana.is_stale(max_age_ms) && !binance.is_stale(max_age_ms))
            .unwrap_or(false)
    }

    /// Clear stale prices based on max age
    #[allow(dead_code)]
    pub fn clear_stale_prices(&self, max_age_ms: u64) {
        match self.solana_price.write() {
            Ok(mut s) => {
                if s.as_ref().is_some_and(|p| p.is_stale(max_age_ms)) {
                    *s = None;
                }
            }
            Err(_) => error!("Failed to acquire write lock for Solana price during cleanup"),
        }
        match self.binance_price.write() {
            Ok(mut b) => {
                if b.as_ref().is_some_and(|p| p.is_stale(max_age_ms)) {
                    *b = None;
                }
            }
            Err(_) => error!("Failed to acquire write lock for Binance price during cleanup"),
        }
    }
}
