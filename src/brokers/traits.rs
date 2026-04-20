//! Broker abstraction traits
//!
//! This module defines the core traits for broker integrations, enabling
//! zero-cost abstraction through static dispatch with generics.

use super::types::{OrderId, Price, Quantity, Symbol};
use crate::utils::errors::BrokerError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Structures
// ============================================================================

/// Real-time quote data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// Stock symbol
    pub symbol: Symbol,
    /// Bid price
    pub bid: Price,
    /// Ask price
    pub ask: Price,
    /// Last trade price
    pub last: Price,
    /// Bid size
    pub bid_size: u32,
    /// Ask size
    pub ask_size: u32,
    /// Quote timestamp
    pub timestamp: DateTime<Utc>,
}

/// OHLCV candlestick data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// Stock symbol
    pub symbol: Symbol,
    /// Candle timestamp (start of period)
    pub timestamp: DateTime<Utc>,
    /// Opening price
    pub open: Price,
    /// Highest price in period
    pub high: Price,
    /// Lowest price in period
    pub low: Price,
    /// Closing price
    pub close: Price,
    /// Volume traded
    pub volume: u64,
}

impl Candle {
    /// Validates that OHLC relationships are correct
    ///
    /// # Errors
    ///
    /// Returns an error if high < low, or if open/close are outside high/low range
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.high.value() < self.low.value() {
            return Err("High price cannot be less than low price");
        }
        if self.open.value() < self.low.value() || self.open.value() > self.high.value() {
            return Err("Open price must be between low and high");
        }
        if self.close.value() < self.low.value() || self.close.value() > self.high.value() {
            return Err("Close price must be between low and high");
        }
        Ok(())
    }

    /// Returns true if this is a bullish candle (close > open)
    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close.value() > self.open.value()
    }

    /// Returns true if this is a bearish candle (close < open)
    #[inline]
    pub fn is_bearish(&self) -> bool {
        self.close.value() < self.open.value()
    }

    /// Returns the body size (absolute difference between open and close)
    #[inline]
    pub fn body_size(&self) -> Decimal {
        (self.close.value() - self.open.value()).abs()
    }

    /// Returns the total range (high - low)
    #[inline]
    pub fn range(&self) -> Decimal {
        self.high.value() - self.low.value()
    }
}

/// Position in a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Stock symbol
    pub symbol: Symbol,
    /// Quantity held (positive for long, negative for short)
    pub quantity: i32,
    /// Average entry price
    pub avg_entry_price: Price,
    /// Current market price
    pub current_price: Price,
    /// Current market value of position
    pub market_value: Decimal,
    /// Unrealized profit/loss
    pub unrealized_pl: Decimal,
    /// Unrealized P/L percentage
    pub unrealized_pl_percent: Decimal,
}

/// Order to be placed or retrieved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Order ID (empty for new orders)
    pub id: OrderId,
    /// Stock symbol
    pub symbol: Symbol,
    /// Buy or sell
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Quantity to trade
    pub quantity: Quantity,
    /// Limit price (for limit orders)
    pub limit_price: Option<Price>,
    /// Stop price (for stop orders)
    pub stop_price: Option<Price>,
    /// Trailing amount (for trailing stop orders)
    pub trailing_amount: Option<Decimal>,
    /// Order status
    pub status: OrderStatus,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Quantity filled so far
    pub filled_quantity: u32,
    /// Average fill price
    pub filled_avg_price: Option<Price>,
    /// Submission timestamp
    pub submitted_at: Option<DateTime<Utc>>,
    /// Fill timestamp
    pub filled_at: Option<DateTime<Utc>>,
}

impl Order {
    /// Creates a new market order
    pub fn market(symbol: Symbol, side: OrderSide, quantity: Quantity) -> Self {
        Self {
            id: OrderId::new(""),
            symbol,
            side,
            order_type: OrderType::Market,
            quantity,
            limit_price: None,
            stop_price: None,
            trailing_amount: None,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::Day,
            filled_quantity: 0,
            filled_avg_price: None,
            submitted_at: None,
            filled_at: None,
        }
    }

    /// Creates a new limit order
    pub fn limit(symbol: Symbol, side: OrderSide, quantity: Quantity, limit_price: Price) -> Self {
        Self {
            id: OrderId::new(""),
            symbol,
            side,
            order_type: OrderType::Limit,
            quantity,
            limit_price: Some(limit_price),
            stop_price: None,
            trailing_amount: None,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::Day,
            filled_quantity: 0,
            filled_avg_price: None,
            submitted_at: None,
            filled_at: None,
        }
    }

    /// Calculates the estimated total value of this order
    #[inline]
    pub fn estimated_value(&self) -> Option<Decimal> {
        self.limit_price
            .or(self.stop_price)
            .map(|p| p.value() * Decimal::from(self.quantity.value()))
    }
}

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// Market order (execute at current market price)
    Market,
    /// Limit order (execute at specified price or better)
    Limit,
    /// Stop order (becomes market order when stop price is reached)
    Stop,
    /// Stop-limit order (becomes limit order when stop price is reached)
    StopLimit,
    /// Trailing stop order (stop price trails the market)
    TrailingStop,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is pending submission
    Pending,
    /// Order has been submitted but not yet filled
    Accepted,
    /// Order has been partially filled
    PartiallyFilled,
    /// Order has been completely filled
    Filled,
    /// Order has been cancelled
    Cancelled,
    /// Order was rejected
    Rejected,
    /// Order has expired
    Expired,
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    /// Good for day (expires at market close)
    Day,
    /// Good till cancelled
    #[serde(rename = "gtc")]
    GTC,
    /// Immediate or cancel
    #[serde(rename = "ioc")]
    IOC,
    /// Fill or kill
    #[serde(rename = "fok")]
    FOK,
}

/// Account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Account number
    pub account_number: String,
    /// Buying power available
    pub buying_power: Decimal,
    /// Cash available
    pub cash: Decimal,
    /// Total portfolio value
    pub portfolio_value: Decimal,
    /// Number of day trades in rolling 5-day window
    pub day_trade_count: u32,
    /// Whether account is flagged as pattern day trader
    pub is_pattern_day_trader: bool,
    /// Account equity
    pub equity: Decimal,
    /// Last equity value
    pub last_equity: Decimal,
}

/// Timeframe for historical candles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Timeframe {
    /// 1-minute candles
    OneMinute,
    /// 5-minute candles
    FiveMinutes,
    /// 15-minute candles
    FifteenMinutes,
    /// 1-hour candles
    OneHour,
    /// Daily candles
    OneDay,
}

// ============================================================================
// Trait Definitions
// ============================================================================

/// Market data provider trait
///
/// Provides real-time and historical market data.
/// Use static dispatch with generics for zero-cost abstraction.
///
/// # Examples
///
/// ```no_run
/// use daytrader::brokers::traits::MarketDataProvider;
///
/// async fn get_data<P: MarketDataProvider>(provider: &P, symbol: &str) {
///     let quote = provider.get_quote(symbol).await.unwrap();
///     println!("Last price: {}", quote.last);
/// }
/// ```
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// Subscribe to real-time quotes for given symbols
    ///
    /// # Errors
    ///
    /// Returns an error if subscription fails or symbols are invalid
    async fn subscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError>;

    /// Unsubscribe from quotes for given symbols
    ///
    /// # Errors
    ///
    /// Returns an error if unsubscription fails
    async fn unsubscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError>;

    /// Get latest quote for a symbol
    ///
    /// # Errors
    ///
    /// Returns an error if symbol is not found or API call fails
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;

    /// Get historical candles for a symbol
    ///
    /// # Errors
    ///
    /// Returns an error if data retrieval fails or parameters are invalid
    async fn get_candles(
        &self,
        symbol: &Symbol,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError>;

    /// Start real-time data stream
    ///
    /// # Errors
    ///
    /// Returns an error if stream cannot be started
    async fn start_stream(&self) -> Result<(), BrokerError>;

    /// Stop real-time data stream
    ///
    /// # Errors
    ///
    /// Returns an error if stream cannot be stopped
    async fn stop_stream(&self) -> Result<(), BrokerError>;
}

/// Order execution provider trait
///
/// Provides order placement and management capabilities.
/// Use static dispatch with generics for zero-cost abstraction.
///
/// # Examples
///
/// ```no_run
/// use daytrader::brokers::traits::{OrderExecutionProvider, Order};
///
/// async fn place_order<P: OrderExecutionProvider>(provider: &P, order: Order) {
///     let order_id = provider.place_order(order).await.unwrap();
///     println!("Order placed: {}", order_id);
/// }
/// ```
#[async_trait]
pub trait OrderExecutionProvider: Send + Sync {
    /// Place a new order
    ///
    /// # Errors
    ///
    /// Returns an error if order is rejected or placement fails
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError>;

    /// Cancel an existing order
    ///
    /// # Errors
    ///
    /// Returns an error if order cannot be cancelled
    async fn cancel_order(&self, order_id: &OrderId) -> Result<(), BrokerError>;

    /// Get order status and details
    ///
    /// # Errors
    ///
    /// Returns an error if order is not found
    async fn get_order(&self, order_id: &OrderId) -> Result<Order, BrokerError>;

    /// Get all open orders
    ///
    /// # Errors
    ///
    /// Returns an error if retrieval fails
    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError>;

    /// Get account information
    ///
    /// # Errors
    ///
    /// Returns an error if account info cannot be retrieved
    async fn get_account(&self) -> Result<Account, BrokerError>;

    /// Get all current positions
    ///
    /// # Errors
    ///
    /// Returns an error if positions cannot be retrieved
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;

    /// Get position for a specific symbol
    ///
    /// # Errors
    ///
    /// Returns an error if position lookup fails
    async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, BrokerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_validation() {
        let symbol = Symbol::new("AAPL").unwrap();

        // Valid candle
        let candle = Candle {
            symbol: symbol.clone(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(15000, 2)).unwrap(),
            high: Price::new(Decimal::new(15100, 2)).unwrap(),
            low: Price::new(Decimal::new(14900, 2)).unwrap(),
            close: Price::new(Decimal::new(15050, 2)).unwrap(),
            volume: 1000,
        };
        assert!(candle.validate().is_ok());
        assert!(candle.is_bullish());

        // Invalid candle (high < low)
        let invalid = Candle {
            symbol: symbol.clone(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(15000, 2)).unwrap(),
            high: Price::new(Decimal::new(14900, 2)).unwrap(),
            low: Price::new(Decimal::new(15000, 2)).unwrap(),
            close: Price::new(Decimal::new(14950, 2)).unwrap(),
            volume: 1000,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_order_creation() {
        let symbol = Symbol::new("AAPL").unwrap();
        let qty = Quantity::new(100).unwrap();

        let market_order = Order::market(symbol.clone(), OrderSide::Buy, qty);
        assert_eq!(market_order.order_type, OrderType::Market);
        assert!(market_order.limit_price.is_none());

        let price = Price::new(Decimal::new(15000, 2)).unwrap();
        let limit_order = Order::limit(symbol, OrderSide::Sell, qty, price);
        assert_eq!(limit_order.order_type, OrderType::Limit);
        assert_eq!(limit_order.limit_price, Some(price));
    }

    #[test]
    fn test_candle_helpers() {
        let symbol = Symbol::new("AAPL").unwrap();
        let candle = Candle {
            symbol,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(10000, 2)).unwrap(),
            high: Price::new(Decimal::new(10200, 2)).unwrap(),
            low: Price::new(Decimal::new(9900, 2)).unwrap(),
            close: Price::new(Decimal::new(10100, 2)).unwrap(),
            volume: 1000,
        };

        assert!(candle.is_bullish());
        assert!(!candle.is_bearish());
        assert_eq!(candle.body_size(), Decimal::new(100, 2));
        assert_eq!(candle.range(), Decimal::new(300, 2));
    }
}
