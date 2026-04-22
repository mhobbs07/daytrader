//! Performance benchmarks for risk management system
//!
//! These benchmarks establish baseline metrics for:
//! - Rule evaluation speed
//! - Concurrent load performance
//! - Memory usage patterns
//! - Event processing throughput

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use daytrader::brokers::types::{OrderId, Quantity, Symbol};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{MarketEvent, Order, OrderSide, OrderStatus, OrderType};
use daytrader::risk::limits::DailyLossTracker;
use daytrader::risk::pdt::PdtEnforcer;
use daytrader::risk::rules::RiskRuleEngine;
use daytrader::risk::rules::TradeContext;
use daytrader::risk::types::AccountId;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark individual risk rule evaluation performance
pub fn bench_rule_evaluation_speed(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let rule_engine = RiskRuleEngine::new(config);

    let context = TradeContext::new(
        "AAPL".to_string(),
        Decimal::new(1000, 0),
        Decimal::new(15000, 2),
        Decimal::new(50000, 2),
        false,
    );

    c.bench_function("rule_evaluation_speed", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(
                    rule_engine
                        .evaluate_violations_only(black_box(context.clone()))
                        .await,
                );
            });
        });
    });
}

/// Benchmark concurrent risk evaluations under high load
fn bench_concurrent_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let rule_engine = Arc::new(RiskRuleEngine::new(config));

    c.bench_function("concurrent_risk_evaluations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut handles = vec![];

                // Spawn 100 concurrent evaluations
                for i in 0..100 {
                    let engine = Arc::clone(&rule_engine);
                    let context = TradeContext::new(
                        format!("SYMBOL{}", i),
                        Decimal::new(1000 + i as i64, 0),
                        Decimal::new(15000 + i as i64 * 100, 2),
                        Decimal::new(50000, 2),
                        false,
                    );

                    let handle = tokio::spawn(async move {
                        let _ = engine.evaluate_violations_only(context).await;
                    });
                    handles.push(handle);
                }

                // Wait for all evaluations to complete
                for handle in handles {
                    let _ = handle.await;
                }
            });
        });
    });
}

/// Benchmark memory usage patterns in risk evaluation
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();

    c.bench_function("memory_usage_patterns", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create multiple rule engines to test memory allocation
                let mut engines = vec![];
                for _ in 0..10 {
                    engines.push(RiskRuleEngine::new(config.clone()));
                }

                // Perform evaluations
                for engine in &engines {
                    let context = TradeContext::new(
                        "AAPL".to_string(),
                        Decimal::new(1000, 0),
                        Decimal::new(15000, 2),
                        Decimal::new(50000, 2),
                        false,
                    );
                    let _ = engine.evaluate_violations_only(context).await;
                }

                black_box(engines);
            });
        });
    });
}

/// Benchmark event processing throughput
fn bench_event_processing_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let event_bus = Arc::new(EventBus::new(10000));
    let config = create_benchmark_risk_config();
    let account_id = AccountId::new("benchmark-account");
    let initial_equity = Decimal::new(10000000, 2);

    // Create risk engine
    let engine = daytrader::risk::RiskEngine::new(
        config.clone(),
        Arc::clone(&event_bus),
        account_id.clone(),
        initial_equity,
    );

    c.bench_function("event_processing_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create fresh engine for each iteration
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    initial_equity,
                );
                let bus = Arc::clone(&event_bus);

                // Spawn event processing task
                let process_handle = tokio::spawn(async move {
                    engine.start().await.unwrap();
                });

                // Publish 1000 events rapidly
                for i in 0..1000 {
                    let order = create_benchmark_order(i);
                    bus.publish(MarketEvent::OrderPlaced {
                        order: Arc::clone(&order),
                    });
                }

                // Wait a bit for processing
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                process_handle.abort();
            });
        });
    });
}

/// Benchmark PDT rule evaluation performance
fn bench_pdt_evaluation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let enforcer = PdtEnforcer::new(
        daytrader::risk::pdt::PdtConfig::from_config(&config),
        Decimal::new(5000000, 2),
    );

    c.bench_function("pdt_rule_evaluation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(enforcer.enforce_pdt_limits().await);
            });
        });
    });
}

/// Benchmark daily loss limit checking
fn bench_loss_limit_checking(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let tracker = DailyLossTracker::from_config(&config, Decimal::new(10000000, 2));

    c.bench_function("loss_limit_checking", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(tracker.check_daily_loss_limit(Decimal::new(-1000, 2)).await);
            });
        });
    });
}

/// Benchmark position sizing calculations
fn bench_position_sizing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let sizer = daytrader::risk::position_sizing::PositionSizer::from_config(
        &config,
        Decimal::new(10000000, 2),
    );

    c.bench_function("position_sizing_calculations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = black_box(sizer.calculate_max_position_size().await);
            });
        });
    });
}

/// Benchmark full risk workflow (event bus → evaluation → response)
fn bench_full_risk_workflow(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let event_bus = Arc::new(EventBus::new(1000));
    let config = create_benchmark_risk_config();
    let account_id = AccountId::new("workflow-benchmark");
    let initial_equity = Decimal::new(10000000, 2);

    c.bench_function("full_risk_workflow", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    initial_equity,
                );

                // Subscribe to violations
                let mut receiver = event_bus.subscribe();

                // Start engine
                let bus_clone = Arc::clone(&event_bus);

                let workflow_handle = tokio::spawn(async move {
                    engine.start().await.unwrap();
                });

                // Send order and wait for processing
                let order = create_benchmark_order(0);
                bus_clone.publish(MarketEvent::OrderPlaced {
                    order: Arc::clone(&order),
                });

                // Wait for response
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Check for violations
                let mut violations = 0;
                while let Ok(MarketEvent::RiskViolation { .. }) = receiver.try_recv() {
                    violations += 1;
                }

                workflow_handle.abort();
                black_box(violations);
            });
        });
    });
}

/// Benchmark memory allocation patterns under sustained load
fn bench_sustained_load_memory(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();

    c.bench_function("sustained_load_memory", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut contexts = vec![];

                // Create many trade contexts
                for i in 0..1000 {
                    contexts.push(TradeContext::new(
                        format!("SYMBOL{}", i),
                        Decimal::new(1000 + i as i64, 0),
                        Decimal::new(15000 + i as i64, 2),
                        Decimal::new(50000, 2),
                        i % 10 == 0, // Some are day trades
                    ));
                }

                // Create rule engine and evaluate all contexts
                let rule_engine = RiskRuleEngine::new(config.clone());
                for context in contexts {
                    let _ = rule_engine.evaluate_violations_only(context).await;
                }
            });
        });
    });
}

/// Benchmark risk evaluation timing under database contention
fn bench_database_contention(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();

    c.bench_function("database_contention", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Simulate database operations that could cause contention
                let mut tasks = vec![];

                for i in 0..50 {
                    let config_clone = config.clone();
                    let task = tokio::spawn(async move {
                        // Simulate database read/write operations
                        let rule_engine = RiskRuleEngine::new(config_clone);
                        let context = TradeContext::new(
                            format!("DB_SYMBOL{}", i),
                            Decimal::new(1000, 0),
                            Decimal::new(15000, 2),
                            Decimal::new(50000, 2),
                            false,
                        );

                        // Multiple evaluations to simulate contention
                        for _ in 0..10 {
                            let _ = rule_engine.evaluate_violations_only(context.clone()).await;
                        }
                    });
                    tasks.push(task);
                }

                // Wait for all database operations
                for task in tasks {
                    let _ = task.await;
                }
            });
        });
    });
}

/// Benchmark risk engine startup and initialization time
fn bench_risk_engine_startup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("risk_engine_startup", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = create_benchmark_risk_config();
                let event_bus = Arc::new(EventBus::new(1000));
                let account_id = AccountId::new("startup-test");

                // Measure engine creation and startup time
                let engine = daytrader::risk::RiskEngine::new(
                    config,
                    Arc::clone(&event_bus),
                    account_id,
                    Decimal::new(10000000, 2),
                );

                let _ = black_box(engine.start().await);
            });
        });
    });
}

/// Benchmark position tracking updates under high frequency
fn bench_position_tracking_high_frequency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let event_bus = Arc::new(EventBus::new(10000));
    let account_id = AccountId::new("position-tracking-bench");

    c.bench_function("position_tracking_high_frequency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    Decimal::new(10000000, 2),
                );
                engine.start().await.unwrap();

                // Simulate high-frequency position updates
                for i in 0..1000 {
                    let position = daytrader::events::types::Position {
                        symbol: Symbol::new("BENCH").unwrap(),
                        quantity: Quantity::new(1000).unwrap(),
                        average_entry_price: Price::new(Decimal::new(15000, 2)).unwrap(),
                        current_price: Price::new(Decimal::new(15000 + i as i64, 2)).unwrap(),
                        unrealized_pl: Decimal::new(i as i64 * 100, 2),
                        realized_pl: Decimal::ZERO,
                        opened_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };

                    event_bus.publish(MarketEvent::PositionUpdated { position });
                }

                // Allow processing time
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            });
        });
    });
}

/// Benchmark PDT rule evaluation with large day trade histories
fn bench_pdt_large_history(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("pdt_large_history", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = create_benchmark_risk_config();
                let enforcer = PdtEnforcer::new(
                    daytrader::risk::pdt::PdtConfig::from_config(&config),
                    Decimal::new(5000000, 2),
                );

                // Simulate large day trade history
                let base_date = chrono::Utc::now().date_naive();
                for i in 0..100 {
                    let trade_date = base_date - chrono::Duration::days(i);
                    for _ in 0..3 {
                        // Multiple trades per day
                        enforcer.record_day_trade(trade_date).await.unwrap();
                    }
                }

                // Benchmark counting trades in window
                let _ = black_box(enforcer.count_day_trades().await);
            });
        });
    });
}

/// Benchmark risk evaluation with complex position portfolios
fn bench_complex_portfolio_risk(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("complex-portfolio-bench");

    c.bench_function("complex_portfolio_risk", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    Decimal::new(10000000, 2),
                );
                engine.start().await.unwrap();

                // Create complex portfolio with many positions
                let symbols = [
                    "AAPL", "GOOGL", "MSFT", "TSLA", "NVDA", "AMZN", "META", "NFLX",
                ];
                for (i, symbol) in symbols.iter().enumerate() {
                    let position = daytrader::events::types::Position {
                        symbol: Symbol::new(symbol).unwrap(),
                        quantity: Quantity::new(100 * (i + 1) as u32).unwrap(),
                        average_entry_price: Price::new(Decimal::new(10000 + i as i64 * 1000, 2))
                            .unwrap(),
                        current_price: Price::new(Decimal::new(10500 + i as i64 * 1000, 2))
                            .unwrap(),
                        unrealized_pl: Decimal::new(500 * (i + 1) as i64, 2),
                        realized_pl: Decimal::ZERO,
                        opened_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };

                    event_bus.publish(MarketEvent::PositionUpdated { position });
                }

                // Allow risk evaluation processing
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

                // Check risk state
                let _ = black_box(engine.get_risk_state().await);
            });
        });
    });
}

/// Benchmark emergency liquidation decision making
fn bench_emergency_liquidation_decisions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_emergency_benchmark_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("emergency-bench");

    c.bench_function("emergency_liquidation_decisions", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    Decimal::new(10000000, 2),
                );
                engine.start().await.unwrap();

                // Simulate emergency loss scenario
                let emergency_fill = Fill {
                    order_id: OrderId::new("emergency-fill"),
                    symbol: Symbol::new("CRASH").unwrap(),
                    side: OrderSide::Sell,
                    filled_quantity: Quantity::new(1000).unwrap(),
                    filled_price: Price::new(Decimal::new(5000, 2)).unwrap(),
                    filled_at: chrono::Utc::now(),
                    commission: Decimal::new(-500000, 2), // $5,000 loss
                };

                event_bus.publish(MarketEvent::OrderFilled {
                    order_id: emergency_fill.order_id.clone(),
                    fill: emergency_fill,
                });

                // Allow emergency processing
                tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

                // Check if emergency actions were triggered
                let _ = black_box(engine.is_trading_allowed().await);
            });
        });
    });
}

/// Benchmark pattern-specific risk evaluation
fn bench_pattern_risk_evaluation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_benchmark_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("pattern-risk-bench");

    c.bench_function("pattern_risk_evaluation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = daytrader::risk::RiskEngine::new(
                    config.clone(),
                    Arc::clone(&event_bus),
                    account_id.clone(),
                    Decimal::new(10000000, 2),
                );
                engine.start().await.unwrap();

                // Simulate multiple pattern detections
                let patterns = ["hammer", "doji", "engulfing", "marubozu", "harami"];
                for (i, pattern) in patterns.iter().enumerate() {
                    let signal = daytrader::events::types::PatternSignal {
                        symbol: Symbol::new(&format!("PATTERN{}", i)).unwrap(),
                        pattern_name: daytrader::brokers::types::PatternName::new(pattern),
                        signal_type: daytrader::events::types::SignalType::Buy,
                        confidence: Confidence::new(0.8).unwrap(),
                        detected_at: chrono::Utc::now(),
                        price: Price::new(Decimal::new(15000 + i as i64 * 100, 2)).unwrap(),
                    };

                    event_bus.publish(MarketEvent::PatternDetected { signal });
                }

                // Allow pattern risk processing
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            });
        });
    });
}

/// Helper function to create benchmark risk configuration
fn create_benchmark_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 1000.0,
        max_daily_loss_percent: 2.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 10000.0,
        max_position_size_percent: 10.0,
        max_concurrent_positions: 10,
        enforce_pdt_rule: true,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 3,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 60.0,
        min_pattern_sample_size: 20,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 500.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds:
            daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}

/// Helper function to create emergency benchmark configuration
fn create_emergency_benchmark_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 2000.0,
        max_daily_loss_percent: 5.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 25000.0,
        max_position_size_percent: 25.0,
        max_concurrent_positions: 10,
        enforce_pdt_rule: false,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 10,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 50.0,
        min_pattern_sample_size: 10,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 1000.0, // Lower threshold for emergency testing
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds:
            daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}

/// Helper function to create benchmark orders
fn create_benchmark_order(id: u32) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(&format!("bench-order-{}", id)),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(100).unwrap(),
        price: None,
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

criterion_group!(
    benches,
    bench_rule_evaluation_speed,
    bench_concurrent_load,
    bench_memory_usage,
    bench_event_processing_throughput,
    bench_pdt_evaluation,
    bench_loss_limit_checking,
    bench_position_sizing,
    bench_full_risk_workflow,
    bench_sustained_load_memory,
    bench_database_contention,
    bench_risk_engine_startup,
    bench_position_tracking_high_frequency,
    bench_pdt_large_history,
    bench_complex_portfolio_risk,
    bench_emergency_liquidation_decisions,
    bench_pattern_risk_evaluation
);
criterion_main!(benches);
