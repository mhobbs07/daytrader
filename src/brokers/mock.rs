//! Mock broker implementation for testing
//!
//! Provides a configurable mock broker that can simulate various scenarios
//! including successful operations, errors, and delays.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

use super::traits::*;
use super::types::*;
use crate::utils::errors::BrokerError;

/// Configuration for mock broker behavior
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Simulated latency for operations (in milliseconds)
    pub latency_ms: u64,
    /// Whether to simulate errors
    pub simulate_errors: bool,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            simulate_errors: false,
            error_rate: 0.0,
        }
    }
}

/// Mock broker state
#[derive(Debug)]
struct MockState {
    /// Subscribed symbols
    subscribed_symbols: Vec<Symbol>,
    /// Mock quotes by symbol
    quotes: HashMap<String, Quote>,
    /// Mock candles by symbol
    candles: HashMap<String, Vec<Candle>>,
    /// Mock orders by ID
    orders: HashMap<String, Order>,
    /// Mock positions by symbol
    positions: HashMap<String, Position>,
    /// Mock account
    account: Account,
    /// Order ID counter
    next_order_id: u64,
    /// Whether stream is active
    stream_active: bool,
    /// Call history for verification
    call_history: Vec<String>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            subscribed_symbols: Vec::new(),
            quotes: HashMap::new(),
            candles: HashMap::new(),
            orders: HashMap::new(),
            positions: HashMap::new(),
            account: Account {
                account_number: "MOCK123".to_string(),
                buying_power: Decimal::new(10000000, 2), // $100,000
                cash: Decimal::new(10000000, 2),
                portfolio_value: Decimal::new(10000000, 2),
                day_trade_count: 0,
                is_pattern_day_trader: false,
                equity: Decimal::new(10000000, 2),
                last_equity: Decimal::new(10000000, 2),
            },
            next_order_id: 1,
            stream_active: false,
            call_history: Vec::new(),
        }
    }
}

/// Mock broker implementation
///
/// Provides a configurable mock for testing broker integrations.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::mock::{MockBroker, MockConfig};
/// use daytrader::brokers::traits::MarketDataProvider;
/// use daytrader::brokers::types::Symbol;
///
/// #[tokio::main]
/// async fn main() {
///     let broker = MockBroker::new(MockConfig::default());
///     
///     // Configure mock data
///     let symbol = Symbol::new("AAPL").unwrap();
///     broker.set_mock_quote(create_mock_quote(symbol.clone()));
///     
///     // Use like a real broker
///     let quote = broker.get_quote(&symbol).await.unwrap();
/// }
/// ```
pub struct MockBroker {
    config: MockConfig,
    state: Arc<RwLock<MockState>>,
}

impl MockBroker {
    /// Creates a new mock broker with the given configuration
    pub fn new(config: MockConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(MockState::default())),
        }
    }

    /// Creates a new mock broker with default configuration
    pub fn default() -> Self {
        Self::new(MockConfig::default())
    }

    /// Sets a mock quote for a symbol
    pub fn set_mock_quote(&self, quote: Quote) {
        let mut state = self.state.write();
        state
            .quotes
            .insert(quote.symbol.as_str().to_string(), quote);
    }

    /// Sets mock candles for a symbol
    pub fn set_mock_candles(&self, symbol: &Symbol, candles: Vec<Candle>) {
        let mut state = self.state.write();
        state.candles.insert(symbol.as_str().to_string(), candles);
    }

    /// Sets a mock position
    pub fn set_mock_position(&self, position: Position) {
        let mut state = self.state.write();
        state
            .positions
            .insert(position.symbol.as_str().to_string(), position);
    }

    /// Sets the mock account state
    pub fn set_mock_account(&self, account: Account) {
        let mut state = self.state.write();
        state.account = account;
    }

    /// Gets the call history for verification
    pub fn get_call_history(&self) -> Vec<String> {
        self.state.read().call_history.clone()
    }

    /// Clears the call history
    pub fn clear_call_history(&self) {
        self.state.write().call_history.clear();
    }

    /// Simulates latency if configured
    async fn simulate_latency(&self) {
        if self.config.latency_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.latency_ms)).await;
        }
    }

    /// Records a call for verification
    fn record_call(&self, method: impl Into<String>) {
        self.state.write().call_history.push(method.into());
    }

    /// Checks if an error should be simulated
    fn should_simulate_error(&self) -> bool {
        if !self.config.simulate_errors {
            return false;
        }
        rand::random::<f64>() < self.config.error_rate
    }
}

#[async_trait]
impl MarketDataProvider for MockBroker {
    async fn subscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("subscribe_quotes({:?})", symbols));

        if self.should_simulate_error() {
            return Err(BrokerError::ConnectionFailed("Mock error".to_string()));
        }

        let mut state = self.state.write();
        for symbol in symbols {
            if !state
                .subscribed_symbols
                .iter()
                .any(|s| s.as_str() == symbol.as_str())
            {
                state.subscribed_symbols.push(symbol);
            }
        }
        Ok(())
    }

    async fn unsubscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("unsubscribe_quotes({:?})", symbols));

        if self.should_simulate_error() {
            return Err(BrokerError::ConnectionFailed("Mock error".to_string()));
        }

        let mut state = self.state.write();
        state
            .subscribed_symbols
            .retain(|s| !symbols.iter().any(|sym| sym.as_str() == s.as_str()));
        Ok(())
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("get_quote({})", symbol));

        if self.should_simulate_error() {
            return Err(BrokerError::SymbolNotFound(symbol.to_string()));
        }

        let state = self.state.read();
        state
            .quotes
            .get(symbol.as_str())
            .cloned()
            .ok_or_else(|| BrokerError::SymbolNotFound(symbol.to_string()))
    }

    async fn get_candles(
        &self,
        symbol: &Symbol,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("get_candles({}, {:?})", symbol, timeframe));

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        let state = self.state.read();
        let candles = state
            .candles
            .get(symbol.as_str())
            .cloned()
            .unwrap_or_default();

        // Filter by time range
        let filtered: Vec<Candle> = candles
            .into_iter()
            .filter(|c| c.timestamp >= start && c.timestamp <= end)
            .collect();

        Ok(filtered)
    }

    async fn start_stream(&self) -> Result<(), BrokerError> {
        self.simulate_latency().await;
        self.record_call("start_stream");

        if self.should_simulate_error() {
            return Err(BrokerError::ConnectionFailed("Mock error".to_string()));
        }

        self.state.write().stream_active = true;
        Ok(())
    }

    async fn stop_stream(&self) -> Result<(), BrokerError> {
        self.simulate_latency().await;
        self.record_call("stop_stream");

        if self.should_simulate_error() {
            return Err(BrokerError::ConnectionFailed("Mock error".to_string()));
        }

        self.state.write().stream_active = false;
        Ok(())
    }
}

#[async_trait]
impl OrderExecutionProvider for MockBroker {
    async fn place_order(&self, mut order: Order) -> Result<OrderId, BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("place_order({:?})", order.symbol));

        if self.should_simulate_error() {
            return Err(BrokerError::OrderRejected {
                reason: Reason::new("Mock rejection"),
            });
        }

        let mut state = self.state.write();

        // Generate order ID
        let order_id = OrderId::new(format!("MOCK{}", state.next_order_id));
        state.next_order_id += 1;

        // Update order
        order.id = order_id.clone();
        order.status = OrderStatus::Accepted;
        order.submitted_at = Some(Utc::now());

        // Store order
        state.orders.insert(order_id.as_str().to_string(), order);

        Ok(order_id)
    }

    async fn cancel_order(&self, order_id: &OrderId) -> Result<(), BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("cancel_order({})", order_id));

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        let mut state = self.state.write();
        if let Some(order) = state.orders.get_mut(order_id.as_str()) {
            order.status = OrderStatus::Cancelled;
            Ok(())
        } else {
            Err(BrokerError::ApiError(format!(
                "Order not found: {}",
                order_id
            )))
        }
    }

    async fn get_order(&self, order_id: &OrderId) -> Result<Order, BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("get_order({})", order_id));

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        let state = self.state.read();
        state
            .orders
            .get(order_id.as_str())
            .cloned()
            .ok_or_else(|| BrokerError::ApiError(format!("Order not found: {}", order_id)))
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        self.simulate_latency().await;
        self.record_call("get_open_orders");

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        let state = self.state.read();
        let orders: Vec<Order> = state
            .orders
            .values()
            .filter(|o| {
                matches!(
                    o.status,
                    OrderStatus::Accepted | OrderStatus::PartiallyFilled
                )
            })
            .cloned()
            .collect();

        Ok(orders)
    }

    async fn get_account(&self) -> Result<Account, BrokerError> {
        self.simulate_latency().await;
        self.record_call("get_account");

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        Ok(self.state.read().account.clone())
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.simulate_latency().await;
        self.record_call("get_positions");

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        Ok(self.state.read().positions.values().cloned().collect())
    }

    async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, BrokerError> {
        self.simulate_latency().await;
        self.record_call(format!("get_position({})", symbol));

        if self.should_simulate_error() {
            return Err(BrokerError::ApiError("Mock error".to_string()));
        }

        Ok(self.state.read().positions.get(symbol.as_str()).cloned())
    }
}

/// Helper function to create a mock quote
pub fn create_mock_quote(symbol: Symbol, price: Decimal) -> Quote {
    let price_obj = Price::new(price).unwrap();
    Quote {
        symbol,
        bid: Price::new(price - Decimal::new(1, 2)).unwrap(),
        ask: Price::new(price + Decimal::new(1, 2)).unwrap(),
        last: price_obj,
        bid_size: 100,
        ask_size: 100,
        timestamp: Utc::now(),
    }
}

/// Helper function to create a mock candle
pub fn create_mock_candle(symbol: Symbol, timestamp: DateTime<Utc>) -> Candle {
    Candle {
        symbol,
        timestamp,
        open: Price::new(Decimal::new(10000, 2)).unwrap(),
        high: Price::new(Decimal::new(10100, 2)).unwrap(),
        low: Price::new(Decimal::new(9900, 2)).unwrap(),
        close: Price::new(Decimal::new(10050, 2)).unwrap(),
        volume: 1000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_broker_quotes() {
        let broker = MockBroker::default();
        let symbol = Symbol::new("AAPL").unwrap();

        // Set mock quote
        let quote = create_mock_quote(symbol.clone(), Decimal::new(15000, 2));
        broker.set_mock_quote(quote.clone());

        // Retrieve quote
        let retrieved = broker.get_quote(&symbol).await.unwrap();
        assert_eq!(retrieved.symbol, symbol);
        assert_eq!(retrieved.last, quote.last);
    }

    #[tokio::test]
    async fn test_mock_broker_orders() {
        let broker = MockBroker::default();
        let symbol = Symbol::new("AAPL").unwrap();
        let qty = Quantity::new(100).unwrap();

        // Place order
        let order = Order::market(symbol, OrderSide::Buy, qty);
        let order_id = broker.place_order(order).await.unwrap();

        // Retrieve order
        let retrieved = broker.get_order(&order_id).await.unwrap();
        assert_eq!(retrieved.status, OrderStatus::Accepted);

        // Cancel order
        broker.cancel_order(&order_id).await.unwrap();
        let cancelled = broker.get_order(&order_id).await.unwrap();
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_mock_broker_call_history() {
        let broker = MockBroker::default();
        let symbol = Symbol::new("AAPL").unwrap();

        broker.get_quote(&symbol).await.ok();
        broker.get_account().await.ok();

        let history = broker.get_call_history();
        assert!(history.contains(&format!("get_quote({})", symbol)));
        assert!(history.contains(&"get_account".to_string()));
    }

    #[tokio::test]
    async fn test_mock_broker_error_simulation() {
        let config = MockConfig {
            latency_ms: 0,
            simulate_errors: true,
            error_rate: 1.0, // Always error
        };
        let broker = MockBroker::new(config);
        let symbol = Symbol::new("AAPL").unwrap();

        let result = broker.get_quote(&symbol).await;
        assert!(result.is_err());
    }
}
