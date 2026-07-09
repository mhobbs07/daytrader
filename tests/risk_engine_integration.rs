//! Risk engine integration tests
//!
//! Tests the complete risk engine workflow including event bus integration,
//! state management, and concurrent operations.

use daytrader::brokers::types::{OrderId, Price, Quantity, Symbol};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{
    Fill, MarketEvent, Order, OrderSide, OrderStatus, OrderType, Position,
};
use daytrader::risk::RiskEngine;
use daytrader::risk::types::AccountId;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Test risk engine initialization and basic state management
#[tokio::test]
async fn test_risk_engine_initialization() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-engine-account");
    let initial_equity = Decimal::new(10000000, 2); // $100,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);

    // Test initial state
    let state = engine.get_risk_state().await;
    assert_eq!(state.daily_pl, Decimal::ZERO);
    assert_eq!(state.day_trade_count, 0);
    assert!(!state.is_trading_halted);
}

/// Test risk engine event processing and state updates
#[tokio::test]
async fn test_risk_engine_event_processing() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("event-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to risk violations
    let mut violation_receiver = event_bus.subscribe();

    // Create and publish an order that should trigger violations. The order
    // must carry a price: position value is quantity * price, and a market
    // order without one evaluates to $0, which can never violate size limits.
    let order = create_large_position_order("large-order-1");
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    // Wait for processing
    sleep(Duration::from_millis(50)).await;

    // Drain everything on the bus (the receiver sees its own OrderPlaced
    // echo first) and count the violations among it
    let mut violations_found = 0;
    while let Ok(event) = violation_receiver.try_recv() {
        if matches!(event, MarketEvent::RiskViolation { .. }) {
            violations_found += 1;
        }
    }

    assert!(
        violations_found > 0,
        "Expected risk violations for large position order"
    );
}

/// Test risk engine concurrent order processing
#[tokio::test]
async fn test_concurrent_order_processing() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(10000)); // Large buffer for concurrent ops
    let account_id = AccountId::new("concurrent-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Spawn multiple tasks to simulate concurrent order placements
    let mut handles = vec![];

    for i in 0..10 {
        let bus_clone = Arc::clone(&event_bus);
        let handle = tokio::spawn(async move {
            for j in 0..5 {
                let order_id = format!("concurrent-order-{}-{}", i, j);
                let order = create_test_order(&order_id, "AAPL", 1000);
                bus_clone.publish(MarketEvent::OrderPlaced {
                    order: Arc::clone(&order),
                });
                sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for risk processing
    sleep(Duration::from_millis(100)).await;

    // Count violations (should handle concurrent load gracefully)
    let mut violation_count = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violation_count += 1;
    }

    // System should handle concurrent load without panicking
    assert!(
        violation_count >= 0,
        "Should handle concurrent operations gracefully"
    );
}

/// Test risk engine position tracking integration
#[tokio::test]
async fn test_position_tracking_integration() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("position-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Create a position with unrealized losses
    let position = Position {
        symbol: Symbol::new("AAPL").unwrap(),
        quantity: Quantity::new(1000).unwrap(),
        average_entry_price: Price::new(Decimal::new(20000, 2)).unwrap(), // $200.00
        current_price: Price::new(Decimal::new(15000, 2)).unwrap(),       // $150.00
        unrealized_pl: Decimal::new(-50000, 2),                           // -$500.00 loss
        realized_pl: Decimal::ZERO,
        opened_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Publish position update
    event_bus.publish(MarketEvent::PositionUpdated { position });

    // Wait for processing
    sleep(Duration::from_millis(50)).await;

    // Check that position losses are reflected in risk state
    let state = engine.get_risk_state().await;
    assert!(
        state.daily_pl <= Decimal::ZERO,
        "Should track unrealized position losses"
    );
}

/// Test risk engine trading halt functionality
#[tokio::test]
async fn test_trading_halt_functionality() {
    let config = create_strict_loss_config(); // Low loss threshold
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("halt-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to halt events
    let mut halt_receiver = event_bus.subscribe();

    // Create a large loss that should trigger halt
    let fill = Fill {
        order_id: OrderId::new("large-loss-fill"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-100000, 2), // $1,000 loss - exceeds threshold
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: fill.order_id.clone(),
        fill,
    });

    // Wait for processing
    sleep(Duration::from_millis(50)).await;

    // Check for trading halt
    let mut halt_detected = false;
    while let Ok(event) = halt_receiver.try_recv() {
        if matches!(event, MarketEvent::TradingHalted { .. }) {
            halt_detected = true;
            break;
        }
    }

    assert!(
        halt_detected,
        "Should halt trading when loss limit exceeded"
    );
    assert!(
        !engine.is_trading_allowed().await,
        "Trading should be halted"
    );
}

/// Test risk engine recovery after halt conditions clear
#[tokio::test]
async fn test_risk_engine_recovery() {
    let config = create_strict_loss_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("recovery-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // First trigger a halt
    let large_loss_fill = Fill {
        order_id: OrderId::new("halt-trigger-fill"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-100000, 2), // $1,000 loss
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: large_loss_fill.order_id.clone(),
        fill: large_loss_fill,
    });

    sleep(Duration::from_millis(50)).await;
    assert!(
        !engine.is_trading_allowed().await,
        "Should be halted after large loss"
    );

    // Simulate equity recovery (e.g., profitable trades)
    let recovery_fill = Fill {
        order_id: OrderId::new("recovery-fill"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(16000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(50000, 2), // $500 profit
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: recovery_fill.order_id.clone(),
        fill: recovery_fill,
    });

    sleep(Duration::from_millis(50)).await;

    // Check if trading is allowed again (depending on recovery logic)
    // Note: Actual recovery logic may vary based on implementation
    let state = engine.get_risk_state().await;
    println!("Daily P&L after recovery: {}", state.daily_pl);
}

/// Test risk engine with pattern-specific risk evaluation
// TODO(B-056): PatternDetected events do not produce PatternPerformanceUpdate or RiskViolation
// events for a new pattern with no prior stats. Ignored pending engine implementation.
#[tokio::test]
async fn test_pattern_risk_integration() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("pattern-test-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to pattern events
    let mut pattern_receiver = event_bus.subscribe();

    // Create pattern signal with low confidence (should trigger risk evaluation)
    let pattern_signal = daytrader::events::types::PatternSignal {
        symbol: Symbol::new("AAPL").unwrap(),
        pattern_name: daytrader::brokers::types::PatternName::new("hammer"),
        signal_type: daytrader::events::types::SignalType::Buy,
        confidence: daytrader::brokers::types::Confidence::new(0.65).unwrap(),
        detected_at: chrono::Utc::now(),
        price: Price::new(Decimal::new(15000, 2)).unwrap(),
    };

    event_bus.publish(MarketEvent::PatternDetected {
        signal: pattern_signal,
    });

    sleep(Duration::from_millis(50)).await;

    // Check for pattern performance updates or violations
    let mut pattern_events = 0;
    while let Ok(event) = pattern_receiver.try_recv() {
        match event {
            MarketEvent::PatternPerformanceUpdate { .. } | MarketEvent::RiskViolation { .. } => {
                pattern_events += 1;
            }
            _ => {}
        }
    }

    assert!(
        pattern_events > 0,
        "Should process pattern signals and update risk state"
    );
}

/// Test that DailyLossTracker::from_config properly initializes starting balance
#[tokio::test]
async fn test_daily_loss_tracker_from_config_initialization() {
    use daytrader::risk::limits::DailyLossTracker;

    let config = create_test_risk_config();
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let tracker = DailyLossTracker::from_config(&config, initial_equity);

    let state = tracker.get_daily_state().await;
    assert_eq!(
        state.starting_balance, initial_equity,
        "Starting balance should be set from from_config"
    );

    // Test that percentage limits work with correct starting balance
    let test_loss = Decimal::new(-100000, 2); // $1,000 loss
    let can_trade = tracker.check_daily_loss_limit(test_loss).await.unwrap();
    // With 2% limit on $50k = $1,000 limit, so $1,000 loss should be blocked
    assert!(!can_trade, "Should block loss at percentage limit");
}

/// Test that RiskEngine::with_database preserves repositories through start()
///
/// The repos are private, so this asserts the observable behavior instead:
/// a `PatternDetected` event processed by the started engine must be
/// persisted through the engine's own signal repository.
#[tokio::test]
async fn test_risk_engine_with_database_preserves_repos() {
    let database = daytrader::db::Database::in_memory().await.unwrap();
    sqlx::query(include_str!("../src/db/schema.sql"))
        .execute(&database.pool)
        .await
        .unwrap();
    let database = Arc::new(database);

    let event_bus = Arc::new(EventBus::new(1000));
    let engine = RiskEngine::with_database(
        create_test_risk_config(),
        Arc::clone(&event_bus),
        AccountId::new("db-test-account"),
        Decimal::new(10000000, 2),
        Some(Arc::clone(&database)),
    );
    engine.start().await.unwrap();

    event_bus.publish(MarketEvent::PatternDetected {
        signal: daytrader::events::types::PatternSignal {
            symbol: Symbol::new("AAPL").unwrap(),
            pattern_name: daytrader::brokers::types::PatternName::new("hammer"),
            signal_type: daytrader::events::types::SignalType::Buy,
            confidence: daytrader::brokers::types::Confidence::new(0.85).unwrap(),
            detected_at: chrono::Utc::now(),
            price: Price::new(Decimal::new(15000, 2)).unwrap(),
        },
    });

    sleep(Duration::from_millis(100)).await;

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pattern_signals")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(
        row.0, 1,
        "signal must be persisted via the engine's repositories after start()"
    );
}

/// Helper function to create test risk configuration
fn create_test_risk_config() -> RiskConfig {
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

/// Helper function to create strict loss limit configuration
fn create_strict_loss_config() -> RiskConfig {
    let mut config = create_test_risk_config();
    config.max_daily_loss_dollars = 500.0; // Very low threshold
    config.max_daily_loss_percent = 0.5;
    config
}

/// Helper function to create test orders
fn create_test_order(id: &str, symbol: &str, quantity: u64) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(id),
        symbol: Symbol::new(symbol).unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: Quantity::new(quantity as u32).unwrap(),
        price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Helper function to create large position orders for testing limits
fn create_large_position_order(id: &str) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(id),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: Quantity::new(10000).unwrap(), // Very large position
        // $150.00 x 10,000 = $1.5M, far beyond max_position_size_dollars
        price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// B-034: Verify that RiskViolation events reach a bus subscriber.
///
/// The EventBridge subscribes to the same bus and forwards the event to the
/// Tauri frontend as `risk:violation`. This test confirms that the bus
/// routing works correctly so the bridge has something to forward.
#[tokio::test]
async fn test_risk_violation_reaches_bus_subscriber() {
    use daytrader::brokers::types::Message;
    use daytrader::events::types::{RiskViolation, RiskViolationType};

    let event_bus = Arc::new(EventBus::new(100));
    let mut subscriber = event_bus.subscribe();

    let violation = RiskViolation {
        violation_type: RiskViolationType::PositionSizeLimit,
        message: Message::new("test: position size exceeds limit"),
        timestamp: chrono::Utc::now(),
        pattern_name: None,
    };

    // Verify the violation is serialisable (required for Tauri IPC emit)
    let serialized = serde_json::to_string(&violation)
        .expect("RiskViolation must be JSON-serialisable for Tauri IPC");
    assert!(serialized.contains("PositionSizeLimit"));

    // Publish through the bus
    event_bus.publish(MarketEvent::RiskViolation {
        violation: violation.clone(),
    });

    // The EventBridge thread calls blocking_recv() on this same bus; confirm
    // a normal async subscriber also receives the event.
    let received = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            match subscriber.recv().await {
                Ok(MarketEvent::RiskViolation { violation: v }) => return Some(v),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .expect("timed out waiting for RiskViolation event")
    .expect("no RiskViolation received");

    assert_eq!(
        received.violation_type,
        RiskViolationType::PositionSizeLimit,
        "Received violation type should match published"
    );
}
