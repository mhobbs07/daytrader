//! Comprehensive Phase 3D performance benchmarks
//!
//! This benchmark suite validates all Phase 3D performance targets:
//! - Pattern detection: < 100μs per symbol
//! - Signal generation: < 100μs per symbol
//! - Full pipeline (pattern + indicators + signal): < 500μs per symbol
//! - Support 50+ symbols in real-time
//! - Memory usage: < 200MB for 50 symbols

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use daytrader::brokers::types::{Confidence, PatternName, Price, Symbol};
use daytrader::events::types::{Candle, SignalType};
use daytrader::patterns::{
    candlestick::{DojiDetector, HammerDetector, ShootingStarDetector},
    engulfing::{BearishEngulfingDetector, BullishEngulfingDetector},
    indicators,
    registry::PatternRegistry,
    signals::{SignalConfig, SignalEngine},
    traits::PatternDetector,
    types::{PatternParams, PatternResult},
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::runtime::Runtime;

// =============================================================================
// Test Data Generators
// =============================================================================

/// Creates a single candle with specific characteristics
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

/// Creates realistic candle data for a single symbol
fn create_single_symbol_data(count: usize) -> Vec<Candle> {
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

/// Creates candle data for multiple symbols
fn create_multi_symbol_data(num_symbols: usize, candles_per_symbol: usize) -> Vec<(Symbol, Vec<Candle>)> {
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

/// Creates pattern results from candles (simulates pattern detection output)
fn create_pattern_results(candles: &[Candle]) -> Vec<PatternResult> {
    let mut results = Vec::new();
    
    // Mix of different patterns
    for (i, candle) in candles.iter().enumerate() {
        if i % 5 == 0 {
            let confidence = Confidence::new(0.7 + (i as f64 * 0.01).sin().abs() * 0.2).unwrap();
            results.push(PatternResult::new(
                PatternName::new("hammer"),
                SignalType::Buy,
                confidence,
                candle.close,
            ));
        } else if i % 7 == 0 {
            let confidence = Confidence::new(0.65 + (i as f64 * 0.02).cos().abs() * 0.25).unwrap();
            results.push(PatternResult::new(
                PatternName::new("shooting_star"),
                SignalType::Sell,
                confidence,
                candle.close,
            ));
        }
    }
    
    results
}

// =============================================================================
// 1. Pattern Detection Performance (Target: < 100μs per symbol)
// =============================================================================

/// Benchmark pattern detection for a single symbol
pub fn bench_pattern_detection_per_symbol(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.0);
    
    let mut group = c.benchmark_group("pattern_detection_per_symbol");
    group.throughput(Throughput::Elements(1)); // 1 symbol per iteration
    
    // Benchmark with typical lookback period (100 candles)
    let candles = create_single_symbol_data(100);
    
    group.bench_function("pattern_detection_100_candles", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(registry.detect_all(black_box(&candles), Some(&params)).await);
            });
        });
    });
    
    group.finish();
}

/// Benchmark individual pattern detectors
pub fn bench_individual_patterns(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let params = PatternParams::new().with_min_confidence(0.0);
    
    let mut group = c.benchmark_group("individual_patterns");
    
    // Hammer pattern
    let hammer_detector = HammerDetector::new();
    let hammer_candles = vec![create_candle(100.0, 105.0, 95.0, 103.0, 1000)];
    
    group.bench_function("hammer_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(hammer_detector.detect(black_box(&hammer_candles), black_box(&params)).await);
            });
        });
    });
    
    // Shooting Star pattern
    let shooting_star_detector = ShootingStarDetector::new();
    let shooting_star_candles = vec![create_candle(100.0, 115.0, 98.0, 102.0, 1000)];
    
    group.bench_function("shooting_star_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(shooting_star_detector.detect(black_box(&shooting_star_candles), black_box(&params)).await);
            });
        });
    });
    
    // Bullish Engulfing pattern
    let bullish_engulfing_detector = BullishEngulfingDetector::new();
    let engulfing_candles = vec![
        create_candle(103.0, 108.0, 101.0, 107.0, 1200),
        create_candle(105.0, 106.0, 102.0, 103.0, 1000),
    ];
    
    group.bench_function("bullish_engulfing_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(bullish_engulfing_detector.detect(black_box(&engulfing_candles), black_box(&params)).await);
            });
        });
    });
    
    group.finish();
}

// =============================================================================
// 2. Indicator Calculation Performance
// =============================================================================

/// Benchmark technical indicator calculations
pub fn bench_indicator_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("indicator_calculations");
    
    let candles = create_single_symbol_data(100);
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close.value()).collect();
    
    // SMA calculation
    group.bench_function("sma_20_period", |b| {
        b.iter(|| {
            let _ = black_box(indicators::sma(black_box(&closes), black_box(20)));
        });
    });
    
    // EMA calculation
    group.bench_function("ema_20_period", |b| {
        b.iter(|| {
            let _ = black_box(indicators::ema(black_box(&closes), black_box(20)));
        });
    });
    
    // RSI calculation
    group.bench_function("rsi_14_period", |b| {
        b.iter(|| {
            let _ = black_box(indicators::rsi(black_box(&closes), black_box(14)));
        });
    });
    
    // MACD calculation
    group.bench_function("macd_standard", |b| {
        b.iter(|| {
            let _ = black_box(indicators::macd(black_box(&closes), black_box(12), black_box(26), black_box(9)));
        });
    });
    
    // Bollinger Bands
    group.bench_function("bollinger_bands_20_period", |b| {
        b.iter(|| {
            let _ = black_box(indicators::bollinger_bands(black_box(&closes), black_box(20)));
        });
    });
    
    // ATR calculation
    group.bench_function("atr_14_period", |b| {
        b.iter(|| {
            let _ = black_box(indicators::atr(black_box(&candles), black_box(14)));
        });
    });
    
    group.finish();
}

// =============================================================================
// 3. Signal Generation Performance (Target: < 100μs per symbol)
// =============================================================================

/// Benchmark signal generation from patterns
pub fn bench_signal_generation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = SignalConfig::default();
    
    let mut group = c.benchmark_group("signal_generation");
    group.throughput(Throughput::Elements(1)); // 1 symbol per iteration
    
    let candles = create_single_symbol_data(100);
    let pattern_results = create_pattern_results(&candles);
    
    group.bench_function("signal_from_patterns", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut engine = SignalEngine::new(config.clone());
                let _ = black_box(engine.generate_signals(black_box(&pattern_results), black_box(&candles)).await);
            });
        });
    });
    
    group.finish();
}

// =============================================================================
// 4. Full Pipeline Performance (Target: < 500μs per symbol)
// =============================================================================

/// Benchmark the complete pipeline: candle → patterns → indicators → signals
pub fn bench_full_pipeline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let config = SignalConfig::default();
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("full_pipeline");
    group.throughput(Throughput::Elements(1)); // 1 symbol per iteration
    
    let candles = create_single_symbol_data(100);
    
    group.bench_function("pattern_to_signal_pipeline", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Step 1: Pattern detection
                let pattern_results = registry.detect_all(black_box(&candles), Some(&params)).await;
                
                // Step 2: Generate signals (includes indicator calculation)
                let mut engine = SignalEngine::new(config.clone());
                let _ = black_box(engine.generate_signals(&pattern_results, black_box(&candles)).await);
            });
        });
    });
    
    group.finish();
}

/// Benchmark full pipeline with varying numbers of symbols
pub fn bench_full_pipeline_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let config = SignalConfig::default();
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("full_pipeline_scaling");
    
    for num_symbols in [1, 10, 50].iter() {
        let multi_symbol_data = create_multi_symbol_data(*num_symbols, 100);
        
        group.throughput(Throughput::Elements(*num_symbols as u64));
        group.bench_with_input(BenchmarkId::from_parameter(num_symbols), num_symbols, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    for (_symbol, candles) in &multi_symbol_data {
                        // Pattern detection
                        let pattern_results = registry.detect_all(black_box(candles), Some(&params)).await;
                        
                        // Signal generation
                        let mut engine = SignalEngine::new(config.clone());
                        let _ = black_box(engine.generate_signals(&pattern_results, black_box(candles)).await);
                    }
                });
            });
        });
    }
    
    group.finish();
}

// =============================================================================
// 5. Parallel vs Sequential Processing
// =============================================================================

/// Benchmark sequential pattern processing
pub fn bench_sequential_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("sequential_processing");
    
    let multi_symbol_data = create_multi_symbol_data(50, 100);
    
    group.throughput(Throughput::Elements(50));
    group.bench_function("50_symbols_sequential", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut all_results = Vec::new();
                for (_symbol, candles) in &multi_symbol_data {
                    let results = registry.detect_all(black_box(candles), Some(&params)).await;
                    all_results.push(results);
                }
                black_box(all_results)
            });
        });
    });
    
    group.finish();
}

/// Benchmark parallel pattern processing
pub fn bench_parallel_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("parallel_processing");
    
    let multi_symbol_data = create_multi_symbol_data(50, 100);
    
    group.throughput(Throughput::Elements(50));
    group.bench_function("50_symbols_parallel", |b| {
        b.iter(|| {
            rt.block_on(async {
                use futures::stream::{self, StreamExt};
                
                let results: Vec<_> = stream::iter(&multi_symbol_data)
                    .map(|(_symbol, candles)| {
                        let registry = registry.clone();
                        let params = params.clone();
                        async move {
                            registry.detect_all(candles, Some(&params)).await
                        }
                    })
                    .buffer_unordered(10) // 10 concurrent tasks
                    .collect()
                    .await;
                
                black_box(results)
            });
        });
    });
    
    group.finish();
}

// =============================================================================
// 6. Memory Allocation Benchmarks
// =============================================================================

/// Benchmark memory allocations in pattern detection
pub fn bench_memory_allocations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("memory_allocations");
    
    // Benchmark with pre-allocated vs dynamic allocation
    let candles = create_single_symbol_data(100);
    
    group.bench_function("with_preallocation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut results = Vec::with_capacity(10); // Pre-allocate
                let detected = registry.detect_all(black_box(&candles), Some(&params)).await;
                results.extend(detected);
                black_box(results)
            });
        });
    });
    
    group.bench_function("without_preallocation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut results = Vec::new(); // No pre-allocation
                let detected = registry.detect_all(black_box(&candles), Some(&params)).await;
                results.extend(detected);
                black_box(results)
            });
        });
    });
    
    group.finish();
}

/// Benchmark memory usage with signal caching
pub fn bench_signal_cache_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = SignalConfig::default();
    
    let mut group = c.benchmark_group("signal_cache");
    
    let candles = create_single_symbol_data(100);
    let pattern_results = create_pattern_results(&candles);
    
    // Benchmark signal generation with cache
    group.bench_function("with_cache", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut engine = SignalEngine::new(config.clone());
                
                // Generate multiple times to test cache hit
                for _ in 0..5 {
                    let _ = black_box(engine.generate_signals(&pattern_results, &candles).await);
                }
            });
        });
    });
    
    group.finish();
}

// =============================================================================
// 7. Latency Distribution Benchmarks
// =============================================================================

/// Benchmark latency distribution for pattern detection
pub fn bench_pattern_detection_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = Arc::new(PatternRegistry::new());
    let params = PatternParams::new().with_min_confidence(0.6);
    
    let mut group = c.benchmark_group("pattern_latency");
    group.sample_size(1000); // Larger sample for accurate percentiles
    
    let candles = create_single_symbol_data(100);
    
    group.bench_function("latency_distribution", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(registry.detect_all(black_box(&candles), Some(&params)).await);
            });
        });
    });
    
    group.finish();
}

// =============================================================================
// Criterion Configuration
// =============================================================================

criterion_group!(
    benches,
    bench_pattern_detection_per_symbol,
    bench_individual_patterns,
    bench_indicator_calculations,
    bench_signal_generation,
    bench_full_pipeline,
    bench_full_pipeline_scaling,
    bench_sequential_processing,
    bench_parallel_processing,
    bench_memory_allocations,
    bench_signal_cache_performance,
    bench_pattern_detection_latency,
);

criterion_main!(benches);
