//! Mock market data driver
//!
//! Owns per-symbol tokio tasks that emit synthetic `MarketEvent::QuoteUpdate`
//! and `MarketEvent::NewCandle` events at a configurable interval.  The driver
//! lives in the Tauri layer so it is free to depend on both `brokers` and
//! `events` — neither layer is violated.
//!
//! # Architecture note
//!
//! `src/brokers/` is foundation and must **not** import from `src/events/`.
//! Therefore the event-emitting loop cannot live inside `MockBroker` itself.
//! This driver is the canonical solution: it wraps the broker for subscribe
//! tracking while owning the event bus and spawning tasks independently.

use dashmap::DashMap;
use daytrader::brokers::types::{Price, Quantity, Symbol};
use daytrader::events::bus::EventBus;
use daytrader::events::types::{Candle, MarketEvent, Quote};
use rand::Rng;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// How often synthetic data is emitted per symbol (in milliseconds).
const DEFAULT_INTERVAL_MS: u64 = 1_000;

/// Number of 1-minute candles to emit before cycling (each tick = one candle).
const CANDLE_TICK_MODULO: u64 = 60;

/// Base prices used for the random-walk simulation per well-known symbol.
fn base_price_for(symbol: &str) -> Decimal {
    match symbol {
        "AAPL" => Decimal::new(18500, 2),  // $185.00
        "MSFT" => Decimal::new(37000, 2),  // $370.00
        "TSLA" => Decimal::new(20000, 2),  // $200.00
        "AMZN" => Decimal::new(17500, 2),  // $175.00
        "GOOGL" => Decimal::new(14000, 2), // $140.00
        "META" => Decimal::new(49000, 2),  // $490.00
        "NVDA" => Decimal::new(80000, 2),  // $800.00
        _ => Decimal::new(10000, 2),       // $100.00 default
    }
}

/// Driver error type
#[derive(Debug, thiserror::Error)]
pub enum MockDriverError {
    #[error("invalid symbol: {0}")]
    InvalidSymbol(String),
}

/// Manages per-symbol synthetic data streams.
///
/// - `subscribe` starts a tokio task for the symbol (idempotent).
/// - `unsubscribe` cancels the task for the symbol (idempotent).
/// - Dropping the driver cancels all tasks.
pub struct MockDataDriver {
    event_bus: EventBus,
    /// Maps symbol string to its active stream task handle.
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
    /// Interval between data emissions.
    interval_ms: u64,
}

impl MockDataDriver {
    /// Creates a new driver backed by `event_bus`, emitting at the default
    /// 1 second interval.
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            tasks: Arc::new(DashMap::new()),
            interval_ms: DEFAULT_INTERVAL_MS,
        }
    }

    /// Creates a driver with a custom emission interval (milliseconds).
    pub fn with_interval(event_bus: EventBus, interval_ms: u64) -> Self {
        Self {
            event_bus,
            tasks: Arc::new(DashMap::new()),
            interval_ms,
        }
    }

    /// Starts emitting synthetic events for `symbol`.
    ///
    /// Calling this multiple times for the same symbol is idempotent — the
    /// existing stream continues and no duplicate stream is started.
    ///
    /// # Errors
    ///
    /// Returns `MockDriverError::InvalidSymbol` if `symbol` is not a valid
    /// `Symbol` (empty or >10 chars).
    pub fn subscribe(&self, symbol: Symbol) -> Result<(), MockDriverError> {
        let key = symbol.as_str().to_string();

        // Idempotency: if already running, do nothing.
        if self.tasks.contains_key(&key) {
            debug!(symbol = %symbol, "subscribe_symbol: already streaming, skipping duplicate");
            return Ok(());
        }

        info!(symbol = %symbol, interval_ms = self.interval_ms, "Starting synthetic data stream");

        let event_bus = self.event_bus.clone();
        let interval = Duration::from_millis(self.interval_ms);

        let handle = tokio::spawn(emit_loop(symbol.clone(), event_bus, interval));

        self.tasks.insert(key, handle);
        Ok(())
    }

    /// Stops emitting events for `symbol`.
    ///
    /// Idempotent — calling this when the symbol is not streaming is a no-op.
    pub fn unsubscribe(&self, symbol: &Symbol) {
        let key = symbol.as_str();
        if let Some((_, handle)) = self.tasks.remove(key) {
            handle.abort();
            info!(symbol = %symbol, "Stopped synthetic data stream");
        } else {
            debug!(symbol = %symbol, "unsubscribe_symbol: not streaming, nothing to stop");
        }
    }

    /// Returns the number of currently active symbol streams.
    pub fn active_stream_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Drop for MockDataDriver {
    fn drop(&mut self) {
        for entry in self.tasks.iter() {
            entry.value().abort();
        }
    }
}

/// Infinite loop that emits `QuoteUpdate` and `NewCandle` events on `interval`.
///
/// Uses a random walk starting from `base_price_for(symbol)`.
async fn emit_loop(symbol: Symbol, event_bus: EventBus, interval: Duration) {
    let base = base_price_for(symbol.as_str());
    let mut current_price = base;
    let mut tick: u64 = 0;

    let mut timer = tokio::time::interval(interval);
    // Skip the immediate first tick so we don't fire before the subscriber
    // has a chance to set up its receiver.
    timer.tick().await;

    loop {
        timer.tick().await;

        let (quote, candle) = generate_tick(symbol.clone(), current_price, tick);
        // Update current_price to the candle close for a continuous walk.
        current_price = candle.close.value();
        tick = tick.wrapping_add(1);

        event_bus.publish(MarketEvent::QuoteUpdate {
            symbol: symbol.clone(),
            quote,
        });

        // Emit a new candle every CANDLE_TICK_MODULO ticks so consumers
        // accumulate a realistic 1-minute bar history.
        if tick % CANDLE_TICK_MODULO == 0 {
            let arc_candle = Arc::new(candle);
            event_bus.publish(MarketEvent::NewCandle {
                symbol: symbol.clone(),
                candle: arc_candle,
            });
            debug!(symbol = %symbol, tick, "Emitted NewCandle");
        }

        debug!(symbol = %symbol, tick, price = %current_price, "Emitted QuoteUpdate");
    }
}

/// Generates a single synthetic quote and candle for `symbol` starting from
/// `current_price` using a small random walk.
///
/// Walk step: ±0..1% of current price, drawn from a uniform distribution.
/// OHLC invariants are maintained: `low <= open,close <= high`.
fn generate_tick(symbol: Symbol, current_price: Decimal, tick: u64) -> (Quote, Candle) {
    let mut rng = rand::thread_rng();

    // Random walk: step is up to 1% of current price in either direction.
    let step_bps: i64 = rng.gen_range(-100..=100); // basis points
    let step = current_price * Decimal::new(step_bps, 4); // /10000 for bps
    let new_price = (current_price + step).max(Decimal::new(1, 2)); // floor $0.01

    // Spread: 0.01% of price
    let half_spread = (new_price * Decimal::new(5, 5)).max(Decimal::new(1, 2));
    let bid_val = new_price - half_spread;
    let ask_val = new_price + half_spread;

    let bid = Price::new(bid_val).unwrap_or_else(|_| {
        warn!("bid price construction failed, using fallback");
        Price::new(Decimal::new(1, 2)).expect("fallback price is valid")
    });
    let ask = Price::new(ask_val).unwrap_or_else(|_| {
        warn!("ask price construction failed, using fallback");
        Price::new(Decimal::new(2, 2)).expect("fallback price is valid")
    });

    let now = chrono::Utc::now();

    let quote = Quote {
        timestamp: now,
        bid,
        ask,
        bid_size: Quantity::new(100 + (tick % 400) as u32).expect("bid_size quantity always valid"),
        ask_size: Quantity::new(100 + ((tick + 200) % 400) as u32)
            .expect("ask_size quantity always valid"),
    };

    // Build OHLC: open = current_price, close = new_price
    // high = max(open, close) + small wick, low = min(open, close) - small wick
    let open_val = current_price;
    let close_val = new_price;
    let wick = (open_val * Decimal::new(2, 4)).max(Decimal::new(1, 2)); // 0.02%
    let high_val = open_val.max(close_val) + wick;
    let low_val = (open_val.min(close_val) - wick).max(Decimal::new(1, 2));

    let make_price = |v: Decimal| {
        Price::new(v)
            .unwrap_or_else(|_| Price::new(Decimal::new(1, 2)).expect("fallback price is valid"))
    };

    let candle = Candle {
        symbol: symbol.clone(),
        timestamp: now,
        open: make_price(open_val),
        high: make_price(high_val),
        low: make_price(low_val),
        close: make_price(close_val),
        volume: 1_000 + rng.gen_range(0u64..9_000),
    };

    (quote, candle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use daytrader::events::types::MarketEvent;

    #[tokio::test]
    async fn test_subscribe_emits_quote_update() {
        let bus = EventBus::new(100);
        let mut rx = bus.subscribe();
        let driver = MockDataDriver::with_interval(bus, 50); // 50ms for fast test

        let symbol = Symbol::new("AAPL").unwrap();
        driver.subscribe(symbol).unwrap();

        // Wait for at least one QuoteUpdate
        let event = tokio::time::timeout(Duration::from_millis(500), async move {
            loop {
                match rx.recv().await {
                    Ok(e @ MarketEvent::QuoteUpdate { .. }) => return Some(e),
                    Ok(_) => continue,
                    Err(_) => return None,
                }
            }
        })
        .await
        .expect("timed out waiting for QuoteUpdate");

        assert!(
            matches!(event, Some(MarketEvent::QuoteUpdate { .. })),
            "expected QuoteUpdate"
        );
    }

    #[tokio::test]
    async fn test_subscribe_idempotent() {
        let bus = EventBus::new(100);
        let driver = MockDataDriver::with_interval(bus, 100);
        let symbol = Symbol::new("MSFT").unwrap();

        driver.subscribe(symbol.clone()).unwrap();
        driver.subscribe(symbol.clone()).unwrap(); // second call must not panic

        assert_eq!(
            driver.active_stream_count(),
            1,
            "must have exactly one task"
        );
    }

    #[tokio::test]
    async fn test_unsubscribe_stops_stream() {
        let bus = EventBus::new(100);
        let driver = MockDataDriver::with_interval(bus, 50);
        let symbol = Symbol::new("TSLA").unwrap();

        driver.subscribe(symbol.clone()).unwrap();
        assert_eq!(driver.active_stream_count(), 1);

        driver.unsubscribe(&symbol);
        assert_eq!(driver.active_stream_count(), 0, "task should be removed");
    }

    #[tokio::test]
    async fn test_unsubscribe_idempotent() {
        let bus = EventBus::new(100);
        let driver = MockDataDriver::with_interval(bus, 100);
        let symbol = Symbol::new("AMZN").unwrap();

        driver.unsubscribe(&symbol); // not subscribed — must not panic
        assert_eq!(driver.active_stream_count(), 0);
    }

    #[test]
    fn test_generate_tick_ohlc_invariants() {
        let symbol = Symbol::new("AAPL").unwrap();
        let base = Decimal::new(18500, 2);

        for tick in 0u64..50 {
            let (_quote, candle) = generate_tick(symbol.clone(), base, tick);
            assert!(
                candle.high.value() >= candle.low.value(),
                "high must be >= low"
            );
            assert!(
                candle.open.value() >= candle.low.value(),
                "open must be >= low"
            );
            assert!(
                candle.open.value() <= candle.high.value(),
                "open must be <= high"
            );
            assert!(
                candle.close.value() >= candle.low.value(),
                "close must be >= low"
            );
            assert!(
                candle.close.value() <= candle.high.value(),
                "close must be <= high"
            );
        }
    }

    #[test]
    fn test_base_price_for_known_symbols() {
        assert!(base_price_for("AAPL") > Decimal::ZERO);
        assert!(base_price_for("UNKNOWN") > Decimal::ZERO);
    }
}
