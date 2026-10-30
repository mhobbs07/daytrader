//! Benchmarks for pattern recognition and signal generation performance
//!
//! This module provides comprehensive benchmarks to measure and optimize
//! the performance of pattern detection, signal generation, and risk integration.

use chrono::{Duration, Utc};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use daytrader::brokers::types::{Confidence, PatternName, Price, Symbol};
use daytrader::events::types::{Candle, SignalType};
use daytrader::patterns::indicators;
use daytrader::patterns::signals::{MarketContextAnalyzer, SignalConfig, SignalEngine};
use daytrader::patterns::types::{PatternContext, PatternResult};
use rust_decimal::Decimal;
use tokio::runtime::Runtime;

/// Generate sample candle data for benchmarking
fn generate_sample_candles(count: usize) -> Vec<Candle> {
    let mut candles = Vec::with_capacity(count);
    let base_time = Utc::now();

    for i in 0..count {
        let time = base_time + Duration::minutes(i as i64);
        let price_base = 100.0 + (i as f64 * 0.1).sin() * 5.0;

        candles.push(Candle {
            timestamp: time,
            open: Price::new(Decimal::from_f64_retain(price_base).unwrap()).unwrap(),
            high: Price::new(Decimal::from_f64_retain(price_base + 1.0).unwrap()).unwrap(),
            low: Price::new(Decimal::from_f64_retain(price_base - 1.0).unwrap()).unwrap(),
            close: Price::new(Decimal::from_f64_retain(price_base + 0.5).unwrap()).unwrap(),
            volume: 1000 + (i % 100) as u64,
            symbol: Symbol::new("AAPL").unwrap(),
        });
    }

    candles
}

/// Generate sample pattern results for benchmarking
fn generate_sample_patterns(count: usize) -> Vec<PatternResult> {
    let mut patterns = Vec::with_capacity(count);
    let pattern_names = ["hammer", "doji", "bullish_engulfing", "morning_star"];

    for i in 0..count {
        let pattern_name = pattern_names[i % pattern_names.len()];
        let confidence = 0.5 + (i as f64 * 0.01).sin() * 0.4; // Vary confidence

        patterns.push(PatternResult {
            pattern_name: PatternName::new(pattern_name),
            confidence: Confidence::new(confidence.clamp(0.0, 1.0)).unwrap(),
            signal_type: if confidence > 0.5 {
                SignalType::Buy
            } else {
                SignalType::Sell
            },
            detected_at: Utc::now(),
            price_level: Price::new(Decimal::from(100)).unwrap(),
            context: PatternContext::default(),
        });
    }

    patterns
}

fn bench_signal_generation(c: &mut Criterion) {
    let config = SignalConfig::default();
    let mut engine = SignalEngine::new(config);
    let rt = Runtime::new().unwrap();

    let candles = generate_sample_candles(1000);
    let patterns = generate_sample_patterns(50);

    c.bench_function("signal_generation_50_patterns", |b| {
        b.iter(|| {
            let result = black_box(
                rt.block_on(engine.generate_signals(black_box(&patterns), black_box(&candles))),
            );
            result.unwrap()
        })
    });
}

fn bench_parallel_signal_generation(c: &mut Criterion) {
    let config = SignalConfig::default();
    let mut engine = SignalEngine::new(config);
    let rt = Runtime::new().unwrap();

    let candles = generate_sample_candles(1000);
    let patterns = generate_sample_patterns(200); // More patterns for parallel processing

    c.bench_function("parallel_signal_generation_200_patterns", |b| {
        b.iter(|| {
            let result = black_box(
                rt.block_on(engine.generate_signals(black_box(&patterns), black_box(&candles))),
            );
            result.unwrap()
        })
    });
}

fn bench_market_context_analysis(c: &mut Criterion) {
    let context_analyzer = MarketContextAnalyzer::with_config(SignalConfig::default());
    let candles = generate_sample_candles(500);

    c.bench_function("market_context_analysis", |b| {
        b.iter(|| black_box(context_analyzer.analyze_context(black_box(&candles))))
    });
}

fn bench_indicator_calculations(c: &mut Criterion) {
    let candles = generate_sample_candles(1000);
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close.value()).collect();

    c.bench_function("rsi_calculation", |b| {
        b.iter(|| black_box(indicators::rsi(black_box(&closes), black_box(14))))
    });

    c.bench_function("sma_calculation", |b| {
        b.iter(|| black_box(indicators::sma(black_box(&closes), black_box(20))))
    });

    c.bench_function("macd_calculation", |b| {
        b.iter(|| {
            black_box(indicators::macd(
                black_box(&closes),
                black_box(12),
                black_box(26),
                black_box(9),
            ))
        })
    });
}

fn bench_memory_usage(c: &mut Criterion) {
    c.bench_function("memory_allocation_patterns", |b| {
        b.iter(|| {
            let mut signals = Vec::new();
            for _ in 0..1000 {
                let signal = generate_sample_patterns(1).into_iter().next().unwrap();
                signals.push(signal);
            }
            black_box(signals)
        })
    });
}

fn bench_database_operations(c: &mut Criterion) {
    // Note: This would require a test database setup
    // For now, we'll benchmark the data transformation operations

    c.bench_function("pattern_stats_transformation", |b| {
        let patterns = generate_sample_patterns(100);

        b.iter(|| {
            let mut stats = std::collections::HashMap::new();
            for pattern in &patterns {
                let key = pattern.pattern_name.as_str().to_string();
                let entry = stats.entry(key).or_insert((0u32, 0.0));
                entry.0 += 1;
                entry.1 += pattern.confidence.value();
            }
            black_box(stats)
        })
    });
}

fn bench_risk_evaluation(c: &mut Criterion) {
    // This would benchmark risk evaluation performance
    // For now, we'll benchmark the confidence multiplier calculations

    c.bench_function("confidence_multiplier_calculation", |b| {
        let confidences = (0..1000).map(|i| i as f64 * 0.001).collect::<Vec<f64>>();

        b.iter(|| {
            let mut results = Vec::new();
            for &conf in &confidences {
                let multiplier = if conf >= 0.8 {
                    1.0
                } else if conf >= 0.6 {
                    0.8
                } else if conf >= 0.4 {
                    0.6
                } else {
                    0.4
                };
                results.push(multiplier);
            }
            black_box(results)
        })
    });
}

criterion_group!(
    benches,
    bench_signal_generation,
    bench_parallel_signal_generation,
    bench_market_context_analysis,
    bench_indicator_calculations,
    bench_memory_usage,
    bench_database_operations,
    bench_risk_evaluation
);

criterion_main!(benches);
