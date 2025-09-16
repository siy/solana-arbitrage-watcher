use crate::config::TradingPair;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Price update from a WebSocket source
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriceUpdate {
    pub source: PriceSource,
    pub pair: TradingPair,
    pub price: f64,
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
        let ms = SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
            .as_millis();
        ms.min(u128::from(u64::MAX)) as u64
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
        let ms = SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
            .as_millis();
        ms.min(u128::from(u64::MAX)) as u64
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
            PriceSource::Solana => {
                if let Ok(mut price) = self.solana_price.write() {
                    *price = Some(source_price);
                }
            }
            PriceSource::Binance => {
                if let Ok(mut price) = self.binance_price.write() {
                    *price = Some(source_price);
                }
            }
        }
    }

    /// Get current prices from both sources if available
    pub fn get_both_prices(&self) -> Option<(SourcePrice, SourcePrice)> {
        let solana = self.solana_price.read().ok()?.clone()?;
        let binance = self.binance_price.read().ok()?.clone()?;
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
        if let Ok(mut s) = self.solana_price.write() {
            if s.as_ref().is_some_and(|p| p.is_stale(max_age_ms)) {
                *s = None;
            }
        }
        if let Ok(mut b) = self.binance_price.write() {
            if b.as_ref().is_some_and(|p| p.is_stale(max_age_ms)) {
                *b = None;
            }
        }
    }
}
