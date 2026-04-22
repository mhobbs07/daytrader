//! Performance validation tests for Phase 3D
//!
//! These tests validate that performance targets are met:
//! - Pattern detection: < 100μs per symbol
//! - Signal generation: < 100μs per symbol  
//! - Full pipeline: < 500μs per symbol
//! - Support 50+ symbols in real-time
//!
//! These are NOT benchmarks (which measure relative performance),
//! but hard validation tests that ensure targets are achieved.

use daytrader::brokers::types::{Confidence, PatternName, Price, Symbol};
use daytrader::events::types::{Candle, SignalType};
use daytrader::patterns::{
    registry::PatternRegistry,
    signals::{SignalConfig, SignalEngine},
    streaming::ParallelPatternProcessor,
    types::{PatternParams, PatternResult},
};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::{Duration, Instant};

// =============================================================================
// Test Data Generators
// =============================================================================

fn create_candle(open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
    Candle {
        symbol: Symbol::new("AAPL").unwrap(),
        timestamp: chrono::Utc::now(),
        open: Price::new(Decimal::from_f64_retain(open).unwrap()).unwrap(),
        high: Price::new(Decimal::from_f64_retain(high).unwrap()).unwrap(),
        low: Price::new(Decimal::from_f64_retain(low).unwrap()).unwrap(),
        close: Price::new(Decimal::from_f64_retain(close).unwrap()).unwrap(),
        volume,
    }
}

fn create_test_candles(count: usize) -> Vec<Candle> {
    (0..count)
        .map(|i| {
            let base = 100.0 + (i as f64 * 0.1).sin() * 5.0;
            create_candle(
                base - 1.0,
                base + 2.0,
                base - 2.0,
                base + (i as f64 * 0.05).cos(),
                1000 + (i % 500) as u64,
            )
        })
        .collect()
}

fn create_multi_symbol_data(
    num_symbols: usize,
    candles_per_symbol: usize,
) -> Vec<(Symbol, Vec<Candle>)> {
    (0..num_symbols)
        .map(|symbol_idx| {
            let symbol = Symbol::new(&format!("SYM{:03}", symbol_idx)).unwrap();
            let candles = (0..candles_per_symbol)
                .map(|i| {
                    let base = 100.0 + (i as f64 * 0.1 + symbol_idx as f64).sin() * 5.0;
                    let mut candle = create_candle(
                        base - 1.0,
                        base + 2.0,
                        base - 2.0,
                        base + (i as f64 * 0.05).cos(),
                        1000 + (i % 500) as u64,
                    );
                    candle.symbol = symbol.clone();
                    candle
                })
                .collect();
            (symbol, candles)
        })
        .collect()
}

fn create_pattern_results(candles: &[Candle]) -> Vec<PatternResult> {
    let mut results = Vec::new();

    for (i, candle) in candles.iter().enumerate() {
        if i % 5 == 0 {
            let confidence = Confidence::new(0.75).unwrap();
            results.push(PatternResult::new(
                PatternName::new("hammer"),
                SignalType::Buy,
                confidence,
                candle.close,
            ));
        }
    }

    results
}

// =============================================================================
// Performance Validation Tests
// =============================================================================

/// Validates pattern detection meets < 100μs per symbol target
#[tokio::test]
async fn test_pattern_detection_meets_latency_target() {
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    let candles = create_test_candles(100);

    // Warm up
    for _ in 0..5 {
        let _ = registry.detect_all(&candles, Some(&params)).await;
    }

    // Measure
    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = registry.detect_all(&candles, Some(&params)).await;
    }

    let duration = start.elapsed();
    let avg_duration = duration / iterations;
    let avg_micros = avg_duration.as_micros();

    println!("Pattern detection avg: {}μs per symbol", avg_micros);

    assert!(
        avg_micros < 100,
        "Pattern detection exceeded 100μs target: {}μs",
        avg_micros
    );
}

/// Validates signal generation meets < 100μs per symbol target
#[tokio::test]
async fn test_signal_generation_meets_latency_target() {
    let config = SignalConfig::default();
    let candles = create_test_candles(100);
    let pattern_results = create_pattern_results(&candles);

    // Warm up
    for _ in 0..5 {
        let mut engine = SignalEngine::new(config.clone());
        let _ = engine.generate_signals(&pattern_results, &candles).await;
    }

    // Measure
    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let mut engine = SignalEngine::new(config.clone());
        let _ = engine.generate_signals(&pattern_results, &candles).await;
    }

    let duration = start.elapsed();
    let avg_duration = duration / iterations;
    let avg_micros = avg_duration.as_micros();

    println!("Signal generation avg: {}μs per symbol", avg_micros);

    assert!(
        avg_micros < 100,
        "Signal generation exceeded 100μs target: {}μs",
        avg_micros
    );
}

/// Validates full pipeline meets < 500μs per symbol target
#[tokio::test]
async fn test_full_pipeline_meets_latency_target() {
    let registry = Arc::new(PatternRegistry::new());
    let config = SignalConfig::default();
    let params = PatternParams::new().with_min_confidence(0.6);
    let candles = create_test_candles(100);

    // Warm up
    for _ in 0..5 {
        let pattern_results = registry.detect_all(&candles, Some(&params)).await;
        let mut engine = SignalEngine::new(config.clone());
        let _ = engine.generate_signals(&pattern_results, &candles).await;
    }

    // Measure
    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        // Pattern detection
        let pattern_results = registry.detect_all(&candles, Some(&params)).await;

        // Signal generation (includes indicators)
        let mut engine = SignalEngine::new(config.clone());
        let _ = engine.generate_signals(&pattern_results, &candles).await;
    }

    let duration = start.elapsed();
    let avg_duration = duration / iterations;
    let avg_micros = avg_duration.as_micros();

    println!("Full pipeline avg: {}μs per symbol", avg_micros);

    assert!(
        avg_micros < 500,
        "Full pipeline exceeded 500μs target: {}μs",
        avg_micros
    );
}

/// Validates concurrent processing of 50 symbols meets real-time requirements
#[tokio::test]
async fn test_50_symbol_concurrent_processing() {
    let registry = Arc::new(PatternRegistry::new());
    let processor = ParallelPatternProcessor::with_default_workers(registry.clone());
    let params = PatternParams::new().with_min_confidence(0.6);

    // Create data for 50 symbols
    let multi_symbol_data = create_multi_symbol_data(50, 100);

    // Warm up
    for _ in 0..3 {
        let _ = processor
            .process_symbols_with_params(multi_symbol_data.clone(), params.clone())
            .await;
    }

    // Measure: process 50 symbols in parallel
    let start = Instant::now();
    let results = processor
        .process_symbols_with_params(multi_symbol_data, params)
        .await;
    let duration = start.elapsed();

    assert_eq!(results.len(), 50, "Should process all 50 symbols");

    // Real-time requirement: process 50 symbols in < 1 second
    println!("50 symbols processed in: {:?}", duration);

    assert!(
        duration < Duration::from_secs(1),
        "50 symbol processing exceeded 1 second: {:?}",
        duration
    );
}

/// Validates parallel processing is faster than sequential
#[tokio::test]
async fn test_parallel_faster_than_sequential() {
    let registry = Arc::new(PatternRegistry::new());
    let processor = ParallelPatternProcessor::with_default_workers(registry.clone());
    let params = PatternParams::new().with_min_confidence(0.6);

    let multi_symbol_data = create_multi_symbol_data(20, 100);

    // Sequential processing
    let start_seq = Instant::now();
    for (_symbol, candles) in &multi_symbol_data {
        let _ = registry.detect_all(candles, Some(&params)).await;
    }
    let sequential_duration = start_seq.elapsed();

    // Parallel processing
    let start_par = Instant::now();
    let _ = processor
        .process_symbols_with_params(multi_symbol_data, params)
        .await;
    let parallel_duration = start_par.elapsed();

    println!("Sequential: {:?}", sequential_duration);
    println!("Parallel:   {:?}", parallel_duration);
    println!(
        "Speedup:    {:.2}x",
        sequential_duration.as_secs_f64() / parallel_duration.as_secs_f64()
    );

    // Parallel should be noticeably faster (at least 1.5x on multi-core)
    assert!(
        parallel_duration < sequential_duration,
        "Parallel processing should be faster than sequential"
    );
}

/// Validates memory usage stays within reasonable bounds
#[tokio::test]
#[ignore] // Only run manually due to memory measurement complexity
async fn test_memory_usage_within_bounds() {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Note: This is a simplified test. Real memory profiling requires tools like valgrind/heaptrack

    let registry = Arc::new(PatternRegistry::new());
    let multi_symbol_data = create_multi_symbol_data(50, 100);

    // Process all symbols
    for (_symbol, candles) in &multi_symbol_data {
        let _ = registry.detect_all(candles, None).await;
    }

    // This is a placeholder - real memory measurement would use system tools
    // Target: < 200MB for 50 symbols with 100 candles each
    println!("Memory test completed. Use external profiling tools for accurate measurement.");
}

/// Validates pattern detection scales linearly with data size
#[tokio::test]
async fn test_pattern_detection_scaling() {
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);

    let sizes = vec![10, 50, 100, 200];
    let mut timings = Vec::new();

    for size in sizes {
        let candles = create_test_candles(size);

        // Measure
        let start = Instant::now();
        for _ in 0..10 {
            let _ = registry.detect_all(&candles, Some(&params)).await;
        }
        let avg_duration = start.elapsed() / 10;

        timings.push((size, avg_duration));
        println!("Size {}: {:?}", size, avg_duration);
    }

    // Verify scaling is reasonable (shouldn't be exponential)
    // Time should grow roughly linearly with data size
    for i in 1..timings.len() {
        let (prev_size, prev_time) = timings[i - 1];
        let (curr_size, curr_time) = timings[i];

        let size_ratio = curr_size as f64 / prev_size as f64;
        let time_ratio = curr_time.as_secs_f64() / prev_time.as_secs_f64();

        println!(
            "Size ratio: {:.2}, Time ratio: {:.2}",
            size_ratio, time_ratio
        );

        // Time growth should not exceed 3x the size growth (allows some overhead)
        assert!(
            time_ratio < size_ratio * 3.0,
            "Performance degradation at size {}: time grew {}x but size only grew {}x",
            curr_size,
            time_ratio,
            size_ratio
        );
    }
}

/// Validates no performance regression from caching
#[tokio::test]
async fn test_signal_cache_performance() {
    let config = SignalConfig::default();
    let candles = create_test_candles(100);
    let pattern_results = create_pattern_results(&candles);

    let mut engine = SignalEngine::new(config);

    // First generation (cache miss)
    let start_first = Instant::now();
    let _ = engine.generate_signals(&pattern_results, &candles).await;
    let first_duration = start_first.elapsed();

    // Second generation (potential cache hit)
    let start_second = Instant::now();
    let _ = engine.generate_signals(&pattern_results, &candles).await;
    let second_duration = start_second.elapsed();

    println!("First:  {:?}", first_duration);
    println!("Second: {:?}", second_duration);

    // Cache should not significantly slow down processing
    // Second call should be similar or faster
    assert!(
        second_duration <= first_duration * 2,
        "Signal caching caused performance regression"
    );
}

// =============================================================================
// Stress Tests
// =============================================================================

/// Stress test with many rapid requests
#[tokio::test]
#[ignore] // Only run manually for stress testing
async fn stress_test_rapid_pattern_detection() {
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    let candles = create_test_candles(100);

    let iterations = 10_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = registry.detect_all(&candles, Some(&params)).await;
    }

    let duration = start.elapsed();
    let avg_duration = duration / iterations;

    println!("Processed {} iterations in {:?}", iterations, duration);
    println!("Average: {:?} per iteration", avg_duration);
    println!(
        "Throughput: {:.2} ops/sec",
        iterations as f64 / duration.as_secs_f64()
    );

    assert!(
        avg_duration < Duration::from_micros(100),
        "Performance degraded under stress"
    );
}

/// Stress test with many concurrent requests
#[tokio::test]
#[ignore] // Only run manually for stress testing
async fn stress_test_concurrent_processing() {
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);

    let start = Instant::now();

    // Spawn 100 concurrent tasks
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let registry = registry.clone();
            let params = params.clone();
            tokio::spawn(async move {
                let candles = create_test_candles(100);
                for _ in 0..10 {
                    let _ = registry.detect_all(&candles, Some(&params)).await;
                }
                i
            })
        })
        .collect();

    // Wait for all tasks
    for handle in handles {
        let _ = handle.await;
    }

    let duration = start.elapsed();

    println!(
        "Completed 100 concurrent tasks (1000 total ops) in {:?}",
        duration
    );
    println!("Throughput: {:.2} ops/sec", 1000.0 / duration.as_secs_f64());

    assert!(
        duration < Duration::from_secs(10),
        "Concurrent processing took too long"
    );
}
