use crate::config::TradingPair;
use crate::price::{PriceSource, PriceUpdate};
use crate::websocket::reconnect::{ReconnectConfig, ReconnectError, ReconnectHandler};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

/// Errors that can occur with Binance WebSocket operations
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum BinanceError {
    #[error("WebSocket connection error: {0}")]
    ConnectionError(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),
    #[error("Connection timeout: {0:?}")]
    Timeout(Duration),
    #[error("Reconnection failed: {0}")]
    ReconnectFailed(#[from] ReconnectError),
    #[error("Invalid trading pair: {0:?}")]
    InvalidTradingPair(TradingPair),
}

/// Binance WebSocket subscription message for ticker streams
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct SubscribeMessage {
    method: String,
    params: Vec<String>,
    id: u64,
}

/// Binance WebSocket ticker data response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TickerData {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    price: String,
    #[serde(rename = "E")]
    event_time: u64,
}

/// Binance WebSocket stream data wrapper
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamData {
    stream: String,
    data: TickerData,
}

/// Configuration for Binance WebSocket client
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BinanceConfig {
    /// WebSocket endpoint URL
    pub base_url: String,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Reconnection configuration
    pub reconnect_config: ReconnectConfig,
}

impl Default for BinanceConfig {
    fn default() -> Self {
        Self {
            base_url: "wss://stream.binance.com:9443/ws".to_string(),
            connection_timeout: Duration::from_secs(10),
            reconnect_config: ReconnectConfig::default(),
        }
    }
}

impl BinanceConfig {
    /// Create new Binance configuration with custom settings
    #[allow(dead_code)]
    pub fn new(base_url: String, connection_timeout: Duration) -> Self {
        Self {
            base_url,
            connection_timeout,
            reconnect_config: ReconnectConfig::default(),
        }
    }

    /// Set reconnection configuration
    #[allow(dead_code)]
    pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }
}

/// Binance WebSocket client for real-time price data
#[allow(dead_code)]
pub struct BinanceClient {
    config: BinanceConfig,
    trading_pair: TradingPair,
    reconnect_handler: ReconnectHandler,
}

impl BinanceClient {
    /// Create new Binance WebSocket client
    #[allow(dead_code)]
    pub fn new(config: BinanceConfig, trading_pair: TradingPair) -> Result<Self, BinanceError> {
        let reconnect_handler = ReconnectHandler::new(config.reconnect_config.clone())
            .map_err(|e| BinanceError::JsonError(serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))))?;

        Ok(Self {
            config,
            trading_pair,
            reconnect_handler,
        })
    }

    /// Create client with default configuration
    #[allow(dead_code)]
    pub fn with_default(trading_pair: TradingPair) -> Result<Self, BinanceError> {
        Self::new(BinanceConfig::default(), trading_pair)
    }

    /// Start the WebSocket client and stream price updates
    #[allow(dead_code)]
    pub async fn start<F>(&mut self, mut callback: F) -> Result<(), BinanceError>
    where
        F: FnMut(PriceUpdate) + Send,
    {
        loop {
            match self.connect_and_stream(&mut callback).await {
                Ok(()) => {
                    // Normal disconnect, reset reconnection handler
                    self.reconnect_handler.reset();
                    break;
                }
                Err(e) => {
                    eprintln!("Binance WebSocket error: {}", e);

                    // Determine if we should reconnect
                    match self.reconnect_handler.should_reconnect() {
                        Ok(delay) => {
                            eprintln!(
                                "Reconnecting to Binance in {:?} (attempt {})",
                                delay,
                                self.reconnect_handler.attempt_count()
                            );
                            sleep(delay).await;
                        }
                        Err(reconnect_error) => {
                            eprintln!("Giving up on Binance reconnection: {}", reconnect_error);
                            return Err(BinanceError::ReconnectFailed(reconnect_error));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Connect to Binance WebSocket and stream data
    #[allow(dead_code)]
    async fn connect_and_stream<F>(&self, callback: &mut F) -> Result<(), BinanceError>
    where
        F: FnMut(PriceUpdate) + Send,
    {
        let url = self.build_websocket_url()?;

        // Connect with timeout
        let (ws_stream, _) = timeout(self.config.connection_timeout, connect_async(&url))
            .await
            .map_err(|_| BinanceError::Timeout(self.config.connection_timeout))?
            .map_err(|e| BinanceError::ConnectionError(Box::new(e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to ticker stream
        let subscribe_msg = self.create_subscribe_message()?;
        let msg_text = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(msg_text)).await.map_err(|e| BinanceError::ConnectionError(Box::new(e)))?;

        // Process incoming messages
        while let Some(message) = read.next().await {
            match message.map_err(|e| BinanceError::ConnectionError(Box::new(e)))? {
                Message::Text(text) => {
                    if let Ok(price_update) = self.parse_ticker_message(&text) {
                        callback(price_update);
                    }
                }
                Message::Ping(payload) => {
                    write.send(Message::Pong(payload)).await.map_err(|e| BinanceError::ConnectionError(Box::new(e)))?;
                }
                Message::Close(_) => {
                    eprintln!("Binance WebSocket connection closed");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Build WebSocket URL for the connection
    fn build_websocket_url(&self) -> Result<Url, BinanceError> {
        let url = Url::parse(&self.config.base_url)?;
        Ok(url)
    }

    /// Create subscription message for ticker stream
    fn create_subscribe_message(&self) -> Result<SubscribeMessage, BinanceError> {
        let symbol = self.trading_pair_to_binance_symbol()?;
        let stream = format!("{}@ticker", symbol.to_lowercase());

        Ok(SubscribeMessage {
            method: "SUBSCRIBE".to_string(),
            params: vec![stream],
            id: 1,
        })
    }

    /// Parse ticker message and convert to PriceUpdate
    fn parse_ticker_message(&self, text: &str) -> Result<PriceUpdate, BinanceError> {
        let stream_data: StreamData = serde_json::from_str(text)?;

        let price: f64 = stream_data.data.price.parse()
            .map_err(|_| BinanceError::JsonError(serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid price format"))))?;

        Ok(PriceUpdate::new(
            PriceSource::Binance,
            self.trading_pair,
            price,
        ))
    }

    /// Convert TradingPair to Binance symbol format
    fn trading_pair_to_binance_symbol(&self) -> Result<String, BinanceError> {
        match self.trading_pair {
            TradingPair::SolUsdt => Ok("SOLUSDT".to_string()),
            TradingPair::SolUsdc => Ok("SOLUSDC".to_string()),
        }
    }

    /// Get current reconnection attempt count
    #[allow(dead_code)]
    pub fn reconnect_attempts(&self) -> usize {
        self.reconnect_handler.attempt_count()
    }

    /// Get elapsed time since first reconnection attempt
    #[allow(dead_code)]
    pub fn reconnect_elapsed_time(&self) -> Option<Duration> {
        self.reconnect_handler.elapsed_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_config_creation() {
        let config = BinanceConfig::default();
        assert_eq!(config.base_url, "wss://stream.binance.com:9443/ws");
        assert_eq!(config.connection_timeout, Duration::from_secs(10));

        let custom_config = BinanceConfig::new(
            "wss://testnet.binance.vision/ws".to_string(),
            Duration::from_secs(5),
        );
        assert_eq!(custom_config.base_url, "wss://testnet.binance.vision/ws");
        assert_eq!(custom_config.connection_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_binance_client_creation() {
        let config = BinanceConfig::default();
        let client = BinanceClient::new(config, TradingPair::SolUsdt);
        assert!(client.is_ok());

        let default_client = BinanceClient::with_default(TradingPair::SolUsdc);
        assert!(default_client.is_ok());
    }

    #[test]
    fn test_trading_pair_to_symbol() {
        let config = BinanceConfig::default();
        let client = BinanceClient::new(config, TradingPair::SolUsdt).unwrap();

        assert_eq!(client.trading_pair_to_binance_symbol().unwrap(), "SOLUSDT");

        let client2 = BinanceClient::with_default(TradingPair::SolUsdc).unwrap();
        assert_eq!(client2.trading_pair_to_binance_symbol().unwrap(), "SOLUSDC");
    }

    #[test]
    fn test_subscribe_message_creation() {
        let client = BinanceClient::with_default(TradingPair::SolUsdt).unwrap();
        let msg = client.create_subscribe_message().unwrap();

        assert_eq!(msg.method, "SUBSCRIBE");
        assert_eq!(msg.params, vec!["solusdt@ticker"]);
        assert_eq!(msg.id, 1);
    }

    #[test]
    fn test_ticker_message_parsing() {
        let client = BinanceClient::with_default(TradingPair::SolUsdt).unwrap();

        let ticker_json = r#"{
            "stream": "solusdt@ticker",
            "data": {
                "s": "SOLUSDT",
                "c": "195.50",
                "E": 1699123456789
            }
        }"#;

        let price_update = client.parse_ticker_message(ticker_json).unwrap();

        assert_eq!(price_update.source, PriceSource::Binance);
        assert_eq!(price_update.pair, TradingPair::SolUsdt);
        assert_eq!(price_update.price, 195.50);
    }

    #[test]
    fn test_url_building() {
        let config = BinanceConfig::default();
        let client = BinanceClient::new(config, TradingPair::SolUsdt).unwrap();

        let url = client.build_websocket_url().unwrap();
        assert_eq!(url.as_str(), "wss://stream.binance.com:9443/ws");
    }

    #[test]
    fn test_reconnect_config_integration() {
        let reconnect_config = ReconnectConfig::new(
            Duration::from_millis(500),
            Duration::from_secs(30),
            1.5,
        );

        let binance_config = BinanceConfig::default()
            .with_reconnect_config(reconnect_config);

        let client = BinanceClient::new(binance_config, TradingPair::SolUsdt).unwrap();

        assert_eq!(client.reconnect_attempts(), 0);
        assert!(client.reconnect_elapsed_time().is_none());
    }

    #[test]
    fn test_invalid_price_format() {
        let client = BinanceClient::with_default(TradingPair::SolUsdt).unwrap();

        let invalid_json = r#"{
            "stream": "solusdt@ticker",
            "data": {
                "s": "SOLUSDT",
                "c": "invalid_price",
                "E": 1699123456789
            }
        }"#;

        let result = client.parse_ticker_message(invalid_json);
        assert!(result.is_err());
    }
}