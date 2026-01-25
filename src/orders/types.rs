//! Order types and validation
//!
//! This module provides type-safe order representations with comprehensive
//! validation logic. It uses the newtype pattern for domain types and
//! builder pattern for complex order construction.

use crate::brokers::traits::{OrderSide, OrderStatus, OrderType, TimeInForce};
use crate::brokers::types::{OrderId, PatternName, Price, Quantity, Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Order Request Types
// ============================================================================

/// Request to place a new order
///
/// This is the primary input type for order placement, containing all
/// necessary information including optional pattern metadata for trades
/// generated from pattern signals.
///
/// # Examples
///
/// ```
/// use daytrader::orders::types::OrderRequest;
/// use daytrader::brokers::types::{Symbol, Quantity};
/// use daytrader::brokers::traits::{OrderSide, OrderType, TimeInForce};
///
/// let request = OrderRequest::builder()
///     .symbol(Symbol::new("AAPL").unwrap())
///     .side(OrderSide::Buy)
///     .order_type(OrderType::Market)
///     .quantity(Quantity::new(100).unwrap())
///     .time_in_force(TimeInForce::Day)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    /// Stock symbol
    pub symbol: Symbol,
    /// Buy or sell
    pub side: OrderSide,
    /// Order type (market, limit, stop, etc.)
    pub order_type: OrderType,
    /// Quantity to trade
    pub quantity: Quantity,
    /// Limit price (required for limit and stop-limit orders)
    pub limit_price: Option<Price>,
    /// Stop price (required for stop and stop-limit orders)
    pub stop_price: Option<Price>,
    /// Trailing percent for trailing stop orders (0.0 to 1.0)
    pub trailing_percent: Option<Decimal>,
    /// Time in force
    pub time_in_force: TimeInForce,
    
    // Pattern metadata (optional)
    /// Pattern name if this is a pattern-based trade
    pub pattern_name: Option<PatternName>,
    /// Signal ID that generated this order
    pub signal_id: Option<String>,
    /// Confidence score of the signal (0.0 to 1.0)
    pub signal_confidence: Option<Decimal>,
}

impl OrderRequest {
    /// Creates a builder for constructing an OrderRequest
    #[inline]
    pub fn builder() -> OrderRequestBuilder {
        OrderRequestBuilder::default()
    }

    /// Validates the order request parameters
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Quantity is zero
    /// - Limit price is missing for limit orders
    /// - Stop price is missing for stop orders
    /// - Trailing percent is invalid or missing for trailing stop orders
    /// - Stop-limit order is missing either limit or stop price
    pub fn validate(&self) -> Result<(), OrderError> {
        // Validate quantity
        if self.quantity.value() == 0 {
            return Err(OrderError::ValidationFailed(
                "Quantity must be positive".into(),
            ));
        }

        // Validate limit price for limit orders
        if self.order_type == OrderType::Limit && self.limit_price.is_none() {
            return Err(OrderError::ValidationFailed(
                "Limit price required for limit orders".into(),
            ));
        }

        // Validate stop price for stop orders
        if matches!(self.order_type, OrderType::Stop | OrderType::StopLimit)
            && self.stop_price.is_none()
        {
            return Err(OrderError::ValidationFailed(
                "Stop price required for stop orders".into(),
            ));
        }

        // Validate stop-limit has both prices
        if self.order_type == OrderType::StopLimit {
            if self.limit_price.is_none() || self.stop_price.is_none() {
                return Err(OrderError::ValidationFailed(
                    "Stop-limit orders require both limit and stop prices".into(),
                ));
            }
        }

        // Validate trailing percent
        if self.order_type == OrderType::TrailingStop {
            if let Some(pct) = self.trailing_percent {
                if pct <= Decimal::ZERO || pct >= Decimal::ONE {
                    return Err(OrderError::ValidationFailed(
                        "Trailing percent must be between 0 and 1".into(),
                    ));
                }
            } else {
                return Err(OrderError::ValidationFailed(
                    "Trailing percent required for trailing stop orders".into(),
                ));
            }
        }

        Ok(())
    }

    /// Estimates the total value of this order
    ///
    /// Returns None if no price information is available (e.g., market order).
    #[inline]
    pub fn estimated_value(&self) -> Option<Decimal> {
        self.limit_price
            .or(self.stop_price)
            .map(|p| p.value() * Decimal::from(self.quantity.value()))
    }
}

// ============================================================================
// Managed Order Type
// ============================================================================

/// Internal order representation with full lifecycle tracking
///
/// This extends the basic broker Order with additional metadata for
/// internal tracking, pattern association, and risk validation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedOrder {
    // Core order fields
    /// Internal order ID
    pub id: OrderId,
    /// Stock symbol
    pub symbol: Symbol,
    /// Buy or sell
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Quantity requested
    pub quantity: Quantity,
    /// Limit price (for limit orders)
    pub limit_price: Option<Price>,
    /// Stop price (for stop orders)
    pub stop_price: Option<Price>,
    /// Trailing amount (percent or price)
    pub trailing_amount: Option<Decimal>,
    /// Current order status
    pub status: OrderStatus,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Quantity filled so far
    pub filled_quantity: u32,
    /// Average fill price
    pub filled_avg_price: Option<Price>,
    /// Timestamp when submitted to broker
    pub submitted_at: Option<DateTime<Utc>>,
    /// Timestamp when filled (or partially filled)
    pub filled_at: Option<DateTime<Utc>>,
    
    // Enhanced tracking fields
    /// Broker's order ID (assigned after submission)
    pub broker_order_id: Option<String>,
    /// Commission paid on this order
    pub commission: Option<Decimal>,
    /// Error message if order was rejected
    pub error_message: Option<String>,
    
    // Pattern tracking
    /// Whether this is a pattern-based trade
    pub is_pattern_trade: bool,
    /// Name of the pattern that generated this order
    pub pattern_name: Option<PatternName>,
    /// Signal ID that generated this order
    pub signal_id: Option<String>,
    /// Confidence score of the signal
    pub signal_confidence: Option<Decimal>,
    
    // Risk tracking
    /// Whether pre-trade risk validation passed
    pub pre_trade_validation_passed: bool,
    /// Timestamp of risk validation
    pub risk_validation_timestamp: Option<DateTime<Utc>>,
    
    // Timestamps
    /// When order was created internally
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl ManagedOrder {
    /// Creates a new managed order from a request
    pub fn from_request(id: OrderId, request: OrderRequest) -> Self {
        let now = Utc::now();
        Self {
            id,
            symbol: request.symbol,
            side: request.side,
            order_type: request.order_type,
            quantity: request.quantity,
            limit_price: request.limit_price,
            stop_price: request.stop_price,
            trailing_amount: request.trailing_percent,
            status: OrderStatus::Pending,
            time_in_force: request.time_in_force,
            filled_quantity: 0,
            filled_avg_price: None,
            submitted_at: None,
            filled_at: None,
            broker_order_id: None,
            commission: None,
            error_message: None,
            is_pattern_trade: request.pattern_name.is_some(),
            pattern_name: request.pattern_name,
            signal_id: request.signal_id,
            signal_confidence: request.signal_confidence,
            pre_trade_validation_passed: false,
            risk_validation_timestamp: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Converts to broker Order type for submission
    pub fn to_broker_order(&self) -> crate::brokers::traits::Order {
        crate::brokers::traits::Order {
            id: self.id.clone(),
            symbol: self.symbol.clone(),
            side: self.side,
            order_type: self.order_type,
            quantity: self.quantity,
            limit_price: self.limit_price,
            stop_price: self.stop_price,
            trailing_amount: self.trailing_amount,
            status: self.status,
            time_in_force: self.time_in_force,
            filled_quantity: self.filled_quantity,
            filled_avg_price: self.filled_avg_price,
            submitted_at: self.submitted_at,
            filled_at: self.filled_at,
        }
    }

    /// Returns true if this order can be cancelled
    #[inline]
    pub fn is_cancellable(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Accepted | OrderStatus::PartiallyFilled
        )
    }

    /// Returns true if this order is in a terminal state
    #[inline]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }

    /// Returns the remaining quantity to fill
    #[inline]
    pub fn remaining_quantity(&self) -> u32 {
        self.quantity.value().saturating_sub(self.filled_quantity)
    }

    /// Updates the order with fill information
    pub fn update_with_fill(&mut self, fill: &FillInfo) {
        self.filled_quantity += fill.filled_quantity.value();
        
        // Update average fill price
        if let Some(existing_avg) = self.filled_avg_price {
            // Weighted average calculation
            let total_value = existing_avg.value() * Decimal::from(self.filled_quantity - fill.filled_quantity.value())
                + fill.filled_price.value() * Decimal::from(fill.filled_quantity.value());
            let new_avg = total_value / Decimal::from(self.filled_quantity);
            self.filled_avg_price = Price::new(new_avg).ok();
        } else {
            self.filled_avg_price = Some(fill.filled_price);
        }

        // Update commission
        self.commission = Some(self.commission.unwrap_or(Decimal::ZERO) + fill.commission);

        // Update status
        if self.filled_quantity >= self.quantity.value() {
            self.status = OrderStatus::Filled;
            self.filled_at = Some(fill.timestamp);
        } else {
            self.status = OrderStatus::PartiallyFilled;
            if self.filled_at.is_none() {
                self.filled_at = Some(fill.timestamp);
            }
        }

        self.updated_at = Utc::now();
    }
}

impl OrderStatus {
    /// Returns true if this status allows cancellation
    #[inline]
    pub fn is_cancellable(&self) -> bool {
        matches!(
            self,
            OrderStatus::Pending | OrderStatus::Accepted | OrderStatus::PartiallyFilled
        )
    }

    /// Returns true if this is a terminal status
    #[inline]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }
}

// ============================================================================
// Fill Information
// ============================================================================

/// Information about an order fill (partial or complete)
///
/// Contains details about the execution of an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillInfo {
    /// Quantity filled in this execution
    pub filled_quantity: Quantity,
    /// Price at which shares were filled
    pub filled_price: Price,
    /// Commission charged for this fill
    pub commission: Decimal,
    /// Timestamp of the fill
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// Order Configuration
// ============================================================================

/// Configuration for order management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfig {
    /// Maximum concurrent orders
    pub max_concurrent_orders: usize,
    /// Allow trading during extended hours
    pub allow_extended_hours: bool,
    /// Default time in force
    pub default_time_in_force: TimeInForce,
    /// Enable automatic order retries
    pub enable_retries: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for OrderConfig {
    fn default() -> Self {
        Self {
            max_concurrent_orders: 10,
            allow_extended_hours: false,
            default_time_in_force: TimeInForce::Day,
            enable_retries: true,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

// ============================================================================
// Order Builder Pattern
// ============================================================================

/// Builder for constructing OrderRequest
///
/// Provides a fluent API for building complex orders with validation.
///
/// # Examples
///
/// ```
/// use daytrader::orders::types::OrderRequest;
/// use daytrader::brokers::types::{Symbol, Price, Quantity};
/// use daytrader::brokers::traits::{OrderSide, OrderType, TimeInForce};
/// use rust_decimal::Decimal;
///
/// let request = OrderRequest::builder()
///     .symbol(Symbol::new("AAPL").unwrap())
///     .side(OrderSide::Buy)
///     .order_type(OrderType::Limit)
///     .quantity(Quantity::new(100).unwrap())
///     .limit_price(Price::new(Decimal::new(15000, 2)).unwrap())
///     .time_in_force(TimeInForce::GTC)
///     .pattern_name("Hammer".into())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct OrderRequestBuilder {
    symbol: Option<Symbol>,
    side: Option<OrderSide>,
    order_type: Option<OrderType>,
    quantity: Option<Quantity>,
    limit_price: Option<Price>,
    stop_price: Option<Price>,
    trailing_percent: Option<Decimal>,
    time_in_force: Option<TimeInForce>,
    pattern_name: Option<PatternName>,
    signal_id: Option<String>,
    signal_confidence: Option<Decimal>,
}

impl OrderRequestBuilder {
    /// Sets the symbol
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the order side
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the order type
    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    /// Sets the quantity
    pub fn quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the limit price
    pub fn limit_price(mut self, price: Price) -> Self {
        self.limit_price = Some(price);
        self
    }

    /// Sets the stop price
    pub fn stop_price(mut self, price: Price) -> Self {
        self.stop_price = Some(price);
        self
    }

    /// Sets the trailing percent
    pub fn trailing_percent(mut self, percent: Decimal) -> Self {
        self.trailing_percent = Some(percent);
        self
    }

    /// Sets the time in force
    pub fn time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = Some(tif);
        self
    }

    /// Sets the pattern name
    pub fn pattern_name(mut self, name: impl Into<String>) -> Self {
        self.pattern_name = Some(PatternName::new(name));
        self
    }

    /// Sets the signal ID
    pub fn signal_id(mut self, id: impl Into<String>) -> Self {
        self.signal_id = Some(id.into());
        self
    }

    /// Sets the signal confidence
    pub fn signal_confidence(mut self, confidence: Decimal) -> Self {
        self.signal_confidence = Some(confidence);
        self
    }

    /// Builds the OrderRequest, validating all required fields
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or validation fails
    pub fn build(self) -> Result<OrderRequest, OrderError> {
        let request = OrderRequest {
            symbol: self.symbol.ok_or_else(|| {
                OrderError::ValidationFailed("Symbol is required".into())
            })?,
            side: self.side.ok_or_else(|| {
                OrderError::ValidationFailed("Order side is required".into())
            })?,
            order_type: self.order_type.ok_or_else(|| {
                OrderError::ValidationFailed("Order type is required".into())
            })?,
            quantity: self.quantity.ok_or_else(|| {
                OrderError::ValidationFailed("Quantity is required".into())
            })?,
            limit_price: self.limit_price,
            stop_price: self.stop_price,
            trailing_percent: self.trailing_percent,
            time_in_force: self.time_in_force.unwrap_or(TimeInForce::Day),
            pattern_name: self.pattern_name,
            signal_id: self.signal_id,
            signal_confidence: self.signal_confidence,
        };

        // Validate the constructed request
        request.validate()?;

        Ok(request)
    }
}

// ============================================================================
// Order Events
// ============================================================================

/// Events emitted during order lifecycle
#[derive(Debug, Clone)]
pub enum OrderEvent {
    /// Order request validated successfully
    OrderValidated {
        order_id: OrderId,
        request: Arc<OrderRequest>,
    },
    /// Order validation failed
    OrderValidationFailed {
        request: Arc<OrderRequest>,
        reason: String,
    },
    /// Order placed and tracked internally
    OrderPlaced {
        order: Arc<ManagedOrder>,
    },
    /// Order submitted to broker successfully
    OrderSubmitted {
        order_id: OrderId,
        broker_order_id: String,
        submitted_at: DateTime<Utc>,
    },
    /// Order partially filled
    OrderPartiallyFilled {
        order_id: OrderId,
        fill_info: Arc<FillInfo>,
        remaining_quantity: u32,
    },
    /// Order completely filled
    OrderFilled {
        order_id: OrderId,
        fill_info: Arc<FillInfo>,
    },
    /// Order cancelled
    OrderCancelled {
        order_id: OrderId,
        reason: String,
        cancelled_at: DateTime<Utc>,
    },
    /// Order rejected by broker
    OrderRejected {
        order_id: OrderId,
        reason: String,
        rejected_at: DateTime<Utc>,
    },
    /// Order expired
    OrderExpired {
        order_id: OrderId,
        expired_at: DateTime<Utc>,
    },
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during order management
#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    /// Order validation failed
    #[error("Order validation failed: {0}")]
    ValidationFailed(String),

    /// Insufficient buying power
    #[error("Insufficient buying power: required ${required}, available ${available}")]
    InsufficientBuyingPower {
        required: Decimal,
        available: Decimal,
    },

    /// Risk violation
    #[error("Risk violation: {0}")]
    RiskViolation(String),

    /// Market closed
    #[error("Market closed")]
    MarketClosed,

    /// Pattern disabled
    #[error("Pattern disabled: {0}")]
    PatternDisabled(String),

    /// Pattern daily limit reached
    #[error("Pattern daily limit reached: {pattern} (limit: {limit})")]
    PatternDailyLimitReached { pattern: String, limit: u32 },

    /// Order not found
    #[error("Order not found: {0}")]
    NotFound(OrderId),

    /// Order not cancellable
    #[error("Order not cancellable: {0} (status: {1:?})")]
    NotCancellable(OrderId, OrderStatus),

    /// Broker rejected order
    #[error("Broker rejected order: {0}")]
    BrokerRejection(String),

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    /// Broker error
    #[error("Broker error: {0}")]
    BrokerError(#[from] crate::utils::errors::BrokerError),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

// ============================================================================
// Order Metrics
// ============================================================================

/// Metrics for order management performance
#[derive(Debug, Default)]
pub struct OrderMetrics {
    /// Total orders placed
    pub orders_placed: u64,
    /// Orders successfully submitted
    pub orders_submitted: u64,
    /// Orders filled
    pub orders_filled: u64,
    /// Orders cancelled
    pub orders_cancelled: u64,
    /// Orders rejected
    pub orders_rejected: u64,
    /// Average placement latency (ms)
    pub avg_placement_latency_ms: f64,
    /// Average validation latency (ms)
    pub avg_validation_latency_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_order_request_validation_market_order() {
        let request = OrderRequest {
            symbol: Symbol::new("AAPL").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: Quantity::new(100).unwrap(),
            limit_price: None,
            stop_price: None,
            trailing_percent: None,
            time_in_force: TimeInForce::Day,
            pattern_name: None,
            signal_id: None,
            signal_confidence: None,
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_order_request_validation_limit_order() {
        let mut request = OrderRequest {
            symbol: Symbol::new("AAPL").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Quantity::new(100).unwrap(),
            limit_price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
            stop_price: None,
            trailing_percent: None,
            time_in_force: TimeInForce::Day,
            pattern_name: None,
            signal_id: None,
            signal_confidence: None,
        };

        assert!(request.validate().is_ok());

        // Should fail without limit price
        request.limit_price = None;
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_order_request_validation_trailing_stop() {
        let mut request = OrderRequest {
            symbol: Symbol::new("AAPL").unwrap(),
            side: OrderSide::Sell,
            order_type: OrderType::TrailingStop,
            quantity: Quantity::new(100).unwrap(),
            limit_price: None,
            stop_price: None,
            trailing_percent: Some(Decimal::new(5, 2)), // 0.05 = 5%
            time_in_force: TimeInForce::Day,
            pattern_name: None,
            signal_id: None,
            signal_confidence: None,
        };

        assert!(request.validate().is_ok());

        // Should fail with invalid trailing percent
        request.trailing_percent = Some(Decimal::new(15, 1)); // 1.5 = 150%
        assert!(request.validate().is_err());

        request.trailing_percent = Some(Decimal::ZERO);
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_order_request_builder() {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(100).unwrap())
            .limit_price(Price::new(Decimal::new(15000, 2)).unwrap())
            .time_in_force(TimeInForce::GTC)
            .pattern_name("Hammer")
            .signal_confidence(Decimal::new(85, 2))
            .build()
            .unwrap();

        assert_eq!(request.symbol.as_str(), "AAPL");
        assert_eq!(request.side, OrderSide::Buy);
        assert_eq!(request.order_type, OrderType::Limit);
        assert!(request.pattern_name.is_some());
    }

    #[test]
    fn test_managed_order_from_request() {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let order_id = OrderId::new("order-123");
        let managed = ManagedOrder::from_request(order_id.clone(), request);

        assert_eq!(managed.id, order_id);
        assert_eq!(managed.status, OrderStatus::Pending);
        assert!(!managed.pre_trade_validation_passed);
        assert!(managed.is_cancellable());
        assert!(!managed.is_terminal());
    }

    #[test]
    fn test_managed_order_fill_update() {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let mut order = ManagedOrder::from_request(OrderId::new("order-123"), request);

        // Partial fill
        let fill1 = FillInfo {
            filled_quantity: Quantity::new(50).unwrap(),
            filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
            commission: Decimal::new(1, 0),
            timestamp: Utc::now(),
        };

        order.update_with_fill(&fill1);
        assert_eq!(order.filled_quantity, 50);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.remaining_quantity(), 50);

        // Complete fill
        let fill2 = FillInfo {
            filled_quantity: Quantity::new(50).unwrap(),
            filled_price: Price::new(Decimal::new(15010, 2)).unwrap(),
            commission: Decimal::new(1, 0),
            timestamp: Utc::now(),
        };

        order.update_with_fill(&fill2);
        assert_eq!(order.filled_quantity, 100);
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.remaining_quantity(), 0);
        assert!(order.is_terminal());
        assert!(!order.is_cancellable());

        // Verify average price calculation
        assert!(order.filled_avg_price.is_some());
        let avg = order.filled_avg_price.unwrap();
        // Average should be (150 * 50 + 150.10 * 50) / 100 = 150.05
        assert_eq!(avg.value(), Decimal::new(15005, 2));

        // Verify commission accumulation
        assert_eq!(order.commission.unwrap(), Decimal::new(2, 0));
    }

    #[test]
    fn test_order_status_helpers() {
        assert!(OrderStatus::Pending.is_cancellable());
        assert!(OrderStatus::Accepted.is_cancellable());
        assert!(OrderStatus::PartiallyFilled.is_cancellable());
        assert!(!OrderStatus::Filled.is_cancellable());
        assert!(!OrderStatus::Cancelled.is_cancellable());
        assert!(!OrderStatus::Rejected.is_cancellable());

        assert!(!OrderStatus::Pending.is_terminal());
        assert!(!OrderStatus::Accepted.is_terminal());
        assert!(!OrderStatus::PartiallyFilled.is_terminal());
        assert!(OrderStatus::Filled.is_terminal());
        assert!(OrderStatus::Cancelled.is_terminal());
        assert!(OrderStatus::Rejected.is_terminal());
        assert!(OrderStatus::Expired.is_terminal());
    }

    #[test]
    fn test_estimated_value() {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(100).unwrap())
            .limit_price(Price::new(Decimal::new(15000, 2)).unwrap())
            .build()
            .unwrap();

        let value = request.estimated_value().unwrap();
        assert_eq!(value, Decimal::new(1500000, 2)); // 100 * $150.00 = $15000.00
    }

    #[test]
    fn test_builder_missing_required_fields() {
        let result = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            // Missing order_type and quantity
            .build();

        assert!(result.is_err());
    }
}
