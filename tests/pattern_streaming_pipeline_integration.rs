//! Integration tests for Pattern Streaming Pipeline (Phase 3D-1)
//!
//! Tests the end-to-end flow of:
//! 1. NewCandle events -> Candle cache
//! 2. Candle cache -> Pattern detection
//! 3. Pattern detection -> PatternDetected events
//! 4. Patterns -> Signal generation
//! 5. Signals -> SignalGenerated events
//! 6. Signals -> Database persistence

use chrono::Utc;
use daytrader::brokers::types::{Confidence, PatternName, Price, Quantity, Symbol};
use daytrader::events::types::{Candle, SignalType};
use daytrader::patterns::streaming::CandleCache;
use daytrader::patterns::traits::{PatternCategory, PatternDetector};
use daytrader::patterns::types::{PatternError, PatternParams, PatternResult};
use rust_decimal::Decimal;

// Mock pattern detector for testing
#[derive(Debug)]
struct TestHammerDetector;

impl PatternDetector for TestHammerDetector {
    fn name(&self) -> &'static str {
        "TestHammer"
    }

    fn min_candles_required(&self) -> usize {
        3
    }

    fn category(&self) -> PatternCategory {
        PatternCategory::Candlestick
    }

    fn detect(
        &self,
        candles: &[Candle],
        _params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Ok(None);
        }

        let last_candle = &candles[candles.len() - 1];

        if last_candle.is_bullish() {
            let result = PatternResult::new(
                PatternName::new("TestHammer"),
                SignalType::Buy,
                Confidence::new(0.85).unwrap(),
                last_candle.close,
            );
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
}

// Helper to create test candles
fn create_test_candle(symbol: &str, price: i64, bullish: bool) -> Candle {
    let open_price = if bullish {
        Decimal::new(price, 2)
    } else {
        Decimal::new(price + 100, 2)
    };

    let close_price = if bullish {
        Decimal::new(price + 100, 2)
    } else {
        Decimal::new(price, 2)
    };

    Candle {
        symbol: Symbol::new(symbol).unwrap(),
        timestamp: Utc::now(),
        open: Price::new(open_price).unwrap(),
        high: Price::new(Decimal::new(price + 200, 2)).unwrap(),
        low: Price::new(Decimal::new(price - 100, 2)).unwrap(),
        close: Price::new(close_price).unwrap(),
        volume: 1_000_000,
    }
}

#[test]
fn test_candle_cache_basic_operations() {
    let mut cache = CandleCache::new(100);

    // Test empty cache
    assert_eq!(cache.symbol_count(), 0);
    assert_eq!(cache.total_candles(), 0);
    assert_eq!(cache.get_all_candles("AAPL").len(), 0);

    // Add candle
    let candle = create_test_candle("AAPL", 15000, true);
    cache.add_candle("AAPL", candle.clone());

    assert_eq!(cache.symbol_count(), 1);
    assert_eq!(cache.total_candles(), 1);

    let retrieved = cache.get_all_candles("AAPL");
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].symbol.as_str(), "AAPL");
}

#[test]
fn test_candle_cache_max_size_enforcement() {
    let mut cache = CandleCache::new(5);

    // Add 10 candles (exceeding max)
    for i in 0..10 {
        let candle = create_test_candle("AAPL", 15000 + i * 100, true);
        cache.add_candle("AAPL", candle);
    }

    // Should only keep the last 5
    let candles = cache.get_all_candles("AAPL");
    assert_eq!(candles.len(), 5);
}

#[test]
fn test_candle_cache_multiple_symbols() {
    let mut cache = CandleCache::new(100);

    // Add candles for different symbols
    let candle1 = create_test_candle("AAPL", 15000, true);
    let candle2 = create_test_candle("GOOGL", 27500, false);
    let candle3 = create_test_candle("TSLA", 25000, true);

    cache.add_candle("AAPL", candle1);
    cache.add_candle("GOOGL", candle2);
    cache.add_candle("TSLA", candle3);

    assert_eq!(cache.symbol_count(), 3);
    assert_eq!(cache.total_candles(), 3);

    assert_eq!(cache.get_all_candles("AAPL").len(), 1);
    assert_eq!(cache.get_all_candles("GOOGL").len(), 1);
    assert_eq!(cache.get_all_candles("TSLA").len(), 1);
}

#[test]
fn test_candle_cache_get_recent_candles() {
    let mut cache = CandleCache::new(100);

    // Add 10 candles
    for i in 0..10 {
        let candle = create_test_candle("AAPL", 15000 + i * 100, true);
        cache.add_candle("AAPL", candle);
    }

    // Get last 5 candles
    let recent = cache.get_recent_candles("AAPL", 5);
    assert_eq!(recent.len(), 5);

    // Get more than available
    let all_as_recent = cache.get_recent_candles("AAPL", 20);
    assert_eq!(all_as_recent.len(), 10);
}

// TODO(B-056): requires external PostgreSQL database + migrations directory not available in this codebase
#[ignore]
#[tokio::test]
async fn test_end_to_end_candle_to_signal_pipeline() {
    panic!(
        "test cannot run: requires external PostgreSQL database and migrations — see TODO(B-056)"
    );
}

// TODO(B-056): requires external database connection to test error handling
#[ignore]
#[tokio::test]
async fn test_pipeline_handles_errors_gracefully() {
    panic!("test cannot run: requires external database connection — see TODO(B-056)");
}

// TODO(B-056): requires external PostgreSQL database + sqlx::migrate! + migrations directory
#[ignore]
#[tokio::test]
async fn test_pipeline_processes_multiple_symbols() {
    panic!(
        "test cannot run: requires external PostgreSQL database and migrations — see TODO(B-056)"
    );
}

// TODO(B-056): requires external PostgreSQL database connection
#[ignore]
#[tokio::test]
async fn test_pipeline_metrics_tracking() {
    panic!("test cannot run: requires external PostgreSQL database connection — see TODO(B-056)");
}

// TODO(B-056): requires external PostgreSQL database connection
#[ignore]
#[tokio::test]
async fn test_candle_cache_in_pipeline_context() {
    panic!("test cannot run: requires external PostgreSQL database connection — see TODO(B-056)");
}

/// B-085: signals published by the live pipeline must carry the symbol of the
/// candle window they were generated from (never a placeholder like
/// "DEFAULT"), and one symbol's signal must not suppress another's in the
/// signal cache.
#[tokio::test]
async fn test_pipeline_signals_carry_candle_symbol() {
    use daytrader::db::repositories::SignalRepository;
    use daytrader::events::EventBus;
    use daytrader::events::types::MarketEvent;
    use daytrader::patterns::registry::PatternRegistry;
    use daytrader::patterns::signals::{SignalConfig, SignalEngine};
    use daytrader::patterns::streaming::PatternStreamingPipeline;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;

    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(include_str!("../src/db/schema.sql"))
        .execute(&pool)
        .await
        .unwrap();

    let registry = Arc::new(PatternRegistry::new());
    registry
        .register("TestHammer", Box::new(TestHammerDetector))
        .unwrap();
    let config = SignalConfig {
        min_confidence: 0.1,
        enable_market_context: false,
        ..SignalConfig::default()
    };
    let engine = Arc::new(RwLock::new(SignalEngine::new(config)));

    let event_bus = EventBus::new(1024);
    let mut rx = event_bus.subscribe();
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        registry,
        engine,
        Arc::new(SignalRepository::new(pool)),
    );
    pipeline.start();

    // Wait for the pipeline task to subscribe before publishing candles.
    for _ in 0..100 {
        if event_bus.subscriber_count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        event_bus.subscriber_count() >= 2,
        "pipeline never subscribed"
    );

    // Interleave bullish candles for two symbols; 3+ per symbol triggers the
    // test detector.
    for i in 0..4 {
        for sym in ["AAPL", "MSFT"] {
            let candle = create_test_candle(sym, 15000 + i * 100, true);
            event_bus.publish(MarketEvent::NewCandle {
                symbol: candle.symbol.clone(),
                candle: Arc::new(candle),
            });
        }
    }

    let expected: HashSet<&str> = ["AAPL", "MSFT"].into_iter().collect();
    let mut signal_symbols: HashSet<String> = HashSet::new();
    let mut pattern_symbols: HashSet<String> = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while signal_symbols.len() < 2 && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(MarketEvent::SignalGenerated { signal })) => {
                let sym = signal.symbol.as_str().to_string();
                assert_ne!(sym, "DEFAULT", "signal published with placeholder symbol");
                assert!(expected.contains(sym.as_str()), "unexpected symbol {sym}");
                signal_symbols.insert(sym);
            }
            Ok(Ok(MarketEvent::PatternDetected { signal })) => {
                pattern_symbols.insert(signal.symbol.as_str().to_string());
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("event bus receive failed: {e:?}"),
            Err(_) => break,
        }
    }
    pipeline.stop().await;

    let both: HashSet<String> = ["AAPL".to_string(), "MSFT".to_string()].into();
    assert_eq!(pattern_symbols, both);
    assert_eq!(
        signal_symbols, both,
        "expected one signal per symbol, each tagged with its own symbol"
    );
}
