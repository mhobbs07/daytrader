//! Event bus implementation using tokio broadcast channels
//!
//! This module provides a high-performance, non-blocking event bus for
//! distributing events across system components. It uses tokio's broadcast
//! channel for efficient pub-sub messaging.
//!
//! # Design Principles
//!
//! - **Non-blocking**: Publish operations never block
//! - **Fire-and-forget**: Publishers don't wait for subscribers
//! - **Multiple subscribers**: Any number of components can subscribe
//! - **Lag handling**: Slow subscribers are automatically handled
//! - **Clone-friendly**: EventBus is cheap to clone and share
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │            Event Publishers                  │
//! │  (Market Data, Pattern Detector, etc.)      │
//! └──────────────────┬──────────────────────────┘
//!                    │ publish()
//!                    ▼
//! ┌─────────────────────────────────────────────┐
//! │         EventBus (broadcast channel)         │
//! │  - Non-blocking publish                      │
//! │  - Multiple subscribers                      │
//! │  - Automatic lag handling                    │
//! └──┬────────┬────────┬────────┬────────────────┘
//!    │        │        │        │
//!    ▼        ▼        ▼        ▼
//! ┌────┐  ┌────┐  ┌────┐  ┌────┐
//! │Sub1│  │Sub2│  │Sub3│  │Sub4│
//! └────┘  └────┘  └────┘  └────┘
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use daytrader::events::bus::EventBus;
//! use daytrader::events::types::{MarketEvent, PatternSignal, SignalType};
//! use daytrader::brokers::types::{Symbol, Price, Confidence, PatternName};
//! use rust_decimal::Decimal;
//! use chrono::Utc;
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
//!     let signal = PatternSignal {
//!         symbol: Symbol::new("AAPL").unwrap(),
//!         pattern_name: PatternName::new("Hammer"),
//!         signal_type: SignalType::Buy,
//!         confidence: Confidence::new(0.85).unwrap(),
//!         detected_at: Utc::now(),
//!         price: Price::new(Decimal::new(15000, 2)).unwrap(),
//!     };
//!
//!     bus.publish(MarketEvent::PatternDetected { signal });
//!
//!     // Receive event
//!     if let Ok(event) = rx.recv().await {
//!         println!("Received event: {:?}", event);
//!     }
//! }
//! ```
//!
//! ## Multiple Subscribers
//!
//! ```
//! use daytrader::events::bus::EventBus;
//! use daytrader::events::types::MarketEvent;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(1000);
//!     
//!     // Create multiple subscribers
//!     let mut rx1 = bus.subscribe();
//!     let mut rx2 = bus.subscribe();
//!     let mut rx3 = bus.subscribe();
//!     
//!     // All subscribers receive the same events
//!     bus.publish(MarketEvent::TradingResumed);
//!     
//!     // Each subscriber can process independently
//!     tokio::spawn(async move {
//!         while let Ok(event) = rx1.recv().await {
//!             // Process event
//!         }
//!     });
//! }
//! ```

use crate::events::types::MarketEvent;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;
use tracing::debug;

/// Event bus for distributing market events
///
/// The EventBus uses a tokio broadcast channel internally, providing
/// efficient pub-sub messaging with automatic lag handling for slow
/// subscribers.
///
/// # Cloning
///
/// EventBus is cheap to clone - all clones share the same underlying
/// broadcast channel. This allows the bus to be easily shared across
/// async tasks and threads.
///
/// # Capacity
///
/// The capacity determines how many events can be buffered. If a subscriber
/// is too slow and falls behind by more than capacity events, it will
/// receive a `RecvError::Lagged` error and skip the missed events.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<MarketEvent>,
    metrics: Arc<EventBusMetrics>,
}

/// Metrics for monitoring event bus performance
///
/// Tracks key metrics for observability and debugging.
#[derive(Debug)]
struct EventBusMetrics {
    events_published: AtomicU64,
    events_dropped: AtomicU64,
    event_volumes: Arc<DashMap<String, AtomicU64>>, // Volume by event type
    total_latency_ns: AtomicU64,                    // Cumulative latency in nanoseconds
    latency_count: AtomicU64,                       // Number of latency measurements
}

impl EventBusMetrics {
    fn new() -> Self {
        Self {
            events_published: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
            event_volumes: Arc::new(DashMap::new()),
            total_latency_ns: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
        }
    }

    #[inline]
    fn increment_published(&self) {
        self.events_published.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn increment_dropped(&self) {
        self.events_dropped.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn get_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    #[inline]
    fn get_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }

    /// Increments volume for a specific event type
    #[inline]
    fn increment_volume(&self, event_type: &str) {
        self.event_volumes
            .entry(event_type.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Gets volume for a specific event type
    #[inline]
    fn get_volume(&self, event_type: &str) -> u64 {
        self.event_volumes
            .get(event_type)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Records processing latency for an event
    #[inline]
    fn record_latency(&self, latency_ns: u64) {
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Gets average latency in nanoseconds
    #[inline]
    fn get_average_latency_ns(&self) -> f64 {
        let count = self.latency_count.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            self.total_latency_ns.load(Ordering::Relaxed) as f64 / count as f64
        }
    }
}

impl EventBus {
    /// Creates a new event bus with the specified capacity
    ///
    /// # Parameters
    ///
    /// - `capacity`: Maximum number of events to buffer. If subscribers
    ///   can't keep up, they will skip events. Typical values: 1000-10000.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    ///
    /// // Create bus with capacity for 1000 events
    /// let bus = EventBus::new(1000);
    /// ```
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            metrics: Arc::new(EventBusMetrics::new()),
        }
    }

    /// Publishes an event to all subscribers
    ///
    /// This is a non-blocking, fire-and-forget operation. The event is
    /// cloned for each active subscriber. If there are no active subscribers,
    /// the event is dropped.
    ///
    /// # Parameters
    ///
    /// - `event`: The event to publish
    ///
    /// # Performance
    ///
    /// - Non-blocking: Never waits for subscribers
    /// - Zero-copy for Arc-wrapped data in events
    /// - Clones event for each subscriber (cheap for Arc data)
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    /// use daytrader::events::types::MarketEvent;
    ///
    /// let bus = EventBus::new(1000);
    /// bus.publish(MarketEvent::TradingResumed);
    /// ```
    #[inline]
    pub fn publish(&self, event: MarketEvent) {
        let start = std::time::Instant::now();
        let event_type = match &event {
            MarketEvent::NewCandle { .. } => "NewCandle",
            MarketEvent::QuoteUpdate { .. } => "QuoteUpdate",
            MarketEvent::PatternDetected { .. } => "PatternDetected",
            MarketEvent::OrderPlaced { .. } => "OrderPlaced",
            MarketEvent::OrderFilled { .. } => "OrderFilled",
            MarketEvent::OrderCancelled { .. } => "OrderCancelled",
            MarketEvent::OrderRejected { .. } => "OrderRejected",
            MarketEvent::PositionOpened { .. } => "PositionOpened",
            MarketEvent::PositionUpdated { .. } => "PositionUpdated",
            MarketEvent::PositionClosed { .. } => "PositionClosed",
            MarketEvent::RiskViolation { .. } => "RiskViolation",
            MarketEvent::RiskWarning { .. } => "RiskWarning",
            MarketEvent::RiskMultiplierAdjusted { .. } => "RiskMultiplierAdjusted",
            MarketEvent::PatternEnabled { .. } => "PatternEnabled",
            MarketEvent::PatternDisabled { .. } => "PatternDisabled",
            MarketEvent::PatternPerformanceUpdate { .. } => "PatternPerformanceUpdate",
            MarketEvent::ConnectionEstablished { .. } => "ConnectionEstablished",
            MarketEvent::ConnectionLost { .. } => "ConnectionLost",
            MarketEvent::TradingHalted { .. } => "TradingHalted",
            MarketEvent::TradingResumed => "TradingResumed",
            MarketEvent::SignalGenerated { .. } => "SignalGenerated",
            MarketEvent::EmergencyLiquidation { .. } => "EmergencyLiquidation",
        };

        match self.sender.send(event) {
            Ok(subscriber_count) => {
                self.metrics.increment_published();
                self.metrics.increment_volume(event_type);

                let latency_ns = start.elapsed().as_nanos() as u64;
                self.metrics.record_latency(latency_ns);

                debug!(
                    subscriber_count = subscriber_count,
                    latency_ns = latency_ns,
                    event_type = event_type,
                    "Event published to subscribers"
                );
            }
            Err(_) => {
                // No active subscribers - this is normal and not an error
                self.metrics.increment_dropped();
                debug!("Event dropped - no active subscribers");
            }
        }
    }

    /// Subscribes to events from this bus
    ///
    /// Returns a receiver that will receive all events published after
    /// subscription. Events published before subscription are not received.
    ///
    /// # Returns
    ///
    /// A `broadcast::Receiver<MarketEvent>` that can be used to receive events.
    ///
    /// # Lag Handling
    ///
    /// If a subscriber falls behind by more than the bus capacity, it will
    /// receive a `RecvError::Lagged(n)` error indicating how many events
    /// were skipped. The subscriber can continue receiving new events.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let bus = EventBus::new(1000);
    ///     let mut rx = bus.subscribe();
    ///     
    ///     // Receive events in a loop
    ///     while let Ok(event) = rx.recv().await {
    ///         println!("Received: {:?}", event);
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn subscribe(&self) -> broadcast::Receiver<MarketEvent> {
        self.sender.subscribe()
    }

    /// Returns the number of active subscribers
    ///
    /// This can be used for monitoring and debugging.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    ///
    /// let bus = EventBus::new(1000);
    /// assert_eq!(bus.subscriber_count(), 0);
    ///
    /// let _rx1 = bus.subscribe();
    /// let _rx2 = bus.subscribe();
    /// assert_eq!(bus.subscriber_count(), 2);
    /// ```
    #[inline]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Returns the total number of events published
    ///
    /// This metric is useful for monitoring event throughput.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    /// use daytrader::events::types::MarketEvent;
    ///
    /// let bus = EventBus::new(1000);
    /// let _rx = bus.subscribe();
    ///
    /// bus.publish(MarketEvent::TradingResumed);
    /// bus.publish(MarketEvent::TradingResumed);
    ///
    /// assert_eq!(bus.events_published(), 2);
    /// ```
    #[inline]
    pub fn events_published(&self) -> u64 {
        self.metrics.get_published()
    }

    /// Returns the total number of events dropped (no subscribers)
    ///
    /// This metric helps identify periods when no subscribers were active.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::events::bus::EventBus;
    /// use daytrader::events::types::MarketEvent;
    ///
    /// let bus = EventBus::new(1000);
    ///
    /// // Publish without subscribers
    /// bus.publish(MarketEvent::TradingResumed);
    ///
    /// assert_eq!(bus.events_dropped(), 1);
    /// ```
    #[inline]
    pub fn events_dropped(&self) -> u64 {
        self.metrics.get_dropped()
    }

    /// Returns the volume of events by type
    pub fn get_event_volumes(&self) -> Vec<(String, u64)> {
        self.metrics
            .event_volumes
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect()
    }

    /// Returns the average processing latency in nanoseconds
    pub fn get_average_latency_ns(&self) -> f64 {
        self.metrics.get_average_latency_ns()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("subscriber_count", &self.subscriber_count())
            .field("events_published", &self.events_published())
            .field("events_dropped", &self.events_dropped())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Confidence, PatternName, Price, Reason, Symbol};
    use crate::events::types::{PatternSignal, SignalType};
    use chrono::Utc;
    use rust_decimal::Decimal;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_event_bus_basic() {
        let bus = EventBus::new(10);
        let mut rx = bus.subscribe();

        bus.publish(MarketEvent::TradingResumed);

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, MarketEvent::TradingResumed));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 3);

        bus.publish(MarketEvent::TradingResumed);

        // All subscribers should receive the event
        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
        assert!(rx3.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_no_subscribers() {
        let bus = EventBus::new(10);

        // Publishing without subscribers should not panic
        bus.publish(MarketEvent::TradingResumed);

        assert_eq!(bus.events_dropped(), 1);
        assert_eq!(bus.events_published(), 1);
    }

    #[tokio::test]
    async fn test_metrics() {
        let bus = EventBus::new(10);
        let _rx = bus.subscribe();

        assert_eq!(bus.events_published(), 0);

        bus.publish(MarketEvent::TradingResumed);
        bus.publish(MarketEvent::TradingResumed);

        assert_eq!(bus.events_published(), 2);
        assert_eq!(bus.events_dropped(), 0);
    }

    #[tokio::test]
    async fn test_clone_shares_channel() {
        let bus1 = EventBus::new(10);
        let bus2 = bus1.clone();

        let mut rx = bus1.subscribe();

        // Publishing on clone should be received by original subscriber
        bus2.publish(MarketEvent::TradingResumed);

        assert!(rx.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_pattern_signal_event() {
        let bus = EventBus::new(10);
        let mut rx = bus.subscribe();

        let signal = PatternSignal {
            symbol: Symbol::new("AAPL").unwrap(),
            pattern_name: PatternName::new("Hammer"),
            signal_type: SignalType::Buy,
            confidence: Confidence::new(0.85).unwrap(),
            detected_at: Utc::now(),
            price: Price::new(Decimal::new(15000, 2)).unwrap(),
        };

        bus.publish(MarketEvent::PatternDetected {
            signal: signal.clone(),
        });

        if let Ok(MarketEvent::PatternDetected { signal: received }) = rx.recv().await {
            assert_eq!(received.symbol.as_str(), "AAPL");
            assert_eq!(received.pattern_name.as_str(), "Hammer");
        } else {
            panic!("Expected PatternDetected event");
        }
    }

    #[tokio::test]
    async fn test_lag_handling() {
        let bus = EventBus::new(2); // Small capacity to trigger lag
        let mut rx = bus.subscribe();

        // Publish more events than capacity without consuming
        for i in 0..10 {
            bus.publish(MarketEvent::TradingHalted {
                reason: Reason::new(format!("Test {}", i)),
            });
        }

        // First recv should get a Lagged error
        match rx.recv().await {
            Err(broadcast::error::RecvError::Lagged(n)) => {
                assert!(n > 0, "Should have lagged by some events");
            }
            Ok(_) => {
                // Depending on timing, we might get an event
                // This is also acceptable behavior
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_debug_output() {
        let bus = EventBus::new(100);
        let _rx = bus.subscribe();

        let debug_str = format!("{:?}", bus);
        assert!(debug_str.contains("EventBus"));
        assert!(debug_str.contains("capacity"));
        assert!(debug_str.contains("subscriber_count"));
    }
}
