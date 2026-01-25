//! Order management system
//!
//! This module provides comprehensive order lifecycle management including
//! order placement, validation, tracking, and broker integration.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────┐
//! │ OrderRequest   │ ← Input from signals or manual trades
//! └────────┬───────┘
//!          │
//!          ▼
//! ┌────────────────┐
//! │  OrderEngine   │ ← Validates and places orders
//! │  - Validation  │
//! │  - Risk check  │
//! │  - Submission  │
//! └────────┬───────┘
//!          │
//!          ├─→ RiskEngine (pre-trade validation)
//!          ├─→ BrokerAdapter (submission)
//!          ├─→ OrderTracker (status monitoring)
//!          └─→ EventBus (lifecycle events)
//! ```
//!
//! # Key Components
//!
//! - [`types`]: Order types, requests, and error definitions
//! - [`engine`]: Core order placement and lifecycle management
//! - [`tracker`]: Real-time order status tracking
//! - [`broker_adapter`]: Broker integration and order submission
//!
//! # Examples
//!
//! ## Placing a Market Order
//!
//! ```no_run
//! use daytrader::orders::{OrderEngine, OrderRequest};
//! use daytrader::brokers::types::{Symbol, Quantity};
//! use daytrader::brokers::traits::{OrderSide, OrderType};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine: OrderEngine = todo!();
//! let request = OrderRequest::builder()
//!     .symbol(Symbol::new("AAPL")?)
//!     .side(OrderSide::Buy)
//!     .order_type(OrderType::Market)
//!     .quantity(Quantity::new(100)?)
//!     .build()?;
//!
//! let order_id = engine.place_order(request).await?;
//! println!("Order placed: {}", order_id);
//! # Ok(())
//! # }
//! ```
//!
//! ## Placing a Limit Order with Pattern Metadata
//!
//! ```no_run
//! use daytrader::orders::{OrderEngine, OrderRequest};
//! use daytrader::brokers::types::{Symbol, Price, Quantity};
//! use daytrader::brokers::traits::{OrderSide, OrderType, TimeInForce};
//! use rust_decimal::Decimal;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine: OrderEngine = todo!();
//! let request = OrderRequest::builder()
//!     .symbol(Symbol::new("AAPL")?)
//!     .side(OrderSide::Buy)
//!     .order_type(OrderType::Limit)
//!     .quantity(Quantity::new(100)?)
//!     .limit_price(Price::new(Decimal::new(15000, 2))?)
//!     .time_in_force(TimeInForce::GTC)
//!     .pattern_name("Hammer")
//!     .signal_confidence(Decimal::new(85, 2))
//!     .build()?;
//!
//! let order_id = engine.place_order(request).await?;
//! # Ok(())
//! # }
//! ```

pub mod broker_adapter;
pub mod engine;
pub mod tracker;
pub mod types;

// Re-export commonly used types
pub use engine::OrderEngine;
pub use tracker::OrderTracker;
pub use types::{
    FillInfo, ManagedOrder, OrderConfig, OrderError, OrderEvent, OrderMetrics, OrderRequest,
    OrderRequestBuilder,
};
