use crate::config::TradingPair;
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
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Check if this price update is stale based on max age
    #[allow(dead_code)]
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        self.age_ms() > max_age_ms
    }
}

/// Price source identifier for arbitrage direction calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[allow(dead_code)]
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
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
            .as_millis() as u64
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
#[derive(Debug, Default)]
pub struct PriceCache {
    pub solana_price: Option<SourcePrice>,
    pub binance_price: Option<SourcePrice>,
}

impl PriceCache {
    /// Create new empty price cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Update price for a specific source
    pub fn update(&mut self, update: &PriceUpdate) {
        let source_price = SourcePrice::from_update(update);
        match update.source {
            PriceSource::Solana => self.solana_price = Some(source_price),
            PriceSource::Binance => self.binance_price = Some(source_price),
        }
    }

    /// Get current prices from both sources if available
    pub fn get_both_prices(&self) -> Option<(SourcePrice, SourcePrice)> {
        match (&self.solana_price, &self.binance_price) {
            (Some(solana), Some(binance)) => Some((solana.clone(), binance.clone())),
            _ => None,
        }
    }

    /// Get price for specific source
    #[allow(dead_code)]
    pub fn get_price(&self, source: PriceSource) -> Option<&SourcePrice> {
        match source {
            PriceSource::Solana => self.solana_price.as_ref(),
            PriceSource::Binance => self.binance_price.as_ref(),
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
    pub fn clear_stale_prices(&mut self, max_age_ms: u64) {
        if let Some(ref price) = self.solana_price {
            if price.is_stale(max_age_ms) {
                self.solana_price = None;
            }
        }

        if let Some(ref price) = self.binance_price {
            if price.is_stale(max_age_ms) {
                self.binance_price = None;
            }
        }
    }
}
