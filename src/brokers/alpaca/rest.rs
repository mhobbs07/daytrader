//! Alpaca REST API client for order execution

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use std::time::Duration;

use super::config::AlpacaConfig;
use super::types::{AlpacaAccount, AlpacaOrder, AlpacaPosition, PlaceOrderRequest};
use crate::brokers::traits::{
    Account, Candle, Order, OrderExecutionProvider, Position, Timeframe,
};
use crate::brokers::types::{OrderId, Reason, Symbol};
use crate::utils::errors::BrokerError;

/// Alpaca REST API client
#[derive(Debug, Clone)]
pub struct RestClient {
    config: AlpacaConfig,
    client: Client,
}

impl RestClient {
    /// Creates a new REST client
    pub fn new(config: AlpacaConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Adds authentication headers to a request builder
    fn add_auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
    }

    /// Makes a GET request to the Alpaca API
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, BrokerError> {
        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.get(&url);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Makes a POST request to the Alpaca API
    async fn post<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, BrokerError> {
        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.post(&url).json(body);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Makes a DELETE request to the Alpaca API
    async fn delete(&self, path: &str) -> Result<(), BrokerError> {
        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.delete(&url);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(self.map_error(status, &error_text))
        }
    }

    /// Handles API response and deserializes or returns error
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, BrokerError> {
        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|e| BrokerError::ApiError(format!("Failed to parse response: {}", e)))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(self.map_error(status, &error_text))
        }
    }

    /// Maps HTTP status codes to BrokerError
    fn map_error(&self, status: StatusCode, message: &str) -> BrokerError {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                BrokerError::AuthenticationFailed(message.to_string())
            }
            StatusCode::NOT_FOUND => BrokerError::SymbolNotFound(message.to_string()),
            StatusCode::TOO_MANY_REQUESTS => {
                // Try to parse retry-after from message, default to 60 seconds
                let retry_after = Duration::from_secs(60);
                BrokerError::RateLimitExceeded { retry_after }
            }
            StatusCode::BAD_REQUEST => {
                // Check if it's an order rejection
                if message.contains("insufficient") {
                    BrokerError::InsufficientFunds {
                        required: rust_decimal::Decimal::ZERO,
                        available: rust_decimal::Decimal::ZERO,
                    }
                } else {
                    BrokerError::OrderRejected {
                        reason: Reason::new(message),
                    }
                }
            }
            _ => BrokerError::ApiError(format!("HTTP {}: {}", status, message)),
        }
    }
}

#[async_trait]
impl OrderExecutionProvider for RestClient {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError> {
        let request = PlaceOrderRequest::from_order(&order);
        let response: AlpacaOrder = self.post("/v2/orders", &request).await?;
        Ok(OrderId::new(response.id))
    }

    async fn cancel_order(&self, order_id: &OrderId) -> Result<(), BrokerError> {
        let path = format!("/v2/orders/{}", order_id.as_str());
        self.delete(&path).await
    }

    async fn get_order(&self, order_id: &OrderId) -> Result<Order, BrokerError> {
        let path = format!("/v2/orders/{}", order_id.as_str());
        let response: AlpacaOrder = self.get(&path).await?;
        response.into_order()
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        let response: Vec<AlpacaOrder> = self.get("/v2/orders?status=open").await?;
        response
            .into_iter()
            .map(|o| o.into_order())
            .collect::<Result<Vec<_>, _>>()
    }

    async fn get_account(&self) -> Result<Account, BrokerError> {
        let response: AlpacaAccount = self.get("/v2/account").await?;
        response.into_account()
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let response: Vec<AlpacaPosition> = self.get("/v2/positions").await?;
        response
            .into_iter()
            .map(|p| p.into_position())
            .collect::<Result<Vec<_>, _>>()
    }

    async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, BrokerError> {
        let path = format!("/v2/positions/{}", symbol.as_str());
        match self.get::<AlpacaPosition>(&path).await {
            Ok(position) => Ok(Some(position.into_position()?)),
            Err(BrokerError::SymbolNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{OrderSide, OrderType, TimeInForce};
    use crate::brokers::types::{Price, Quantity};
    use rust_decimal::Decimal;

    fn test_config() -> AlpacaConfig {
        AlpacaConfig::paper().with_credentials(
            "test_key".to_string(),
            "test_secret".to_string(),
        )
    }

    #[test]
    fn test_client_creation() {
        let config = test_config();
        let client = RestClient::new(config.clone());
        assert_eq!(client.config.api_key, config.api_key);
    }

    #[test]
    fn test_place_order_request_from_order() {
        let symbol = Symbol::new("AAPL").unwrap();
        let qty = Quantity::new(100).unwrap();
        let price = Price::new(Decimal::new(15000, 2)).unwrap();

        let order = Order::limit(symbol, OrderSide::Buy, qty, price);
        let request = PlaceOrderRequest::from_order(&order);

        assert_eq!(request.symbol, "AAPL");
        assert_eq!(request.qty, "100");
        assert_eq!(request.side, "buy");
        assert_eq!(request.order_type, "limit");
        assert_eq!(request.limit_price, Some("150.00".to_string()));
    }

    #[tokio::test]
    async fn test_map_error() {
        let config = test_config();
        let client = RestClient::new(config);

        let error = client.map_error(StatusCode::UNAUTHORIZED, "Invalid credentials");
        assert!(matches!(error, BrokerError::AuthenticationFailed(_)));

        let error = client.map_error(StatusCode::NOT_FOUND, "Symbol not found");
        assert!(matches!(error, BrokerError::SymbolNotFound(_)));

        let error = client.map_error(StatusCode::TOO_MANY_REQUESTS, "Rate limit");
        assert!(matches!(error, BrokerError::RateLimitExceeded { .. }));
    }

    // Note: Integration tests with actual API calls would go in tests/ directory
    // and would use paper trading credentials from environment variables
}