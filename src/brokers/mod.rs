//! Broker abstraction layer
//! 
//! This module provides a trait-based abstraction for broker APIs,
//! enabling flexibility to switch between different brokers (Alpaca, IBKR, etc.)
//! without changing business logic.
//!
//! # Architecture
//!
//! The broker abstraction uses **static dispatch with generics** for zero-cost
//! abstractions. This means no virtual function calls or dynamic dispatch overhead.
//!
//! # Design Principles
//!
//! 1. **Type Safety**: Domain types use the newtype pattern to prevent errors
//! 2. **Static Dispatch**: Generic traits instead of trait objects for performance
//! 3. **Pure Functions**: Side effects only at boundaries
//! 4. **Functional Composition**: Build complex operations from simple functions
//!
//! # Example Usage
//!
//! ```no_run
//! use daytrader::brokers::{
//!     traits::{MarketDataProvider, OrderExecutionProvider},
//!     types::{Symbol, Quantity, OrderSide},
//!     mock::MockBroker,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // Use generic function with static dispatch
//!     async fn get_quote<P: MarketDataProvider>(provider: &P, symbol: Symbol) {
//!         let quote = provider.get_quote(&symbol).await.unwrap();
//!         println!("Price: {}", quote.last);
//!     }
//!     
//!     let broker = MockBroker::default();
//!     let symbol = Symbol::new("AAPL").unwrap();
//!     get_quote(&broker, symbol).await;
//! }
//! ```

pub mod alpaca;
pub mod mock;
pub mod traits;
pub mod types;

// Re-export commonly used types for convenience
pub use alpaca::{AlpacaConfig, AlpacaProvider};
pub use mock::MockBroker;
pub use traits::{
    MarketDataProvider, OrderExecutionProvider,
    Quote, Candle, Position, Order, Account,
    OrderSide, OrderType, OrderStatus, TimeInForce, Timeframe,
};

pub use types::{
    BrokerProvider, Confidence, Message, Notes, OrderId, PatternName, Price, Quantity, Reason,
    Symbol,
};