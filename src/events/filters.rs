//! Functional event filters for composable event stream processing
//!
//! This module provides functional, iterator-like filters for processing
//! event streams. Filters are composable, lazy, and zero-copy where possible.
//!
//! # Design Principles
//!
//! - **Functional**: Pure functions that transform streams
//! - **Lazy**: No processing until stream is consumed
//! - **Composable**: Filters can be chained together
//! - **Zero-copy**: Uses references and Arc to avoid cloning
//! - **Type-safe**: Returns typed streams, not trait objects
//!
//! # Architecture
//!
//! ```text
//! EventBus → subscribe() → Receiver<MarketEvent>
//!                              ↓
//!                         filter_candles()
//!                              ↓
//!                    Stream<(Symbol, Arc<Candle>)>
//!                              ↓
//!                       filter_symbol("AAPL")
//!                              ↓
//!                    Stream<(Symbol, Arc<Candle>)>
//!                              ↓
//!                          collect() / for_each()
//! ```
//!
//! # Examples
//!
//! ## Filter by Event Type
//!
//! ```
//! use daytrader::events::bus::EventBus;
//! use daytrader::events::filters::filter_candles;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(1000);
//!     let rx = bus.subscribe();
//!     
//!     // Filter for candle events only
//!     let mut candle_stream = filter_candles(rx);
//!     
//!     while let Some((symbol, candle)) = candle_stream.next().await {
//!         println!("Candle for {}: ${}", symbol, candle.close);
//!     }
//! }
//! ```
//!
//! ## Filter by Symbol
//!
//! ```
//! use daytrader::events::bus::EventBus;
//! use daytrader::events::filters::{filter_candles, filter_symbol};
//! use daytrader::brokers::types::Symbol;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(1000);
//!     let rx = bus.subscribe();
//!     
//!     let symbol = Symbol::new("AAPL").unwrap();
//!     
//!     // Chain filters: candles for AAPL only
//!     let mut stream = filter_candles(filter_symbol(rx, symbol));
//!     
//!     while let Some((sym, candle)) = stream.next().await {
//!         println!("AAPL candle: ${}", candle.close);
//!     }
//! }
//! ```

use crate::brokers::types::{OrderId, PatternName, Symbol};
use crate::events::types::{Candle, MarketEvent, Order, PatternSignal, RiskViolation};
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

/// Converts a broadcast receiver into a stream
///
/// This is a helper function that wraps a broadcast receiver in a stream
/// adapter, handling lagged messages gracefully.
///
/// # Parameters
///
/// - `rx`: The broadcast receiver to convert
///
/// # Returns
///
/// A stream of `MarketEvent` items
pub fn to_stream(rx: broadcast::Receiver<MarketEvent>) -> Pin<Box<dyn Stream<Item = MarketEvent> + Send>> {
    Box::pin(BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => Some(event),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                tracing::warn!("Event stream lagged by {} events", n);
                None
            }
        }
    }))
}

/// Filters events to extract only new candle events
///
/// Returns a stream of `(Symbol, Arc<Candle>)` tuples containing only
/// candle events. Other event types are filtered out.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
///
/// # Returns
///
/// A stream of `(Symbol, Arc<Candle>)` tuples
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_candles;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let mut candle_stream = filter_candles(rx);
///     
///     while let Some((symbol, candle)) = candle_stream.next().await {
///         println!("Received candle for {}", symbol);
///     }
/// }
/// ```
pub fn filter_candles(
    stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>,
) -> Pin<Box<dyn Stream<Item = (Symbol, Arc<Candle>)> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        if let MarketEvent::NewCandle { symbol, candle } = event {
            Some((symbol, candle))
        } else {
            None
        }
    }))
}

/// Filters events to extract only pattern detection events
///
/// Returns a stream of `PatternSignal` items containing only pattern
/// detection events. Other event types are filtered out.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
///
/// # Returns
///
/// A stream of `PatternSignal` items
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_patterns;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let mut pattern_stream = filter_patterns(rx);
///     
///     while let Some(signal) = pattern_stream.next().await {
///         println!("Pattern detected: {} ({})", 
///                  signal.pattern_name, signal.confidence);
///     }
/// }
/// ```
pub fn filter_patterns(
    stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>,
) -> Pin<Box<dyn Stream<Item = PatternSignal> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        if let MarketEvent::PatternDetected { signal } = event {
            Some(signal)
        } else {
            None
        }
    }))
}

/// Filters events to extract only order placement events
///
/// Returns a stream of `Arc<Order>` items containing only order placement
/// events. Other event types are filtered out.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
///
/// # Returns
///
/// A stream of `Arc<Order>` items
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_orders;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let mut order_stream = filter_orders(rx);
///     
///     while let Some(order) = order_stream.next().await {
///         println!("Order placed: {} {} {}", 
///                  order.side, order.quantity, order.symbol);
///     }
/// }
/// ```
pub fn filter_orders(stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>) -> Pin<Box<dyn Stream<Item = Arc<Order>> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        if let MarketEvent::OrderPlaced { order } = event {
            Some(order)
        } else {
            None
        }
    }))
}

/// Filters events to extract only order filled events
///
/// Returns a stream of `OrderId` items for filled orders.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
///
/// # Returns
///
/// A stream of `OrderId` items
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_fills;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let mut fill_stream = filter_fills(rx);
///     
///     while let Some(order_id) = fill_stream.next().await {
///         println!("Order filled: {}", order_id);
///     }
/// }
/// ```
pub fn filter_fills(stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>) -> Pin<Box<dyn Stream<Item = OrderId> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        if let MarketEvent::OrderFilled { order_id, .. } = event {
            Some(order_id)
        } else {
            None
        }
    }))
}

/// Filters events to include only those for a specific symbol
///
/// This is a generic filter that can be applied to any event stream.
/// It filters events to include only those matching the specified symbol.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
/// - `target_symbol`: The symbol to filter for
///
/// # Returns
///
/// A stream of `MarketEvent` items for the specified symbol
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_symbol;
/// use daytrader::brokers::types::Symbol;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let symbol = Symbol::new("AAPL").unwrap();
///     let mut stream = filter_symbol(rx, symbol);
///     
///     while let Some(event) = stream.next().await {
///         println!("Event for AAPL: {:?}", event);
///     }
/// }
/// ```
pub fn filter_symbol(
    stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>,
    target_symbol: Symbol,
) -> Pin<Box<dyn Stream<Item = MarketEvent> + Send>> {
    let target = target_symbol.clone();
    Box::pin(stream.filter(move |event| {
        let matches = match event {
            MarketEvent::NewCandle { symbol, .. } => symbol == &target,
            MarketEvent::QuoteUpdate { symbol, .. } => symbol == &target,
            MarketEvent::PatternDetected { signal } => signal.symbol == target.clone(),
            MarketEvent::OrderPlaced { order } => order.symbol == target.clone(),
            MarketEvent::PositionOpened { position } => position.symbol == target.clone(),
            MarketEvent::PositionUpdated { position } => position.symbol == target.clone(),
            MarketEvent::PositionClosed { symbol, .. } => symbol == &target,
            // System events and other events don't have symbols
            _ => false,
        };
        async move { matches }
    }))
}

/// Filters events to include only those matching a list of symbols
///
/// Similar to [`filter_symbol`] but accepts multiple symbols.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
/// - `symbols`: Vector of symbols to filter for
///
/// # Returns
///
/// A stream of `MarketEvent` items for any of the specified symbols
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_symbols;
/// use daytrader::brokers::types::Symbol;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     let symbols = vec![
///         Symbol::new("AAPL").unwrap(),
///         Symbol::new("GOOGL").unwrap(),
///         Symbol::new("MSFT").unwrap(),
///     ];
///     
///     let mut stream = filter_symbols(rx, symbols);
///     
///     while let Some(event) = stream.next().await {
///         println!("Event for watchlist: {:?}", event);
///     }
/// }
/// ```
pub fn filter_symbols(
    stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>,
    symbols: Vec<Symbol>,
) -> Pin<Box<dyn Stream<Item = MarketEvent> + Send>> {
    let symbols = symbols.clone();
    Box::pin(stream.filter(move |event| {
        let event_symbol = match &event {
            MarketEvent::NewCandle { symbol, .. } => Some(symbol),
            MarketEvent::QuoteUpdate { symbol, .. } => Some(symbol),
            MarketEvent::PatternDetected { signal } => Some(&signal.symbol),
            MarketEvent::OrderPlaced { order } => Some(&order.symbol),
            MarketEvent::PositionOpened { position } => Some(&position.symbol),
            MarketEvent::PositionUpdated { position } => Some(&position.symbol),
            MarketEvent::PositionClosed { symbol, .. } => Some(symbol),
            _ => None,
        };
        let matches = event_symbol.map_or(false, |sym| symbols.contains(sym));
        async move { matches }
    }))
}

/// Filters pattern signals by minimum confidence threshold
///
/// Returns a stream of pattern signals that meet or exceed the specified
/// confidence threshold.
///
/// # Parameters
///
/// - `rx`: Broadcast receiver for market events
/// - `min_confidence`: Minimum confidence score (0.0 to 1.0)
///
/// # Returns
///
/// A stream of `PatternSignal` items with confidence >= threshold
///
/// # Examples
///
/// ```
/// use daytrader::events::bus::EventBus;
/// use daytrader::events::filters::filter_high_confidence_patterns;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = EventBus::new(1000);
///     let rx = bus.subscribe();
///     
///     // Only patterns with 80%+ confidence
///     let mut stream = filter_high_confidence_patterns(rx, 0.8);
///     
///     while let Some(signal) = stream.next().await {
///         println!("High-confidence pattern: {} ({})", 
///                  signal.pattern_name, signal.confidence);
///     }
/// }
/// ```
pub fn filter_high_confidence_patterns(
    stream: Pin<Box<dyn Stream<Item = MarketEvent> + Send>>,
    min_confidence: f64,
) -> Pin<Box<dyn Stream<Item = PatternSignal> + Send>> {
    Box::pin(filter_patterns(stream).filter(move |signal| {
        let matches = signal.confidence.value() >= min_confidence;
        async move { matches }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Confidence, PatternName, Price, Reason};
    use crate::events::bus::EventBus;
    use crate::events::types::SignalType;
    use chrono::Utc;
    use futures::StreamExt;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn test_filter_candles() {
        let bus = EventBus::new(10);
        let rx = bus.subscribe();

        let symbol = Symbol::new("AAPL").unwrap();
        let candle = Arc::new(Candle {
            symbol: symbol.clone(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        });

        // Publish mixed events
        bus.publish(MarketEvent::TradingResumed);
        bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle: candle.clone(),
        });
        bus.publish(MarketEvent::TradingHalted {
            reason: Reason::new("test"),
        });

        let mut stream = filter_candles(to_stream(rx));

        // Should only receive the candle event
        if let Some((recv_symbol, recv_candle)) = stream.next().await {
            assert_eq!(recv_symbol.as_str(), "AAPL");
            assert_eq!(recv_candle.close, candle.close);
        } else {
            panic!("Expected candle event");
        }
    }

    #[tokio::test]
    async fn test_filter_patterns() {
        let bus = EventBus::new(10);
        let rx = bus.subscribe();

        let signal = PatternSignal {
            symbol: Symbol::new("AAPL").unwrap(),
            pattern_name: PatternName::new("Hammer"),
            signal_type: SignalType::Buy,
            confidence: Confidence::new(0.85).unwrap(),
            detected_at: Utc::now(),
            price: Price::new(Decimal::new(15000, 2)).unwrap(),
        };

        bus.publish(MarketEvent::TradingResumed);
        bus.publish(MarketEvent::PatternDetected {
            signal: signal.clone(),
        });

        let mut stream = filter_patterns(to_stream(rx));

        if let Some(recv_signal) = stream.next().await {
            assert_eq!(recv_signal.pattern_name.as_str(), "Hammer");
            assert_eq!(recv_signal.symbol.as_str(), "AAPL");
        } else {
            panic!("Expected pattern event");
        }
    }

    #[tokio::test]
    async fn test_filter_symbol() {
        let bus = EventBus::new(10);
        let rx = bus.subscribe();

        let aapl = Symbol::new("AAPL").unwrap();
        let googl = Symbol::new("GOOGL").unwrap();

        let candle = Arc::new(Candle {
            symbol: aapl.clone(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        });

        // Publish events for different symbols
        bus.publish(MarketEvent::NewCandle {
            symbol: aapl.clone(),
            candle: candle.clone(),
        });
        bus.publish(MarketEvent::NewCandle {
            symbol: googl.clone(),
            candle: candle.clone(),
        });

        let mut stream = filter_symbol(to_stream(rx), aapl.clone());

        // Should only receive AAPL events
        if let Some(MarketEvent::NewCandle { symbol, .. }) = stream.next().await {
            assert_eq!(symbol.as_str(), "AAPL");
        } else {
            panic!("Expected AAPL candle event");
        }
    }

    #[tokio::test]
    async fn test_filter_high_confidence() {
        let bus = EventBus::new(10);
        let rx = bus.subscribe();

        let high_conf = PatternSignal {
            symbol: Symbol::new("AAPL").unwrap(),
            pattern_name: PatternName::new("Hammer"),
            signal_type: SignalType::Buy,
            confidence: Confidence::new(0.85).unwrap(),
            detected_at: Utc::now(),
            price: Price::new(Decimal::new(15000, 2)).unwrap(),
        };

        let low_conf = PatternSignal {
            symbol: Symbol::new("GOOGL").unwrap(),
            pattern_name: PatternName::new("Doji"),
            signal_type: SignalType::Buy,
            confidence: Confidence::new(0.65).unwrap(),
            detected_at: Utc::now(),
            price: Price::new(Decimal::new(28000, 2)).unwrap(),
        };

        bus.publish(MarketEvent::PatternDetected {
            signal: low_conf.clone(),
        });
        bus.publish(MarketEvent::PatternDetected {
            signal: high_conf.clone(),
        });

        let mut stream = filter_high_confidence_patterns(to_stream(rx), 0.8);

        // Should only receive high-confidence pattern
        if let Some(signal) = stream.next().await {
            assert_eq!(signal.pattern_name.as_str(), "Hammer");
            assert!(signal.confidence.value() >= 0.8);
        } else {
            panic!("Expected high-confidence pattern");
        }
    }

    #[tokio::test]
    async fn test_chained_filters() {
        let bus = EventBus::new(10);
        let rx = bus.subscribe();

        let aapl = Symbol::new("AAPL").unwrap();
        let candle = Arc::new(Candle {
            symbol: aapl.clone(),
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
        bus.publish(MarketEvent::TradingResumed);

        // Chain filters: symbol filter -> candle filter
        let mut stream = filter_candles(filter_symbol(to_stream(rx), aapl.clone()));
        tokio::pin!(stream);

        if let Some((symbol, _)) = stream.next().await {
            assert_eq!(symbol.as_str(), "AAPL");
        } else {
            panic!("Expected filtered candle");
        }
    }
}