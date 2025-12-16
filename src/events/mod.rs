//! Event bus system for real-time communication
//!
//! This module provides an asynchronous event bus for decoupling system components
//! using functional, iterator-based patterns. The event bus enables real-time
//! communication between market data feeds, pattern detectors, risk management,
//! execution engines, and the UI.
//!
//! # Architecture
//!
//! The event system follows a functional pub-sub pattern:
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │              Event Publishers                    │
//! │  (Market Data, Pattern Detector, Risk Mgr)      │
//! └──────────────────┬──────────────────────────────┘
//!                    │
//!                    ▼
//! ┌─────────────────────────────────────────────────┐
//! │         EventBus (broadcast channel)             │
//! │  - Non-blocking publish                          │
//! │  - Multiple subscribers                          │
//! │  - Automatic lag handling                        │
//! └──┬────────┬────────┬────────┬────────────────────┘
//!    │        │        │        │
//!    ▼        ▼        ▼        ▼
//! ┌────┐  ┌────┐  ┌────┐  ┌────┐
//! │Pat │  │Risk│  │Exec│  │ UI │
//! │Det │  │Mgr │  │Eng │  │    │
//! └────┘  └────┘  └────┘  └────┘
//! ```
//!
//! # Key Features
//!
//! - **Non-blocking**: Publish operations never block
//! - **Type-safe**: Strongly-typed events prevent errors
//! - **Zero-copy**: Uses `Arc<T>` for large data structures
//! - **Functional**: Composable filters for event streams
//! - **Performant**: Minimal allocations, inline hot paths
//!
//! # Design Principles
//!
//! 1. **Immutable Events**: Events cannot be modified after creation
//! 2. **Zero-Copy Distribution**: Use `Arc<T>` for large data (Candle, Order)
//! 3. **Functional Filters**: Composable, lazy stream transformations
//! 4. **Fire-and-Forget**: Publishers don't wait for subscribers
//! 5. **Lag Handling**: Slow subscribers skip events rather than block
//!
//! # Examples
//!
//! ## Basic Event Bus Usage
//!
//! ```
//! use daytrader::events::{EventBus, MarketEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create event bus with capacity for 1000 events
//!     let bus = EventBus::new(1000);
//!     
//!     // Subscribe to events
//!     let mut rx = bus.subscribe();
//!     
//!     // Publish an event
//!     bus.publish(MarketEvent::TradingResumed);
//!     
//!     // Receive event
//!     if let Ok(event) = rx.recv().await {
//!         println!("Received: {:?}", event);
//!     }
//! }
//! ```
//!
//! ## Filtering Events
//!
//! ```
//! use daytrader::events::{EventBus, filters};
//! use daytrader::brokers::types::Symbol;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(1000);
//!     let rx = bus.subscribe();
//!     
//!     // Filter for candles from AAPL only
//!     let symbol = Symbol::new("AAPL").unwrap();
//!     let mut stream = filters::filter_candles(
//!         filters::filter_symbol(rx, symbol)
//!     );
//!     
//!     while let Some((symbol, candle)) = stream.next().await {
//!         println!("AAPL candle: ${}", candle.close);
//!     }
//! }
//! ```
//!
//! ## Pattern Detection Pipeline
//!
//! ```
//! use daytrader::events::{EventBus, filters, MarketEvent, PatternSignal};
//! use daytrader::brokers::types::{Symbol, Price, Confidence};
//! use futures::StreamExt;
//! use rust_decimal::Decimal;
//! use chrono::Utc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(1000);
//!     
//!     // Pattern detector subscribes to candles
//!     let rx1 = bus.subscribe();
//!     tokio::spawn(async move {
//!         let mut candles = filters::filter_candles(rx1);
//!         while let Some((symbol, candle)) = candles.next().await {
//!             // Detect patterns and publish signals
//!             // bus.publish(MarketEvent::PatternDetected { signal });
//!         }
//!     });
//!     
//!     // Risk manager subscribes to pattern signals
//!     let rx2 = bus.subscribe();
//!     tokio::spawn(async move {
//!         let mut patterns = filters::filter_high_confidence_patterns(rx2, 0.8);
//!         while let Some(signal) = patterns.next().await {
//!             // Validate risk rules
//!             println!("High-confidence pattern: {}", signal.pattern_name);
//!         }
//!     });
//! }
//! ```
//!
//! # Module Organization
//!
//! - [`types`]: Event type definitions and helper structures
//! - [`bus`]: EventBus implementation using tokio broadcast channels
//! - [`filters`]: Functional filters for composable event stream processing

pub mod bus;
pub mod filters;
pub mod types;

// Re-export commonly used types for convenience
pub use bus::EventBus;
pub use types::{
    Candle, Fill, MarketEvent, Order, OrderSide, OrderStatus, OrderType,
    PatternSignal, Position, Quote, RiskViolation, RiskViolationType, SignalType,
};

// Re-export broker types that are used in events
pub use crate::brokers::types::{Confidence, OrderId, Price, Quantity, Symbol};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::brokers::types::{Confidence, PatternName, Price, Symbol};
    use chrono::Utc;
    use futures::StreamExt;
    use rust_decimal::Decimal;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_end_to_end_event_flow() {
        // Create event bus
        let bus = EventBus::new(100);

        // Create multiple subscribers
        let rx1 = bus.subscribe();
        let rx2 = bus.subscribe();

        // Subscriber 1: Filter for candles
        let handle1 = tokio::spawn(async move {
            let mut stream = filters::filter_candles(rx1);
            let mut count = 0;
            while let Some(_) = stream.next().await {
                count += 1;
                if count >= 2 {
                    break;
                }
            }
            count
        });

        // Subscriber 2: Filter for patterns
        let handle2 = tokio::spawn(async move {
            let mut stream = filters::filter_patterns(rx2);
            let mut count = 0;
            while let Some(_) = stream.next().await {
                count += 1;
                if count >= 1 {
                    break;
                }
            }
            count
        });

        // Publish events
        let symbol = Symbol::new("AAPL").unwrap();

        // Publish candle
        let candle = Arc::new(types::Candle {
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        });

        bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle: candle.clone(),
        });

        // Publish pattern
        let signal = types::PatternSignal {
            symbol: symbol.clone(),
            pattern_name: PatternName::new("Hammer"),
            signal_type: types::SignalType::Buy,
            confidence: Confidence::new(0.85).unwrap(),
            detected_at: Utc::now(),
            price: Price::new(Decimal::new(105, 0)).unwrap(),
        };

        bus.publish(MarketEvent::PatternDetected { signal });

        // Publish another candle
        bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle: candle.clone(),
        });

        // Wait for subscribers to process
        let count1 = handle1.await.unwrap();
        let count2 = handle2.await.unwrap();

        assert_eq!(count1, 2, "Should receive 2 candles");
        assert_eq!(count2, 1, "Should receive 1 pattern");
    }

    #[tokio::test]
    async fn test_multiple_symbol_filtering() {
        let bus = EventBus::new(100);
        let rx = bus.subscribe();

        let aapl = Symbol::new("AAPL").unwrap();
        let googl = Symbol::new("GOOGL").unwrap();

        let watchlist = vec![aapl.clone(), googl.clone()];

        let handle = tokio::spawn(async move {
            let mut stream = filters::filter_symbols(rx, watchlist);
            let mut symbols = Vec::new();
            for _ in 0..2 {
                if let Some(event) = stream.next().await {
                    if let MarketEvent::NewCandle { symbol, .. } = event {
                        symbols.push(symbol);
                    }
                }
            }
            symbols
        });

        // Publish candles for different symbols
        let candle = Arc::new(types::Candle {
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        });

        bus.publish(MarketEvent::NewCandle {
            symbol: aapl.clone(),
            candle: candle.clone(),
        });

        bus.publish(MarketEvent::NewCandle {
            symbol: Symbol::new("TSLA").unwrap(), // Not in watchlist
            candle: candle.clone(),
        });

        bus.publish(MarketEvent::NewCandle {
            symbol: googl.clone(),
            candle: candle.clone(),
        });

        let symbols = handle.await.unwrap();
        assert_eq!(symbols.len(), 2);
        assert!(symbols.iter().any(|s| s.as_str() == "AAPL"));
        assert!(symbols.iter().any(|s| s.as_str() == "GOOGL"));
    }

    #[tokio::test]
    async fn test_event_bus_metrics() {
        let bus = EventBus::new(100);
        let _rx = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 1);
        assert_eq!(bus.events_published(), 0);

        bus.publish(MarketEvent::TradingResumed);
        bus.publish(MarketEvent::TradingResumed);

        assert_eq!(bus.events_published(), 2);
    }
}