//! Performance benchmarks for pattern detection algorithms
//!
//! These benchmarks establish baseline metrics for:
//! - Individual pattern detection speed (Hammer, Shooting Star, Doji, Engulfing)
//! - Pattern registry performance
//! - Memory usage patterns
//! - Concurrent detection performance
//! - Different data sizes and complexities

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use daytrader::brokers::types::{Price, Symbol};
use daytrader::events::types::Candle;
use daytrader::patterns::{
    candlestick::{DojiDetector, HammerDetector, ShootingStarDetector},
    engulfing::{BearishEngulfingDetector, BullishEngulfingDetector},
    registry::PatternRegistry,
    traits::PatternDetector,
    types::PatternParams,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark individual hammer pattern detection
pub fn bench_hammer_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let detector = HammerDetector::new();
    let params = PatternParams::new().with_min_confidence(0.0); // Accept all detections for benchmarking

    // Create test data with various hammer patterns
    let test_candles = create_hammer_test_data();

    c.bench_function("hammer_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(detector.detect(black_box(candles), black_box(&params)));
                }
            });
        });
    });
}

/// Benchmark individual shooting star pattern detection
pub fn bench_shooting_star_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let detector = ShootingStarDetector::new();
    let params = PatternParams::new().with_min_confidence(0.0);

    let test_candles = create_shooting_star_test_data();

    c.bench_function("shooting_star_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(detector.detect(black_box(candles), black_box(&params)));
                }
            });
        });
    });
}

/// Benchmark individual doji pattern detection
pub fn bench_doji_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let detector = DojiDetector::new();
    let params = PatternParams::new().with_min_confidence(0.0);

    let test_candles = create_doji_test_data();

    c.bench_function("doji_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(detector.detect(black_box(candles), black_box(&params)));
                }
            });
        });
    });
}

/// Benchmark bullish engulfing pattern detection
pub fn bench_bullish_engulfing_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let detector = BullishEngulfingDetector::new();
    let params = PatternParams::new().with_min_confidence(0.0);

    let test_candles = create_bullish_engulfing_test_data();

    c.bench_function("bullish_engulfing_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(detector.detect(black_box(candles), black_box(&params)));
                }
            });
        });
    });
}

/// Benchmark bearish engulfing pattern detection
pub fn bench_bearish_engulfing_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let detector = BearishEngulfingDetector::new();
    let params = PatternParams::new().with_min_confidence(0.0);

    let test_candles = create_bearish_engulfing_test_data();

    c.bench_function("bearish_engulfing_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(detector.detect(black_box(candles), black_box(&params)));
                }
            });
        });
    });
}

/// Benchmark pattern registry performance
pub fn bench_pattern_registry(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.0);

    // Create mixed test data
    let test_candles = create_mixed_pattern_test_data();

    c.bench_function("pattern_registry_detect_all", |b| {
        b.iter(|| {
            rt.block_on(async {
                for candles in &test_candles {
                    let _ = black_box(registry.detect_all(black_box(candles), Some(&params)).await);
                }
            });
        });
    });
}

/// Benchmark concurrent pattern detection
pub fn bench_concurrent_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.0);

    let test_candles = create_mixed_pattern_test_data();

    c.bench_function("concurrent_pattern_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut handles = vec![];

                // Spawn concurrent detection tasks
                for chunk in test_candles.chunks(10) {
                    let registry_clone = Arc::clone(&registry);
                    let chunk_data: Vec<_> = chunk.to_vec();
                    let params = params.clone();

                    let handle = tokio::spawn(async move {
                        for candles in chunk_data {
                            let _ = registry_clone.detect_all(&candles, Some(&params)).await;
                        }
                    });
                    handles.push(handle);
                }

                // Wait for all tasks to complete
                for handle in handles {
                    let _ = handle.await;
                }
            });
        });
    });
}

/// Benchmark performance with different data sizes
pub fn bench_data_size_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.0);

    let mut group = c.benchmark_group("data_size_scaling");

    for size in [10, 50, 100, 500, 1000].iter() {
        let test_candles = create_large_test_data(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _size| {
            b.iter(|| {
                rt.block_on(async {
                    let _ = black_box(
                        registry
                            .detect_all(black_box(&test_candles), Some(&params))
                            .await,
                    );
                });
            });
        });
    }
    group.finish();
}

/// Benchmark memory usage patterns
pub fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let params = PatternParams::new().with_min_confidence(0.0);

    c.bench_function("memory_usage_patterns", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create multiple registry instances to test memory allocation
                let mut registries = vec![];
                for _ in 0..5 {
                    registries.push(Arc::new(PatternRegistry::new()));
                }

                // Perform detections
                let test_candles = create_mixed_pattern_test_data();
                for registry in &registries {
                    for candles in &test_candles {
                        let _ = registry.detect_all(candles, Some(&params)).await;
                    }
                }

                black_box(registries);
            });
        });
    });
}

/// Benchmark pattern detection throughput
pub fn bench_detection_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.0);

    // Create a large dataset for throughput testing
    let test_candles = create_large_test_data(1000);

    c.bench_function("detection_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(
                    registry
                        .detect_all(black_box(&test_candles), Some(&params))
                        .await,
                );
            });
        });
    });
}

/// Helper function to create hammer test data
fn create_hammer_test_data() -> Vec<Vec<Candle>> {
    vec![
        // Perfect hammer pattern
        vec![create_candle(100.0, 105.0, 95.0, 103.0, 1000)],
        // Hammer with volume confirmation
        vec![
            create_candle(100.0, 105.0, 95.0, 103.0, 1000),
            create_candle(102.0, 104.0, 101.0, 103.0, 800),
        ],
        // Non-hammer pattern
        vec![create_candle(100.0, 110.0, 99.0, 105.0, 1000)],
    ]
}

/// Helper function to create shooting star test data
fn create_shooting_star_test_data() -> Vec<Vec<Candle>> {
    vec![
        // Perfect shooting star
        vec![create_candle(100.0, 115.0, 98.0, 102.0, 1000)],
        // Shooting star with volume
        vec![
            create_candle(100.0, 115.0, 98.0, 102.0, 1000),
            create_candle(103.0, 106.0, 101.0, 104.0, 900),
        ],
        // Non-shooting star
        vec![create_candle(100.0, 105.0, 95.0, 103.0, 1000)],
    ]
}

/// Helper function to create doji test data
fn create_doji_test_data() -> Vec<Vec<Candle>> {
    vec![
        // Standard doji
        vec![create_candle(100.0, 101.0, 99.0, 100.1, 1000)],
        // Dragonfly doji
        vec![create_candle(100.0, 101.0, 95.0, 100.1, 1000)],
        // Gravestone doji
        vec![create_candle(100.0, 105.0, 99.0, 100.1, 1000)],
        // Non-doji
        vec![create_candle(100.0, 110.0, 95.0, 105.0, 1000)],
    ]
}

/// Helper function to create bullish engulfing test data
fn create_bullish_engulfing_test_data() -> Vec<Vec<Candle>> {
    vec![
        // Perfect bullish engulfing
        vec![
            create_candle(105.0, 106.0, 102.0, 103.0, 1000), // Bearish candle
            create_candle(103.0, 108.0, 101.0, 107.0, 1200), // Bullish engulfing
        ],
        // Bullish engulfing with gap
        vec![
            create_candle(105.0, 106.0, 102.0, 103.0, 1000),
            create_candle(102.0, 108.0, 101.0, 107.0, 1200),
        ],
    ]
}

/// Helper function to create bearish engulfing test data
fn create_bearish_engulfing_test_data() -> Vec<Vec<Candle>> {
    vec![
        // Perfect bearish engulfing
        vec![
            create_candle(100.0, 105.0, 99.0, 104.0, 1000), // Bullish candle
            create_candle(104.0, 106.0, 98.0, 99.0, 1200),  // Bearish engulfing
        ],
        // Bearish engulfing with gap
        vec![
            create_candle(100.0, 105.0, 99.0, 104.0, 1000),
            create_candle(105.0, 107.0, 98.0, 99.0, 1200),
        ],
    ]
}

/// Helper function to create mixed pattern test data
fn create_mixed_pattern_test_data() -> Vec<Vec<Candle>> {
    let mut data = vec![];
    data.extend(create_hammer_test_data());
    data.extend(create_shooting_star_test_data());
    data.extend(create_doji_test_data());
    data.extend(create_bullish_engulfing_test_data());
    data.extend(create_bearish_engulfing_test_data());
    data
}

/// Helper function to create large test data
fn create_large_test_data(size: usize) -> Vec<Candle> {
    (0..size)
        .map(|i| {
            let base = 100.0 + (i as f64 * 0.1).sin() * 5.0;
            create_candle(
                base - 2.0,
                base + 3.0,
                base - 3.0,
                base + (i as f64 * 0.05).cos(),
                1000 + (i % 500) as u64,
            )
        })
        .collect()
}

/// Helper function to create a candle
fn create_candle(open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
    Candle {
        symbol: Symbol::new("BENCH").unwrap(),
        timestamp: chrono::Utc::now(),
        open: Price::new(Decimal::from_f64_retain(open).unwrap()).unwrap(),
        high: Price::new(Decimal::from_f64_retain(high).unwrap()).unwrap(),
        low: Price::new(Decimal::from_f64_retain(low).unwrap()).unwrap(),
        close: Price::new(Decimal::from_f64_retain(close).unwrap()).unwrap(),
        volume,
    }
}

criterion_group!(
    benches,
    bench_hammer_detection,
    bench_shooting_star_detection,
    bench_doji_detection,
    bench_bullish_engulfing_detection,
    bench_bearish_engulfing_detection,
    bench_pattern_registry,
    bench_concurrent_detection,
    bench_data_size_scaling,
    bench_memory_usage,
    bench_detection_throughput
);
criterion_main!(benches);
