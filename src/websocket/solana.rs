use crate::config::{RpcProvider, TradingPair};
use crate::price::{PriceSource, PriceUpdate};
use crate::websocket::reconnect::{ReconnectConfig, ReconnectError, ReconnectHandler};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Errors that can occur with Solana WebSocket operations
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SolanaError {
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
    #[error("No RPC providers available")]
    NoProvidersAvailable,
    #[error("All RPC providers failed")]
    AllProvidersFailed,
    #[error("Invalid account data received")]
    InvalidAccountData,
}

/// Solana JSON-RPC request for account subscription
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct AccountSubscribeRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// Solana JSON-RPC response wrapper
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// Solana JSON-RPC error structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Solana account notification structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AccountNotification {
    subscription: u64,
    result: AccountData,
}

/// Solana account data structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AccountData {
    context: Context,
    value: AccountValue,
}

/// Solana context information
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Context {
    slot: u64,
}

/// Solana account value structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AccountValue {
    data: Option<serde_json::Value>,
    executable: bool,
    lamports: u64,
    owner: String,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
}

/// Configuration for Solana WebSocket client
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SolanaConfig {
    /// RPC providers with priority ordering
    pub rpc_providers: Vec<RpcProvider>,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Reconnection configuration
    pub reconnect_config: ReconnectConfig,
    /// Account address to monitor for price data
    pub account_address: Option<String>,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_providers: vec![
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
            ],
            connection_timeout: Duration::from_secs(10),
            reconnect_config: ReconnectConfig::default(),
            account_address: None,
        }
    }
}

impl SolanaConfig {
    /// Create new Solana configuration with custom providers
    #[allow(dead_code)]
    pub fn new(rpc_providers: Vec<RpcProvider>, connection_timeout: Duration) -> Self {
        Self {
            rpc_providers,
            connection_timeout,
            reconnect_config: ReconnectConfig::default(),
            account_address: None,
        }
    }

    /// Set reconnection configuration
    #[allow(dead_code)]
    pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }

    /// Set account address to monitor
    #[allow(dead_code)]
    pub fn with_account_address(mut self, address: String) -> Self {
        self.account_address = Some(address);
        self
    }
}

/// Solana WebSocket client for real-time price data from DEX pools
#[allow(dead_code)]
pub struct SolanaClient {
    config: SolanaConfig,
    trading_pair: TradingPair,
    reconnect_handler: ReconnectHandler,
    current_provider_index: usize,
}

impl SolanaClient {
    /// Create new Solana WebSocket client
    #[allow(dead_code)]
    pub fn new(config: SolanaConfig, trading_pair: TradingPair) -> Result<Self, SolanaError> {
        if config.rpc_providers.is_empty() {
            return Err(SolanaError::NoProvidersAvailable);
        }

        let reconnect_handler =
            ReconnectHandler::new(config.reconnect_config.clone()).map_err(|e| {
                SolanaError::JsonError(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    e,
                )))
            })?;

        Ok(Self {
            config,
            trading_pair,
            reconnect_handler,
            current_provider_index: 0,
        })
    }

    /// Create client with default configuration
    #[allow(dead_code)]
    pub fn with_default(trading_pair: TradingPair) -> Result<Self, SolanaError> {
        Self::new(SolanaConfig::default(), trading_pair)
    }

    /// Create client from RPC providers
    #[allow(dead_code)]
    pub fn from_providers(
        rpc_providers: Vec<RpcProvider>,
        trading_pair: TradingPair,
    ) -> Result<Self, SolanaError> {
        let config = SolanaConfig::new(rpc_providers, Duration::from_secs(10));
        Self::new(config, trading_pair)
    }

    /// Start the WebSocket client and stream price updates
    #[allow(dead_code)]
    pub async fn start<F>(&mut self, mut callback: F) -> Result<(), SolanaError>
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
                    eprintln!("Solana WebSocket error: {}", e);

                    // Try next provider if available
                    if self.try_next_provider() {
                        eprintln!(
                            "Switching to provider: {}",
                            self.get_current_provider().name
                        );
                        continue;
                    }

                    // Determine if we should reconnect
                    match self.reconnect_handler.should_reconnect() {
                        Ok(delay) => {
                            eprintln!(
                                "Reconnecting to Solana in {:?} (attempt {})",
                                delay,
                                self.reconnect_handler.attempt_count()
                            );
                            sleep(delay).await;
                            // Reset provider index for retry
                            self.current_provider_index = 0;
                        }
                        Err(reconnect_error) => {
                            eprintln!("Giving up on Solana reconnection: {}", reconnect_error);
                            return Err(SolanaError::ReconnectFailed(reconnect_error));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Connect to Solana WebSocket and stream data
    #[allow(dead_code)]
    async fn connect_and_stream<F>(&self, callback: &mut F) -> Result<(), SolanaError>
    where
        F: FnMut(PriceUpdate) + Send,
    {
        let provider = self.get_current_provider();
        let url = &provider.websocket_url;

        eprintln!("Connecting to Solana via: {}", provider.name);

        // Connect with timeout
        let (ws_stream, _) = timeout(self.config.connection_timeout, connect_async(url))
            .await
            .map_err(|_| SolanaError::Timeout(self.config.connection_timeout))?
            .map_err(|e| SolanaError::ConnectionError(Box::new(e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to account updates (mock implementation for demo)
        let subscribe_msg = self.create_account_subscribe_message()?;
        let msg_text = serde_json::to_string(&subscribe_msg)?;
        write
            .send(Message::Text(msg_text))
            .await
            .map_err(|e| SolanaError::ConnectionError(Box::new(e)))?;

        // Process incoming messages
        while let Some(message) = read.next().await {
            match message.map_err(|e| SolanaError::ConnectionError(Box::new(e)))? {
                Message::Text(text) => {
                    if let Ok(price_update) = self.parse_account_message(&text) {
                        callback(price_update);
                    }
                }
                Message::Ping(payload) => {
                    write
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|e| SolanaError::ConnectionError(Box::new(e)))?;
                }
                Message::Close(_) => {
                    eprintln!("Solana WebSocket connection closed");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Get current RPC provider
    fn get_current_provider(&self) -> &RpcProvider {
        &self.config.rpc_providers[self.current_provider_index]
    }

    /// Try to switch to next available provider
    fn try_next_provider(&mut self) -> bool {
        if self.current_provider_index + 1 < self.config.rpc_providers.len() {
            self.current_provider_index += 1;
            true
        } else {
            false
        }
    }

    /// Create account subscription message
    fn create_account_subscribe_message(&self) -> Result<AccountSubscribeRequest, SolanaError> {
        // Mock account address - in real implementation this would be
        // the actual pool account for the trading pair
        let account_address = self
            .config
            .account_address
            .as_ref()
            .unwrap_or(&self.get_mock_pool_address()?)
            .clone();

        let params = serde_json::json!([
            account_address,
            {
                "encoding": "jsonParsed",
                "commitment": "confirmed"
            }
        ]);

        Ok(AccountSubscribeRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "accountSubscribe".to_string(),
            params,
        })
    }

    /// Parse account message and convert to PriceUpdate
    fn parse_account_message(&self, text: &str) -> Result<PriceUpdate, SolanaError> {
        // First try to parse as account notification
        if let Ok(notification) = serde_json::from_str::<AccountNotification>(text) {
            return self.extract_price_from_account_data(&notification);
        }

        // If not a notification, it might be a subscription confirmation
        if text.contains("result") && text.contains("subscription") {
            // This is likely a subscription confirmation - ignore for now
            return Err(SolanaError::InvalidAccountData);
        }

        Err(SolanaError::InvalidAccountData)
    }

    /// Extract price from account data (simplified implementation)
    fn extract_price_from_account_data(
        &self,
        notification: &AccountNotification,
    ) -> Result<PriceUpdate, SolanaError> {
        // This is a simplified mock implementation
        // In a real implementation, you would parse the actual DEX pool data
        // to extract token prices, reserves, etc.

        // Mock price extraction - generate a realistic price for demonstration
        let mock_price = match self.trading_pair {
            TradingPair::SolUsdt => 195.0 + (notification.result.context.slot as f64 % 10.0),
            TradingPair::SolUsdc => 194.8 + (notification.result.context.slot as f64 % 10.0),
        };

        Ok(PriceUpdate::new(
            PriceSource::Solana,
            self.trading_pair,
            mock_price,
        ))
    }

    /// Get mock pool address for trading pair
    fn get_mock_pool_address(&self) -> Result<String, SolanaError> {
        match self.trading_pair {
            TradingPair::SolUsdt => {
                Ok("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".to_string())
            }
            TradingPair::SolUsdc => {
                Ok("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string())
            }
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

    /// Get current provider name
    #[allow(dead_code)]
    pub fn current_provider_name(&self) -> &str {
        &self.get_current_provider().name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::reconnect::ReconnectConfig;

    #[test]
    fn test_solana_config_creation() {
        let config = SolanaConfig::default();
        assert!(!config.rpc_providers.is_empty());
        assert_eq!(config.connection_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_solana_client_creation() {
        let config = SolanaConfig::default();
        let client = SolanaClient::new(config, TradingPair::SolUsdt);
        assert!(client.is_ok());

        let default_client = SolanaClient::with_default(TradingPair::SolUsdc);
        assert!(default_client.is_ok());
    }

    #[test]
    fn test_empty_providers_error() {
        let config = SolanaConfig::new(vec![], Duration::from_secs(10));
        let client = SolanaClient::new(config, TradingPair::SolUsdt);
        assert!(matches!(client, Err(SolanaError::NoProvidersAvailable)));
    }

    #[test]
    fn test_mock_pool_addresses() {
        let client = SolanaClient::with_default(TradingPair::SolUsdt).unwrap();
        assert!(client.get_mock_pool_address().is_ok());

        let client2 = SolanaClient::with_default(TradingPair::SolUsdc).unwrap();
        assert!(client2.get_mock_pool_address().is_ok());
    }

    #[test]
    fn test_account_subscribe_message_creation() {
        let client = SolanaClient::with_default(TradingPair::SolUsdt).unwrap();
        let msg = client.create_account_subscribe_message().unwrap();

        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.method, "accountSubscribe");
        assert_eq!(msg.id, 1);
    }

    #[test]
    fn test_provider_switching() {
        let providers = vec![
            RpcProvider {
                name: "Provider1".to_string(),
                websocket_url: "wss://provider1.com".parse().unwrap(),
                priority: 1,
            },
            RpcProvider {
                name: "Provider2".to_string(),
                websocket_url: "wss://provider2.com".parse().unwrap(),
                priority: 2,
            },
        ];

        let config = SolanaConfig::new(providers, Duration::from_secs(10));
        let mut client = SolanaClient::new(config, TradingPair::SolUsdt).unwrap();

        assert_eq!(client.current_provider_name(), "Provider1");
        assert!(client.try_next_provider());
        assert_eq!(client.current_provider_name(), "Provider2");
        assert!(!client.try_next_provider());
    }

    #[test]
    fn test_config_with_reconnect_settings() {
        let reconnect_config =
            ReconnectConfig::new(Duration::from_millis(500), Duration::from_secs(30), 1.5);

        let solana_config = SolanaConfig::default().with_reconnect_config(reconnect_config);

        let client = SolanaClient::new(solana_config, TradingPair::SolUsdt).unwrap();

        assert_eq!(client.reconnect_attempts(), 0);
        assert!(client.reconnect_elapsed_time().is_none());
    }

    #[test]
    fn test_account_address_configuration() {
        let custom_address = "CustomPoolAddress123456789".to_string();
        let config = SolanaConfig::default().with_account_address(custom_address.clone());

        assert_eq!(config.account_address, Some(custom_address));
    }

    #[test]
    fn test_price_extraction_mock() {
        let client = SolanaClient::with_default(TradingPair::SolUsdt).unwrap();

        let notification = AccountNotification {
            subscription: 123,
            result: AccountData {
                context: Context { slot: 100 },
                value: AccountValue {
                    data: None,
                    executable: false,
                    lamports: 1000000,
                    owner: "11111111111111111111111111111111".to_string(),
                    rent_epoch: 300,
                },
            },
        };

        let price_update = client.extract_price_from_account_data(&notification).unwrap();
        assert_eq!(price_update.source, PriceSource::Solana);
        assert_eq!(price_update.pair, TradingPair::SolUsdt);
        assert!(price_update.price > 0.0);
    }
}