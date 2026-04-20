//! Alpaca broker integration
//!
//! This module provides integration with Alpaca Markets for both
//! order execution (REST API) and real-time market data (WebSocket).

pub mod config;
pub mod rest;
pub mod types;
pub mod websocket;

pub use config::AlpacaConfig;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::brokers::traits::{
    Account, Candle, MarketDataProvider, Order, OrderExecutionProvider, Position, Quote, Timeframe,
};
use crate::brokers::types::{OrderId, Symbol};
use crate::utils::errors::BrokerError;

use rest::RestClient;
use websocket::WebSocketClient;

/// Alpaca broker provider combining REST and WebSocket
///
/// This struct provides a unified interface to Alpaca Markets,
/// implementing both order execution and market data traits.
///
/// # Examples
///
/// ```no_run
/// use daytrader::brokers::alpaca::{AlpacaProvider, AlpacaConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AlpacaConfig::paper()
///         .with_credentials("key".to_string(), "secret".to_string());
///     
///     let mut provider = AlpacaProvider::new(config);
///     provider.connect_websocket().await?;
///     
///     // Use provider for trading and market data
///     let account = provider.get_account().await?;
///     println!("Buying power: {}", account.buying_power);
///     
///     Ok(())
/// }
/// ```
pub struct AlpacaProvider {
    config: AlpacaConfig,
    rest_client: RestClient,
    ws_client: Option<WebSocketClient>,
}

impl AlpacaProvider {
    /// Creates a new Alpaca provider
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::brokers::alpaca::{AlpacaProvider, AlpacaConfig};
    ///
    /// let config = AlpacaConfig::paper();
    /// let provider = AlpacaProvider::new(config);
    /// ```
    pub fn new(config: AlpacaConfig) -> Self {
        Self {
            rest_client: RestClient::new(config.clone()),
            ws_client: None,
            config,
        }
    }

    /// Connects the WebSocket client for real-time market data
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket connection fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use daytrader::brokers::alpaca::{AlpacaProvider, AlpacaConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AlpacaConfig::paper();
    /// let mut provider = AlpacaProvider::new(config);
    /// provider.connect_websocket().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_websocket(&mut self) -> Result<(), BrokerError> {
        let ws = WebSocketClient::connect(&self.config).await?;
        self.ws_client = Some(ws);
        Ok(())
    }

    /// Returns whether the WebSocket is connected
    pub fn is_websocket_connected(&self) -> bool {
        self.ws_client.is_some()
    }

    /// Returns a reference to the REST client
    pub fn rest_client(&self) -> &RestClient {
        &self.rest_client
    }

    /// Returns a reference to the WebSocket client if connected
    pub fn ws_client(&self) -> Option<&WebSocketClient> {
        self.ws_client.as_ref()
    }
}

// Implement OrderExecutionProvider by delegating to REST client
#[async_trait]
impl OrderExecutionProvider for AlpacaProvider {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError> {
        self.rest_client.place_order(order).await
    }

    async fn cancel_order(&self, order_id: &OrderId) -> Result<(), BrokerError> {
        self.rest_client.cancel_order(order_id).await
    }

    async fn get_order(&self, order_id: &OrderId) -> Result<Order, BrokerError> {
        self.rest_client.get_order(order_id).await
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        self.rest_client.get_open_orders().await
    }

    async fn get_account(&self) -> Result<Account, BrokerError> {
        self.rest_client.get_account().await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.rest_client.get_positions().await
    }

    async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, BrokerError> {
        self.rest_client.get_position(symbol).await
    }
}

// Implement MarketDataProvider by delegating to WebSocket client
#[async_trait]
impl MarketDataProvider for AlpacaProvider {
    async fn subscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        match &self.ws_client {
            Some(ws) => ws.subscribe_quotes(symbols).await,
            None => Err(BrokerError::ConnectionFailed(
                "WebSocket not connected. Call connect_websocket() first.".to_string(),
            )),
        }
    }

    async fn unsubscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        match &self.ws_client {
            Some(ws) => ws.unsubscribe_quotes(symbols).await,
            None => Err(BrokerError::ConnectionFailed(
                "WebSocket not connected".to_string(),
            )),
        }
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        match &self.ws_client {
            Some(ws) => ws.get_quote(symbol).await,
            None => Err(BrokerError::ConnectionFailed(
                "WebSocket not connected".to_string(),
            )),
        }
    }

    async fn get_candles(
        &self,
        symbol: &Symbol,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError> {
        // Historical candles are fetched via REST API
        // This would need to be implemented in the REST client
        // For now, return an error
        Err(BrokerError::ApiError(
            "Historical candles not yet implemented".to_string(),
        ))
    }

    async fn start_stream(&self) -> Result<(), BrokerError> {
        match &self.ws_client {
            Some(ws) => ws.start_stream().await,
            None => Err(BrokerError::ConnectionFailed(
                "WebSocket not connected".to_string(),
            )),
        }
    }

    async fn stop_stream(&self) -> Result<(), BrokerError> {
        match &self.ws_client {
            Some(ws) => ws.stop_stream().await,
            None => Ok(()), // Already stopped
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AlpacaConfig {
        AlpacaConfig::paper().with_credentials("test_key".to_string(), "test_secret".to_string())
    }

    #[test]
    fn test_provider_creation() {
        let config = test_config();
        let provider = AlpacaProvider::new(config);
        assert!(!provider.is_websocket_connected());
    }

    #[test]
    fn test_rest_client_access() {
        let config = test_config();
        let provider = AlpacaProvider::new(config);
        let _rest = provider.rest_client();
        // Just verify we can access it
    }

    #[tokio::test]
    async fn test_websocket_not_connected_error() {
        let config = test_config();
        let provider = AlpacaProvider::new(config);

        let symbol = Symbol::new("AAPL").unwrap();
        let result = provider.get_quote(&symbol).await;
        assert!(result.is_err());

        if let Err(BrokerError::ConnectionFailed(msg)) = result {
            assert!(msg.contains("WebSocket not connected"));
        } else {
            panic!("Expected ConnectionFailed error");
        }
    }
}
