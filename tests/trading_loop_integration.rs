//! End-to-end trading loop integration
//!
//! Exercises the wiring the Tauri app relies on: order placement through the
//! risk-gated engine, broker fill polling (`sync_order_statuses`), fill
//! propagation via `OrderFilled` events into the risk engine's day-trade
//! tracker and the positions module's P&L tracker, and `SignalGenerated`
//! consumption by the execution engine.
//!
//! The signal->order automation leg is gated by a wall-clock market-hours
//! rule inside the strategy, so these tests drive the order engine with a
//! fixed in-session clock and verify the execution engine's signal intake
//! via its metrics instead of asserting on time-dependent auto-execution.

use chrono::{TimeZone, Utc};
use daytrader::brokers::mock::{MockBroker, MockConfig};
use daytrader::brokers::traits::{OrderExecutionProvider, OrderSide, OrderType};
use daytrader::brokers::types::{Confidence, PatternName, Price, Quantity, Symbol};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{MarketEvent, SignalType};
use daytrader::execution::engine::{ExecutionConfig, ExecutionEngine};
use daytrader::execution::strategy::{ConservativeConfig, ConservativeStrategy};
use daytrader::orders::broker_adapter::AlpacaAdapter;
use daytrader::orders::engine::OrderEngine;
use daytrader::orders::types::{FillInfo, OrderConfig, OrderRequest};
use daytrader::patterns::signals::{
    MarketContext, SignalComponent, SignalComponentType, TradingSignal,
};
use daytrader::patterns::types::TrendContext;
use daytrader::positions::tracker::PositionConfig;
use daytrader::positions::{PnLConfig, PositionTracker};
use daytrader::risk::RiskEngine;
use daytrader::risk::types::AccountId;
use daytrader::utils::clock::FixedClock;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

fn test_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 500.0,
        max_daily_loss_percent: 2.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 20000.0,
        max_position_size_percent: 25.0,
        max_concurrent_positions: 5,
        enforce_pdt_rule: true,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 3,
        max_trades_per_pattern_per_day: 5,
        stop_pattern_after_consecutive_losses: 3,
        required_win_rate_for_auto: 60.0,
        min_pattern_sample_size: 20,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 500.0,
        pattern_profiles: HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: Default::default(),
    }
}

/// Wednesday 2024-06-12 15:00 UTC == 11:00 EDT — inside market hours
fn open_market_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2024, 6, 12, 15, 0, 0).unwrap(),
    ))
}

struct TestHarness {
    event_bus: Arc<EventBus>,
    risk_engine: Arc<RiskEngine>,
    order_engine: Arc<OrderEngine>,
    execution_engine: Arc<ExecutionEngine>,
    pnl_tracker: Arc<PositionTracker>,
}

async fn build_harness() -> TestHarness {
    let event_bus = Arc::new(EventBus::new(1000));

    let broker = Arc::new(MockBroker::new(MockConfig {
        auto_fill_orders: true,
        ..MockConfig::default()
    }));

    let risk_engine = Arc::new(RiskEngine::new(
        test_risk_config(),
        Arc::clone(&event_bus),
        AccountId::new("test-account"),
        Decimal::new(10000000, 2), // $100,000 matches MockBroker default account
    ));

    let db_pool = Arc::new(sqlx::SqlitePool::connect(":memory:").await.unwrap());
    sqlx::query(include_str!("../src/db/schema.sql"))
        .execute(&*db_pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO accounts (broker, account_number, account_type) \
         VALUES ('mock', 'test-account', 'paper')",
    )
    .execute(&*db_pool)
    .await
    .unwrap();

    let order_engine = Arc::new(
        OrderEngine::new(
            broker.clone() as Arc<dyn OrderExecutionProvider>,
            Arc::clone(&risk_engine),
            Arc::new(AlpacaAdapter::new(
                broker.clone() as Arc<dyn OrderExecutionProvider>
            )),
            Arc::clone(&event_bus),
            Arc::clone(&db_pool),
            1,
            OrderConfig::default(),
        )
        .with_clock(open_market_clock()),
    );

    let pnl_tracker = Arc::new(PositionTracker::new(
        broker.clone() as Arc<dyn OrderExecutionProvider>,
        (*event_bus).clone(),
        Arc::clone(&db_pool),
        PnLConfig::default(),
        PositionConfig::default(),
    ));

    let execution_engine = Arc::new(
        ExecutionEngine::new(
            Arc::clone(&order_engine),
            Arc::clone(&pnl_tracker),
            Arc::clone(&risk_engine),
            Arc::clone(&event_bus),
            Arc::clone(&db_pool),
            Arc::new(ConservativeStrategy::new(ConservativeConfig {
                use_market_orders: false,
                ..ConservativeConfig::default()
            })),
            ExecutionConfig {
                auto_execution_enabled: false,
                ..ExecutionConfig::default()
            },
        )
        .await
        .unwrap(),
    );

    TestHarness {
        event_bus,
        risk_engine,
        order_engine,
        execution_engine,
        pnl_tracker,
    }
}

fn buy_signal(symbol: &Symbol, confidence: f64, price: Decimal) -> TradingSignal {
    TradingSignal::new(
        symbol.clone(),
        SignalType::Buy,
        Confidence::new(confidence).unwrap(),
        Price::new(price).unwrap(),
        MarketContext::new(TrendContext::StrongUptrend { strength: 0.8 }),
    )
    .add_component(SignalComponent::new(
        SignalComponentType::Pattern,
        "hammer",
        confidence,
        0.4,
        1.0,
    ))
}

/// Polls `probe` every 20 ms until it returns true or ~2 s elapse.
async fn wait_for(mut probe: impl AsyncFnMut() -> bool) -> bool {
    for _ in 0..100 {
        if probe().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn test_order_fill_reaches_both_position_trackers() {
    let harness = build_harness().await;
    let symbol = Symbol::new("AAPL").unwrap();

    // Risk engine consumes OrderPlaced/OrderFilled from the bus
    harness.risk_engine.start().await.unwrap();

    let mut fill_rx = harness.event_bus.subscribe();

    // Place a risk-gated limit order carrying pattern attribution
    let limit = Price::new(Decimal::new(15000, 2)).unwrap();
    let request = OrderRequest::builder()
        .symbol(symbol.clone())
        .side(OrderSide::Buy)
        .order_type(OrderType::Limit)
        .quantity(Quantity::new(10).unwrap())
        .limit_price(limit)
        .pattern_name("hammer")
        .build()
        .unwrap();
    let order_id = harness.order_engine.place_order(request).await.unwrap();

    // The status poller surfaces the mock broker's fill
    let updated = harness.order_engine.sync_order_statuses().await.unwrap();
    assert_eq!(updated, 1);

    let order = harness.order_engine.get_order(&order_id).await.unwrap();
    assert_eq!(
        order.status,
        daytrader::brokers::traits::OrderStatus::Filled
    );
    assert_eq!(order.filled_avg_price, Some(limit));

    // The OrderFilled event is on the bus (what the app's fill listener and
    // the risk engine consume)
    let mut fill = None;
    while let Ok(event) = fill_rx.try_recv() {
        if let MarketEvent::OrderFilled { fill: f, .. } = event {
            fill = Some(f);
        }
    }
    let fill = fill.expect("OrderFilled event must be published");
    assert_eq!(fill.filled_price, limit);

    // Risk engine's FIFO day-trade tracker picks the fill up asynchronously
    let risk_engine = Arc::clone(&harness.risk_engine);
    let risk_symbol = symbol.clone();
    let seen = wait_for(async || {
        risk_engine
            .position_tracker()
            .get_position(&risk_symbol)
            .await
            .is_some()
    })
    .await;
    assert!(seen, "fill must reach the risk engine's position tracker");

    // P&L tracker is fed the same way the app's fill listener feeds it
    let fill_info = FillInfo {
        filled_quantity: fill.filled_quantity,
        filled_price: fill.filled_price,
        commission: fill.commission,
        timestamp: fill.filled_at,
    };
    harness
        .pnl_tracker
        .update_from_fill(&order, &fill_info)
        .await
        .unwrap();

    let positions = harness.pnl_tracker.get_all_positions().await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol, symbol);
    assert_eq!(positions[0].quantity.value(), 10);
    assert_eq!(positions[0].avg_entry_price, limit);
    assert_eq!(
        positions[0].pattern_name,
        Some(PatternName::new("hammer")),
        "pattern attribution must survive the order lifecycle"
    );
}

#[tokio::test]
async fn test_execution_engine_consumes_signals_and_respects_kill_switch() {
    let harness = build_harness().await;
    let symbol = Symbol::new("MSFT").unwrap();

    harness.execution_engine.start().await.unwrap();
    assert!(!harness.execution_engine.is_automation_enabled());

    // With automation disabled, a high-confidence signal is routed to manual
    // review — consumed, counted, and no order is placed
    harness.event_bus.publish(MarketEvent::SignalGenerated {
        signal: buy_signal(&symbol, 0.9, Decimal::new(30000, 2)),
    });

    let engine = Arc::clone(&harness.execution_engine);
    let reviewed = wait_for(async || engine.get_metrics().manual_review == 1).await;
    assert!(
        reviewed,
        "signal must be consumed and counted as manual review"
    );
    assert!(harness.order_engine.get_active_orders().is_empty());
    assert!(harness.order_engine.get_archived_orders().await.is_empty());

    // The kill switch flips at runtime (what the UI toggle calls)
    harness.execution_engine.enable_automation();
    assert!(harness.execution_engine.is_automation_enabled());
    harness.execution_engine.disable_automation();
    assert!(!harness.execution_engine.is_automation_enabled());
}
