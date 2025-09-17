use crate::config::{RpcProvider, TradingPair};
use crate::price::{PriceSource, PriceUpdate};
use crate::websocket::reconnect::{ReconnectConfig, ReconnectError, ReconnectHandler};
use base64::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
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
    #[error("Pool data parsing error: {0}")]
    PoolParsingError(String),
}

/// Simplified Raydium AMM pool state for price extraction
/// Based on Raydium LIQUIDITY_STATE_LAYOUT_V4 structure
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[allow(dead_code)]
pub struct RaydiumPoolState {
    /// Pool status (should be 6 for active pools)
    pub status: u64,
    /// Pool nonce for PDA derivation
    pub nonce: u64,
    /// Max order volume
    pub max_order: u64,
    /// Pool depth
    pub depth: u64,
    /// Base token decimals
    pub base_decimals: u64,
    /// Quote token decimals
    pub quote_decimals: u64,
    /// Pool state (should be 1 for initialized)
    pub state: u64,
    /// Reset flag
    pub reset_flag: u64,
    /// Minimum size
    pub min_size: u64,
    /// Volume multiplier
    pub vol_max_cut_ratio: u64,
    /// Amount wave ratio
    pub amount_wave_ratio: u64,
    /// Base lot size
    pub base_lot_size: u64,
    /// Quote lot size
    pub quote_lot_size: u64,
    /// Minimum price multiplier
    pub min_price_multiplier: u64,
    /// Maximum price multiplier
    pub max_price_multiplier: u64,
    /// System decimals value
    pub system_decimals_value: u64,
    /// Minimum separate numerator
    pub min_separate_numerator: u64,
    /// Minimum separate denominator
    pub min_separate_denominator: u64,
    /// Trade fee numerator
    pub trade_fee_numerator: u64,
    /// Trade fee denominator
    pub trade_fee_denominator: u64,
    /// Pnl numerator
    pub pnl_numerator: u64,
    /// Pnl denominator
    pub pnl_denominator: u64,
    /// Swap fee numerator
    pub swap_fee_numerator: u64,
    /// Swap fee denominator
    pub swap_fee_denominator: u64,
    /// Base need take pnl
    pub base_need_take_pnl: u64,
    /// Quote need take pnl
    pub quote_need_take_pnl: u64,
    /// Quote total pnl
    pub quote_total_pnl: u64,
    /// Base total pnl
    pub base_total_pnl: u64,
    /// Pool total deposited base amount
    pub pool_base_token_amount: u64,
    /// Pool total deposited quote amount
    pub pool_quote_token_amount: u64,
    /// Swap base in amount
    pub swap_base_in_amount: u64,
    /// Swap quote out amount
    pub swap_quote_out_amount: u64,
    /// Swap base out amount
    pub swap_base_out_amount: u64,
    /// Swap quote in amount
    pub swap_quote_in_amount: u64,
    /// Base vault key (32 bytes)
    pub base_vault: [u8; 32],
    /// Quote vault key (32 bytes)
    pub quote_vault: [u8; 32],
    /// Base mint key (32 bytes)
    pub base_mint: [u8; 32],
    /// Quote mint key (32 bytes)
    pub quote_mint: [u8; 32],
    /// LP mint key (32 bytes)
    pub lp_mint: [u8; 32],
    /// OpenBook market key (32 bytes)
    pub open_orders: [u8; 32],
    /// Market key (32 bytes)
    pub market_id: [u8; 32],
    /// Market base vault (32 bytes)
    pub market_base_vault: [u8; 32],
    /// Market quote vault (32 bytes)
    pub market_quote_vault: [u8; 32],
    /// Market authority (32 bytes)
    pub market_authority: [u8; 32],
    /// Withdraw queue key (32 bytes)
    pub withdraw_queue: [u8; 32],
    /// LP vault key (32 bytes)
    pub lp_vault: [u8; 32],
    /// Owner key (32 bytes)
    pub owner: [u8; 32],
    /// LP reserve (u64 but using 8 bytes)
    pub lp_reserve: u64,
    /// Padding to match expected layout
    pub padding: [u8; 7],
}

impl RaydiumPoolState {
    /// Calculate price of base token in terms of quote token
    /// Price = quote_amount / base_amount
    pub fn calculate_price(&self) -> Result<f64, SolanaError> {
        if self.pool_base_token_amount == 0 {
            return Err(SolanaError::PoolParsingError("Base token amount is zero".to_string()));
        }

        // Convert token amounts to f64 accounting for decimals
        let base_amount = self.pool_base_token_amount as f64 / 10f64.powi(self.base_decimals as i32);
        let quote_amount = self.pool_quote_token_amount as f64 / 10f64.powi(self.quote_decimals as i32);

        if base_amount == 0.0 {
            return Err(SolanaError::PoolParsingError("Calculated base amount is zero".to_string()));
        }

        Ok(quote_amount / base_amount)
    }

    /// Validate that this is an active pool
    pub fn is_active(&self) -> bool {
        self.status == 6 && self.state == 1
    }
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
                    provider_type: crate::config::RpcProviderType::Helius,
                },
                RpcProvider {
                    name: "QuickNode".to_string(),
                    websocket_url: "wss://mainnet.solana.com"
                        .parse()
                        .expect("Invalid default RPC URL"),
                    priority: 2,
                    provider_type: crate::config::RpcProviderType::QuickNode,
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
                    log::error!("Solana WebSocket error: {}", e);

                    // Try next provider if available
                    if self.try_next_provider() {
                        log::info!(
                            "Switching to provider: {}",
                            self.get_current_provider().name
                        );
                        continue;
                    }

                    // Determine if we should reconnect
                    match self.reconnect_handler.should_reconnect() {
                        Ok(delay) => {
                            log::warn!(
                                "Reconnecting to Solana in {:?} (attempt {})",
                                delay,
                                self.reconnect_handler.attempt_count()
                            );
                            sleep(delay).await;
                            // Reset provider index for retry
                            self.current_provider_index = 0;
                        }
                        Err(reconnect_error) => {
                            log::error!("Giving up on Solana reconnection: {}", reconnect_error);
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

        log::info!("Connecting to Solana via: {}", provider.name);

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
                    log::info!("Solana WebSocket connection closed");
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
            .unwrap_or(&self.get_pool_address()?)
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

    /// Extract price from Raydium pool account data
    fn extract_price_from_account_data(
        &self,
        notification: &AccountNotification,
    ) -> Result<PriceUpdate, SolanaError> {
        // Extract base64 encoded account data
        let account_data = notification
            .result
            .value
            .data
            .as_ref()
            .and_then(|data| {
                // Account data can be returned as [data, encoding] array or as a string
                if let Some(array) = data.as_array() {
                    array.first().and_then(|v| v.as_str())
                } else {
                    data.as_str()
                }
            })
            .ok_or(SolanaError::InvalidAccountData)?;

        // Decode base64 data
        let decoded_data = BASE64_STANDARD.decode(account_data)
            .map_err(|e| SolanaError::PoolParsingError(format!("Base64 decode error: {}", e)))?;

        // Try to deserialize as Raydium pool state
        let pool_state = match RaydiumPoolState::try_from_slice(&decoded_data) {
            Ok(state) => state,
            Err(_e) => {
                // If full deserialization fails, try to extract just the pool amounts
                // This is a fallback approach for when the struct doesn't match exactly
                return self.extract_price_from_raw_data(&decoded_data);
            }
        };

        // Validate that this is an active pool
        if !pool_state.is_active() {
            return Err(SolanaError::PoolParsingError(
                "Pool is not active".to_string()
            ));
        }

        // Calculate price from pool reserves
        let price = pool_state.calculate_price()?;

        Ok(PriceUpdate::new(
            PriceSource::Solana,
            self.trading_pair,
            price,
        ))
    }

    /// Fallback method to extract price from raw account data
    /// This attempts to read just the pool token amounts from known offsets
    fn extract_price_from_raw_data(&self, data: &[u8]) -> Result<PriceUpdate, SolanaError> {
        // Based on Raydium pool layout, token amounts are typically at specific offsets
        // This is a simplified extraction focusing on the pool reserves
        if data.len() < 400 {
            return Err(SolanaError::PoolParsingError(
                "Account data too short for pool state".to_string()
            ));
        }

        // Attempt to read pool token amounts from expected offsets
        // These offsets are based on the Raydium LIQUIDITY_STATE_LAYOUT_V4
        let base_amount_offset = 232; // Approximate offset for pool_base_token_amount
        let quote_amount_offset = 240; // Approximate offset for pool_quote_token_amount

        if data.len() < quote_amount_offset + 8 {
            return Err(SolanaError::PoolParsingError(
                "Insufficient data for token amounts".to_string()
            ));
        }

        // Read u64 values in little-endian format
        let base_amount = u64::from_le_bytes([
            data[base_amount_offset],
            data[base_amount_offset + 1],
            data[base_amount_offset + 2],
            data[base_amount_offset + 3],
            data[base_amount_offset + 4],
            data[base_amount_offset + 5],
            data[base_amount_offset + 6],
            data[base_amount_offset + 7],
        ]);

        let quote_amount = u64::from_le_bytes([
            data[quote_amount_offset],
            data[quote_amount_offset + 1],
            data[quote_amount_offset + 2],
            data[quote_amount_offset + 3],
            data[quote_amount_offset + 4],
            data[quote_amount_offset + 5],
            data[quote_amount_offset + 6],
            data[quote_amount_offset + 7],
        ]);

        if base_amount == 0 {
            return Err(SolanaError::PoolParsingError(
                "Base token amount is zero".to_string()
            ));
        }

        // Calculate price with standard Solana token decimals
        // SOL has 9 decimals, USDT/USDC typically have 6 decimals
        let base_decimals = 9; // SOL
        let quote_decimals = match self.trading_pair {
            TradingPair::SolUsdt => 6, // USDT decimals
            TradingPair::SolUsdc => 6, // USDC decimals
        };

        let base_amount_f64 = base_amount as f64 / 10f64.powi(base_decimals);
        let quote_amount_f64 = quote_amount as f64 / 10f64.powi(quote_decimals);

        if base_amount_f64 == 0.0 {
            return Err(SolanaError::PoolParsingError(
                "Calculated base amount is zero".to_string()
            ));
        }

        let price = quote_amount_f64 / base_amount_f64;

        // Sanity check - SOL price should be reasonable (between $10 and $1000)
        if price < 10.0 || price > 1000.0 {
            return Err(SolanaError::PoolParsingError(
                format!("Calculated price {} seems unreasonable", price)
            ));
        }

        Ok(PriceUpdate::new(
            PriceSource::Solana,
            self.trading_pair,
            price,
        ))
    }

    /// Get real Raydium pool address for trading pair
    fn get_pool_address(&self) -> Result<String, SolanaError> {
        match self.trading_pair {
            // Real Raydium SOL/USDT pool address (mainnet)
            TradingPair::SolUsdt => Ok("7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX".to_string()),
            // Real Raydium SOL/USDC pool address (mainnet)
            TradingPair::SolUsdc => Ok("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string()),
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
    fn test_pool_addresses() {
        let client = SolanaClient::with_default(TradingPair::SolUsdt).unwrap();
        assert!(client.get_pool_address().is_ok());
        assert_eq!(client.get_pool_address().unwrap(), "7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX");

        let client2 = SolanaClient::with_default(TradingPair::SolUsdc).unwrap();
        assert!(client2.get_pool_address().is_ok());
        assert_eq!(client2.get_pool_address().unwrap(), "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2");
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
    fn test_price_extraction_fallback() {
        let client = SolanaClient::with_default(TradingPair::SolUsdt).unwrap();

        // Create mock account data that will trigger fallback parsing
        // This simulates a base64-encoded account with minimal pool data
        let mut mock_data = vec![0u8; 400];

        // Set mock pool amounts at expected offsets
        let base_amount: u64 = 1000000000000000; // 1 million SOL (with 9 decimals)
        let quote_amount: u64 = 200000000000000; // 200 million USDT (with 6 decimals)

        // Write base amount at offset 232
        mock_data[232..240].copy_from_slice(&base_amount.to_le_bytes());
        // Write quote amount at offset 240
        mock_data[240..248].copy_from_slice(&quote_amount.to_le_bytes());

        let encoded_data = BASE64_STANDARD.encode(&mock_data);

        let notification = AccountNotification {
            subscription: 123,
            result: AccountData {
                context: Context { slot: 100 },
                value: AccountValue {
                    data: Some(serde_json::Value::String(encoded_data)),
                    executable: false,
                    lamports: 1000000,
                    owner: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
                    rent_epoch: 300,
                },
            },
        };

        let price_update = client
            .extract_price_from_account_data(&notification)
            .unwrap();
        assert_eq!(price_update.source, PriceSource::Solana);
        assert_eq!(price_update.pair, TradingPair::SolUsdt);
        assert!(price_update.price > 0.0);
        // Expected price: 200M / 1M = 200 USDT per SOL
        assert!((price_update.price - 200.0).abs() < 0.1);
    }
}
