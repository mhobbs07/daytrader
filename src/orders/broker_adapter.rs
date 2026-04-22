//! Broker adapter for order submission and status synchronization
//!
//! This module provides a trait-based abstraction for broker integration
//! with a concrete implementation for Alpaca.

use crate::brokers::traits::{Order, OrderExecutionProvider, OrderStatus};
use crate::brokers::types::OrderId;
use crate::orders::types::{ManagedOrder, OrderError};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, info};

// ============================================================================
// Broker Adapter Trait
// ============================================================================

/// Trait for broker order adapter
///
/// Abstracts broker-specific order submission and status synchronization.
/// Implementations handle format conversions and broker-specific logic.
#[async_trait]
pub trait BrokerAdapter: Send + Sync {
    /// Submits an order to the broker
    ///
    /// # Errors
    ///
    /// Returns an error if submission fails or broker rejects the order
    async fn submit_order(&self, order: &ManagedOrder) -> Result<String, OrderError>;

    /// Cancels an order with the broker
    ///
    /// # Errors
    ///
    /// Returns an error if cancellation fails
    async fn cancel_order(&self, broker_order_id: &str) -> Result<(), OrderError>;

    /// Synchronizes order status from broker
    ///
    /// # Errors
    ///
    /// Returns an error if status query fails
    async fn sync_order_status(&self, broker_order_id: &str) -> Result<OrderStatus, OrderError>;

    /// Gets order details from broker
    ///
    /// # Errors
    ///
    /// Returns an error if order is not found
    async fn get_broker_order(&self, broker_order_id: &str) -> Result<Order, OrderError>;
}

// ============================================================================
// Alpaca Adapter Implementation
// ============================================================================

/// Alpaca broker adapter
///
/// Implements order submission and status tracking for Alpaca's REST API.
///
/// # Examples
///
/// ```no_run
/// use daytrader::orders::broker_adapter::AlpacaAdapter;
/// use daytrader::brokers::alpaca::rest::AlpacaRestClient;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let alpaca_client = Arc::new(AlpacaRestClient::new(
///     "api_key".to_string(),
///     "api_secret".to_string(),
///     "https://paper-api.alpaca.markets".to_string(),
/// ));
///
/// let adapter = AlpacaAdapter::new(alpaca_client);
/// # Ok(())
/// # }
/// ```
pub struct AlpacaAdapter {
    client: Arc<dyn OrderExecutionProvider>,
}

impl AlpacaAdapter {
    /// Creates a new Alpaca adapter
    pub fn new(client: Arc<dyn OrderExecutionProvider>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl BrokerAdapter for AlpacaAdapter {
    async fn submit_order(&self, order: &ManagedOrder) -> Result<String, OrderError> {
        info!(
            order_id = %order.id,
            symbol = %order.symbol,
            side = ?order.side,
            quantity = order.quantity.value(),
            order_type = ?order.order_type,
            "Submitting order to Alpaca"
        );

        // Convert to broker Order type
        let broker_order = order.to_broker_order();

        // Submit to broker
        let broker_order_id = self.client.place_order(broker_order).await?;

        debug!(
            order_id = %order.id,
            broker_order_id = %broker_order_id,
            "Order submitted successfully"
        );

        Ok(broker_order_id.into_inner())
    }

    async fn cancel_order(&self, broker_order_id: &str) -> Result<(), OrderError> {
        info!(
            broker_order_id = %broker_order_id,
            "Cancelling order with Alpaca"
        );

        self.client
            .cancel_order(&OrderId::new(broker_order_id))
            .await?;

        debug!(
            broker_order_id = %broker_order_id,
            "Order cancelled successfully"
        );

        Ok(())
    }

    async fn sync_order_status(&self, broker_order_id: &str) -> Result<OrderStatus, OrderError> {
        debug!(
            broker_order_id = %broker_order_id,
            "Syncing order status from Alpaca"
        );

        let order = self
            .client
            .get_order(&OrderId::new(broker_order_id))
            .await?;

        Ok(order.status)
    }

    async fn get_broker_order(&self, broker_order_id: &str) -> Result<Order, OrderError> {
        debug!(
            broker_order_id = %broker_order_id,
            "Fetching order details from Alpaca"
        );

        let order = self
            .client
            .get_order(&OrderId::new(broker_order_id))
            .await?;

        Ok(order)
    }
}

// ============================================================================
// Mock Adapter for Testing
// ============================================================================

#[cfg(test)]
/// Mock broker adapter for testing
pub struct MockBrokerAdapter {
    /// Simulates successful submissions
    pub should_succeed: bool,
    /// Simulated broker order ID prefix
    pub broker_order_id: String,
    /// Counter to generate unique ids per submit call
    submit_counter: std::sync::atomic::AtomicU64,
}

#[cfg(test)]
impl MockBrokerAdapter {
    /// Creates a new mock adapter that succeeds
    pub fn new_success() -> Self {
        Self {
            should_succeed: true,
            broker_order_id: "mock-broker-order".to_string(),
            submit_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Creates a new mock adapter that fails
    pub fn new_failure() -> Self {
        Self {
            should_succeed: false,
            broker_order_id: String::new(),
            submit_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl BrokerAdapter for MockBrokerAdapter {
    async fn submit_order(&self, _order: &ManagedOrder) -> Result<String, OrderError> {
        if self.should_succeed {
            let n = self
                .submit_counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(format!("{}-{}", self.broker_order_id, n))
        } else {
            Err(OrderError::BrokerRejection("Mock rejection".into()))
        }
    }

    async fn cancel_order(&self, _broker_order_id: &str) -> Result<(), OrderError> {
        if self.should_succeed {
            Ok(())
        } else {
            Err(OrderError::BrokerError(
                crate::utils::errors::BrokerError::ApiError("Cancel failed".into()),
            ))
        }
    }

    async fn sync_order_status(&self, _broker_order_id: &str) -> Result<OrderStatus, OrderError> {
        Ok(OrderStatus::Accepted)
    }

    async fn get_broker_order(&self, _broker_order_id: &str) -> Result<Order, OrderError> {
        use crate::brokers::types::{Price, Quantity, Symbol};
        use rust_decimal::Decimal;

        Ok(Order {
            id: OrderId::new(&self.broker_order_id),
            symbol: Symbol::new("AAPL").unwrap(),
            side: crate::brokers::traits::OrderSide::Buy,
            order_type: crate::brokers::traits::OrderType::Market,
            quantity: Quantity::new(100).unwrap(),
            limit_price: None,
            stop_price: None,
            trailing_amount: None,
            status: OrderStatus::Accepted,
            time_in_force: crate::brokers::traits::TimeInForce::Day,
            filled_quantity: 0,
            filled_avg_price: None,
            submitted_at: Some(chrono::Utc::now()),
            filled_at: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{OrderSide, OrderType};
    use crate::brokers::types::{Quantity, Symbol};
    use crate::orders::types::OrderRequest;

    fn create_test_order() -> ManagedOrder {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        ManagedOrder::from_request(OrderId::new("test-order"), request)
    }

    #[tokio::test]
    async fn test_mock_adapter_success() {
        let adapter = MockBrokerAdapter::new_success();
        let order = create_test_order();

        let result = adapter.submit_order(&order).await;
        assert!(result.is_ok());
        // Counter starts at 0, so first id is "-0"
        assert_eq!(result.unwrap(), "mock-broker-order-0");
    }

    #[tokio::test]
    async fn test_mock_adapter_failure() {
        let adapter = MockBrokerAdapter::new_failure();
        let order = create_test_order();

        let result = adapter.submit_order(&order).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_adapter_cancel() {
        let adapter = MockBrokerAdapter::new_success();

        let result = adapter.cancel_order("broker-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_adapter_sync_status() {
        let adapter = MockBrokerAdapter::new_success();

        let result = adapter.sync_order_status("broker-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), OrderStatus::Accepted);
    }
}
