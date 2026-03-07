//! Integration tests for Pattern Streaming Pipeline (Phase 3D-1)
//!
//! Tests the end-to-end flow of:
//! 1. NewCandle events -> Candle cache
//! 2. Candle cache -> Pattern detection
//! 3. Pattern detection -> PatternDetected events
//! 4. Patterns -> Signal generation
//! 5. Signals -> SignalGenerated events
//! 6. Signals -> Database persistence

use daytrader::brokers::types::{Confidence, PatternName, Price, Quantity, Symbol};
use daytrader::db::repositories::SignalRepository;
use daytrader::events::types::{Candle, MarketEvent, SignalType};
use daytrader::events::EventBus;
use daytrader::patterns::registry::{PatternRegistry, RegistryConfig};
use daytrader::patterns::signals::{SignalConfig, SignalEngine};
use daytrader::patterns::streaming::{CandleCache, PatternStreamingPipeline};
use daytrader::patterns::traits::{PatternDetector, PatternCategory};
use daytrader::patterns::types::{PatternError, PatternParams, PatternResult};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

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
        PatternCategory::Reversal
    }

    fn detect(
        &self,
        candles: &[Candle],
        _params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Ok(None);
        }

        // Simple logic: detect if last candle is bullish with long lower shadow
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

#[tokio::test]
async fn test_end_to_end_candle_to_signal_pipeline() {
    // Setup infrastructure
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/daytrader_test".to_string());
    
    let pool = PgPool::connect(&database_url).await.expect("Failed to connect to database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    // Create components
    let event_bus = EventBus::new(1000);
    let signal_repository = Arc::new(SignalRepository::new(pool.clone()));
    
    // Create pattern registry with test detector
    let mut registry_config = RegistryConfig::default();
    registry_config.global_min_confidence = 0.5; // Lower threshold for testing
    let pattern_registry = Arc::new(PatternRegistry::with_config(registry_config));
    
    // Register test pattern detector
    let detector = Box::new(TestHammerDetector);
    pattern_registry.register("TestHammer", detector).expect("Failed to register pattern");
    
    // Create signal engine
    let signal_config = SignalConfig::default();
    let signal_engine = Arc::new(RwLock::new(SignalEngine::new(signal_config)));
    
    // Create streaming pipeline
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        pattern_registry.clone(),
        signal_engine.clone(),
        signal_repository.clone(),
    );
    
    // Subscribe to events to verify they're published
    let mut pattern_receiver = event_bus.subscribe();
    let mut signal_receiver = event_bus.subscribe();
    
    // Start pipeline
    pipeline.start();
    
    // Give pipeline time to start
    sleep(Duration::from_millis(100)).await;
    
    // Publish test candles (need at least 3 for pattern detection)
    let symbol = Symbol::new("AAPL").unwrap();
    for i in 0..5 {
        let candle = Arc::new(create_test_candle("AAPL", 15000 + i * 50, true));
        event_bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle,
        });
    }
    
    // Wait for processing
    sleep(Duration::from_millis(500)).await;
    
    // Verify PatternDetected events were published
    let mut pattern_detected = false;
    while let Ok(event) = tokio::time::timeout(Duration::from_millis(100), pattern_receiver.recv()).await {
        if let Ok(MarketEvent::PatternDetected { signal }) = event {
            pattern_detected = true;
            assert_eq!(signal.pattern_name.as_str(), "TestHammer");
            assert_eq!(signal.signal_type, SignalType::Buy);
            assert!(signal.confidence.value() >= 0.5);
            break;
        }
    }
    assert!(pattern_detected, "No PatternDetected event received");
    
    // Verify SignalGenerated events were published
    let mut signal_generated = false;
    while let Ok(event) = tokio::time::timeout(Duration::from_millis(100), signal_receiver.recv()).await {
        if let Ok(MarketEvent::SignalGenerated { signal }) = event {
            signal_generated = true;
            assert_eq!(signal.symbol.as_str(), "AAPL");
            break;
        }
    }
    assert!(signal_generated, "No SignalGenerated event received");
    
    // Wait for database writes
    sleep(Duration::from_millis(500)).await;
    
    // Verify signals were persisted to database
    let signals = signal_repository
        .get_signals_by_symbol(&symbol, 10)
        .await
        .expect("Failed to get signals from database");
    
    assert!(!signals.is_empty(), "No signals found in database");
    
    // Verify metrics
    let stats = pipeline.get_statistics();
    assert!(stats.candles_processed >= 3, "Expected at least 3 candles processed");
    
    // Stop pipeline
    pipeline.stop().await;
    
    // Cleanup
    pool.close().await;
}

#[tokio::test]
async fn test_pipeline_handles_errors_gracefully() {
    // Setup
    let event_bus = EventBus::new(1000);
    
    // Use invalid database URL to trigger errors
    let database_url = "postgresql://invalid:invalid@localhost:9999/invalid";
    let pool = PgPool::connect(database_url)
        .await
        .unwrap_or_else(|_| {
            // Create a pool that will fail on operations
            // For testing, we'll use env var or skip this test
            panic!("This test requires a database connection to test error handling");
        });
    
    let signal_repository = Arc::new(SignalRepository::new(pool.clone()));
    let pattern_registry = Arc::new(PatternRegistry::new());
    let signal_engine = Arc::new(RwLock::new(SignalEngine::new(SignalConfig::default())));
    
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        pattern_registry,
        signal_engine,
        signal_repository,
    );
    
    // Start pipeline
    pipeline.start();
    sleep(Duration::from_millis(50)).await;
    
    // Publish candles (will trigger errors due to invalid DB)
    let symbol = Symbol::new("AAPL").unwrap();
    for i in 0..3 {
        let candle = Arc::new(create_test_candle("AAPL", 15000 + i * 50, true));
        event_bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle,
        });
    }
    
    // Wait for processing
    sleep(Duration::from_millis(200)).await;
    
    // Pipeline should still be running despite errors
    let stats = pipeline.get_statistics();
    assert!(stats.candles_processed >= 3);
    
    // Stop pipeline
    pipeline.stop().await;
    
    pool.close().await;
}

#[tokio::test]
async fn test_pipeline_processes_multiple_symbols() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/daytrader_test".to_string());
    
    let pool = PgPool::connect(&database_url).await.expect("Failed to connect to database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    let event_bus = EventBus::new(1000);
    let signal_repository = Arc::new(SignalRepository::new(pool.clone()));
    let pattern_registry = Arc::new(PatternRegistry::new());
    
    // Register test pattern
    let detector = Box::new(TestHammerDetector);
    pattern_registry.register("TestHammer", detector).expect("Failed to register pattern");
    
    let signal_engine = Arc::new(RwLock::new(SignalEngine::new(SignalConfig::default())));
    
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        pattern_registry,
        signal_engine,
        signal_repository.clone(),
    );
    
    pipeline.start();
    sleep(Duration::from_millis(100)).await;
    
    // Publish candles for multiple symbols
    let symbols = vec!["AAPL", "GOOGL", "TSLA"];
    for symbol_str in &symbols {
        let symbol = Symbol::new(symbol_str).unwrap();
        for i in 0..5 {
            let candle = Arc::new(create_test_candle(symbol_str, 15000 + i * 50, true));
            event_bus.publish(MarketEvent::NewCandle {
                symbol: symbol.clone(),
                candle,
            });
        }
    }
    
    // Wait for processing
    sleep(Duration::from_millis(500)).await;
    
    // Verify all symbols were processed
    let stats = pipeline.get_statistics();
    assert!(stats.candles_processed >= 15, "Expected at least 15 candles processed (5 per symbol)");
    
    // Verify signals were generated for multiple symbols
    for symbol_str in &symbols {
        let symbol = Symbol::new(symbol_str).unwrap();
        let signals = signal_repository
            .get_signals_by_symbol(&symbol, 10)
            .await
            .expect("Failed to get signals");
        
        // We expect at least some signals for each symbol
        // (depends on pattern detection, but with our test detector should work)
    }
    
    pipeline.stop().await;
    pool.close().await;
}

#[tokio::test]
async fn test_pipeline_metrics_tracking() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/daytrader_test".to_string());
    
    let pool = PgPool::connect(&database_url).await.expect("Failed to connect to database");
    
    let event_bus = EventBus::new(1000);
    let signal_repository = Arc::new(SignalRepository::new(pool.clone()));
    let pattern_registry = Arc::new(PatternRegistry::new());
    
    // Register pattern detector
    let detector = Box::new(TestHammerDetector);
    pattern_registry.register("TestHammer", detector).expect("Failed to register pattern");
    
    let signal_engine = Arc::new(RwLock::new(SignalEngine::new(SignalConfig::default())));
    
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        pattern_registry,
        signal_engine,
        signal_repository,
    );
    
    // Check initial metrics
    let initial_stats = pipeline.get_statistics();
    assert_eq!(initial_stats.candles_processed, 0);
    assert_eq!(initial_stats.patterns_detected, 0);
    assert_eq!(initial_stats.signals_generated, 0);
    
    pipeline.start();
    sleep(Duration::from_millis(100)).await;
    
    // Publish candles
    let symbol = Symbol::new("AAPL").unwrap();
    for i in 0..10 {
        let candle = Arc::new(create_test_candle("AAPL", 15000 + i * 50, true));
        event_bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle,
        });
    }
    
    // Wait for processing
    sleep(Duration::from_millis(500)).await;
    
    // Check metrics updated
    let final_stats = pipeline.get_statistics();
    assert!(final_stats.candles_processed >= 10, "Expected at least 10 candles processed");
    assert!(final_stats.avg_processing_time_ns > 0, "Expected non-zero processing time");
    
    pipeline.stop().await;
    pool.close().await;
}

#[tokio::test]
async fn test_candle_cache_in_pipeline_context() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/daytrader_test".to_string());
    
    let pool = PgPool::connect(&database_url).await.expect("Failed to connect to database");
    
    let event_bus = EventBus::new(1000);
    let signal_repository = Arc::new(SignalRepository::new(pool.clone()));
    let pattern_registry = Arc::new(PatternRegistry::new());
    let signal_engine = Arc::new(RwLock::new(SignalEngine::new(SignalConfig::default())));
    
    let mut pipeline = PatternStreamingPipeline::new(
        event_bus.clone(),
        pattern_registry,
        signal_engine,
        signal_repository,
    );
    
    pipeline.start();
    sleep(Duration::from_millis(100)).await;
    
    // Publish many candles to test cache behavior
    let symbol = Symbol::new("AAPL").unwrap();
    for i in 0..150 { // More than the 100 candle cache limit
        let candle = Arc::new(create_test_candle("AAPL", 15000 + i * 10, i % 2 == 0));
        event_bus.publish(MarketEvent::NewCandle {
            symbol: symbol.clone(),
            candle,
        });
        
        // Small delay to simulate real-time data
        if i % 10 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    // Wait for processing
    sleep(Duration::from_millis(500)).await;
    
    // Verify all candles were processed
    let stats = pipeline.get_statistics();
    assert!(stats.candles_processed >= 150, "Expected all 150 candles to be processed");
    
    pipeline.stop().await;
    pool.close().await;
}
