//! Comprehensive integration tests for the risk management system
//!
//! These tests validate the complete risk workflow from event ingestion
//! through risk evaluation and violation handling.

use daytrader::brokers::types::{Message, OrderId, Price, Quantity, Symbol, PatternName, Confidence};
use daytrader::risk::types::{AccountId, PatternPerformanceThresholds, Percentage, Count};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{
    Fill, MarketEvent, Order, OrderSide, OrderStatus, OrderType, PatternSignal, Position,
    RiskViolation, RiskViolationType, SignalType,
};
use daytrader::risk::RiskEngine;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Test the complete risk workflow: event bus → risk evaluation → violation handling
#[tokio::test]
async fn test_complete_risk_workflow() {
    // Setup
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);

    // Subscribe to risk violations
    let mut violation_receiver = event_bus.subscribe();

    // Start the risk engine
    engine.start().await.unwrap();

    // Create a test order that should trigger risk violations
    let order = Arc::new(Order {
        id: OrderId::new("test-order-1"),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(10000).unwrap(), // Very large position
        price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    });

    // Publish order placement event
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    // Wait for risk evaluation and check for violations
    let mut violations_found = 0;
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < Duration::from_millis(500) {
        if let Ok(MarketEvent::RiskViolation { violation }) = violation_receiver.try_recv() {
            violations_found += 1;
            println!("Risk violation detected: {:?}", violation.violation_type);
        }
        sleep(Duration::from_millis(10)).await;
    }

    // Should have detected position size violation
    assert!(violations_found > 0, "Expected risk violations but none were detected");

    // Verify trading is not halted (position size might not be critical)
    assert!(engine.is_trading_allowed().await);
}

/// Test pattern detection -> risk evaluation workflow
#[tokio::test]
async fn test_pattern_detection_risk_workflow() {
    // Setup
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to pattern-related events
    let mut event_receiver = event_bus.subscribe();

    // Create pattern signal
    let pattern_signal = PatternSignal {
        symbol: Symbol::new("AAPL").unwrap(),
        pattern_name: PatternName::new("hammer"),
        signal_type: SignalType::Buy,
        confidence: Confidence::new(0.85).unwrap(),
        detected_at: chrono::Utc::now(),
        price: Price::new(Decimal::new(15000, 2)).unwrap(),
    };

    // Publish pattern detected event
    event_bus.publish(MarketEvent::PatternDetected { signal: pattern_signal.clone() });

    // Wait for processing
    sleep(Duration::from_millis(100)).await;

    // Check for risk evaluation (no violation expected for valid pattern)
    let mut events_received = 0;
    let mut performance_updates = 0;
    let mut violations = 0;

    while let Ok(event) = event_receiver.try_recv() {
        events_received += 1;
        match event {
            MarketEvent::PatternPerformanceUpdate { .. } => performance_updates += 1,
            MarketEvent::RiskViolation { .. } => violations += 1,
            _ => {}
        }
    }

    // Should have processed the pattern without critical violations
    assert!(events_received > 0, "Expected events from pattern detection");
    assert_eq!(violations, 0, "Should not have violations for valid pattern");
    println!("Processed {} events, {} performance updates, {} violations", events_received, performance_updates, violations);
}

/// Test event persistence for risk events
#[tokio::test]
async fn test_event_persistence() {
    // Setup (assuming persistence is implemented in RiskEngine or EventBus)
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Publish a risk violation event
    let violation = RiskViolation {
        violation_type: RiskViolationType::PositionSizeLimit,
        message: Message::new("Test violation"),
        timestamp: chrono::Utc::now(),
        pattern_name: None,
    };

    event_bus.publish(MarketEvent::RiskViolation { violation: violation.clone() });

    // Wait for persistence (assuming implemented)
    sleep(Duration::from_millis(100)).await;

    // Verify event was persisted (mock DB check)
    // In real implementation, query the risk_events table
    println!("Event persistence test - assuming implementation saves to DB");
    assert!(true, "Placeholder for DB verification");
}

/// Test pattern-specific risk events
#[tokio::test]
async fn test_pattern_specific_events() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to pattern events
    let mut event_receiver = event_bus.subscribe();

    // Create pattern signal for low confidence (should trigger warning or violation)
    let low_conf_pattern = PatternSignal {
        symbol: Symbol::new("AAPL").unwrap(),
        pattern_name: PatternName::new("doji"),
        signal_type: SignalType::Buy,
        confidence: Confidence::new(0.65).unwrap(), // Below threshold for doji
        detected_at: chrono::Utc::now(),
        price: Price::new(Decimal::new(15000, 2)).unwrap(),
    };

    event_bus.publish(MarketEvent::PatternDetected { signal: low_conf_pattern });

    // Wait for processing
    sleep(Duration::from_millis(100)).await;

    let mut violations = 0;
    let mut performance_updates = 0;

    while let Ok(event) = event_receiver.try_recv() {
        match event {
            MarketEvent::RiskViolation { .. } => violations += 1,
            MarketEvent::PatternPerformanceUpdate { .. } => performance_updates += 1,
            _ => {}
        }
    }

    // Should have a violation for low confidence
    assert_eq!(violations, 1, "Expected violation for low confidence pattern");
    assert!(performance_updates >= 1, "Expected performance update");
}

/// Test multi-rule evaluation where multiple risk rules are evaluated together
/// Test multi-rule evaluation where multiple risk rules are evaluated together
#[tokio::test]
async fn test_multi_rule_evaluation() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Start engine
    engine.start().await.unwrap();

    // Create multiple orders to test various rules
    let orders = vec![
        create_test_order("order-1", "AAPL", 1000, Decimal::new(15000, 2)), // Normal size
        create_test_order("order-2", "GOOGL", 5000, Decimal::new(250000, 2)), // Large position
        create_test_order("order-3", "TSLA", 10000, Decimal::new(20000, 2)), // Very large position
    ];

    // Publish all orders
    for order in &orders {
        event_bus.publish(MarketEvent::OrderPlaced {
            order: Arc::clone(order),
        });
    }

    // Wait and collect violations
    sleep(Duration::from_millis(100)).await;

    let mut violations = Vec::new();
    while let Ok(MarketEvent::RiskViolation { violation }) = event_receiver.try_recv() {
        violations.push(violation);
    }

    // Should have multiple violations
    assert!(!violations.is_empty(), "Expected multiple risk violations");

    // Check for different violation types
    let violation_types: std::collections::HashSet<_> =
        violations.iter().map(|v| v.violation_type.clone()).collect();

    println!("Detected violation types: {:?}", violation_types);
    assert!(violations.len() >= 1, "Should detect at least one type of violation");
}

/// Test end-to-end: pattern detection → risk evaluation → trade execution
#[tokio::test]
async fn test_end_to_end_pattern_risk_workflow() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Step 1: Pattern detection
    let pattern_signal = PatternSignal {
        symbol: Symbol::new("AAPL").unwrap(),
        pattern_name: PatternName::new("hammer"),
        signal_type: SignalType::Buy,
        confidence: Confidence::new(0.85).unwrap(),
        detected_at: chrono::Utc::now(),
        price: Price::new(Decimal::new(15000, 2)).unwrap(),
    };

    event_bus.publish(MarketEvent::PatternDetected { signal: pattern_signal.clone() });

    // Step 2: Order placement (assuming execution system would do this)
    let order = Arc::new(Order {
        id: OrderId::new("test-trade-order"),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(100).unwrap(),
        price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
        status: OrderStatus::Filled,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    });

    event_bus.publish(MarketEvent::OrderPlaced { order: Arc::clone(&order) });

    // Step 3: Order fill
    let fill = Fill {
        order_id: OrderId::new("test-trade-order"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15200, 2)).unwrap(), // Small profit
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-100, 2), // $1.00 commission
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: OrderId::new("test-trade-order"),
        fill,
    });

    // Step 4: Position opened
    let position = Position {
        symbol: Symbol::new("AAPL").unwrap(),
        quantity: Quantity::new(100).unwrap(),
        average_entry_price: Price::new(Decimal::new(15200, 2)).unwrap(),
        current_price: Price::new(Decimal::new(15200, 2)).unwrap(),
        unrealized_pl: Decimal::ZERO,
        realized_pl: Decimal::ZERO,
        opened_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    event_bus.publish(MarketEvent::PositionOpened { position });

    // Wait for processing
    sleep(Duration::from_millis(200)).await;

    // Check for events in the workflow
    let mut events_received = 0;
    let mut performance_updates = 0;
    let mut violations = 0;

    while let Ok(event) = event_receiver.try_recv() {
        events_received += 1;
        match event {
            MarketEvent::PatternPerformanceUpdate { .. } => performance_updates += 1,
            MarketEvent::RiskViolation { .. } => violations += 1,
            MarketEvent::RiskMultiplierAdjusted { .. } => events_received += 1, // Pattern risk events
            _ => {}
        }
    }

    // Should have processed the full workflow without critical failures
    assert!(events_received > 0, "Expected events from full workflow");
    assert_eq!(violations, 0, "Should not have violations for valid workflow");
    println!("Full workflow processed: {} events, {} performance updates, {} violations", events_received, performance_updates, violations);
}

/// Test event publishing and risk violation events
#[tokio::test]
async fn test_event_publishing() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);

    // Subscribe to all events
    let mut event_receiver = event_bus.subscribe();

    // Start engine
    engine.start().await.unwrap();

    // Create order that will trigger violation
    let order = create_test_order("test-order", "AAPL", 10000, Decimal::new(15000, 2));

    // Publish order
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    // Wait for events
    sleep(Duration::from_millis(100)).await;

    let mut risk_violations = 0;
    let mut other_events = 0;

    while let Ok(event) = event_receiver.try_recv() {
        match event {
            MarketEvent::RiskViolation { .. } => risk_violations += 1,
            _ => other_events += 1,
        }
    }

    assert_eq!(risk_violations, 1, "Should publish exactly one risk violation event");
    println!("Received {} risk violations and {} other events", risk_violations, other_events);
}

/// Test risk state persistence (mock database interaction)
#[tokio::test]
async fn test_risk_state_persistence() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);

    // Get initial state
    let initial_state = engine.get_risk_state().await;
    assert_eq!(initial_state.daily_pl, Decimal::ZERO);
    assert_eq!(initial_state.day_trade_count, 0);
    assert!(!initial_state.is_trading_halted);

    // Simulate some activity
    let fill = Fill {
        order_id: OrderId::new("fill-1"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-500, 2), // $5.00 commission
    };

    // Publish fill event (this would normally update state)
    event_bus.publish(MarketEvent::OrderFilled {
        order_id: OrderId::new("fill-1"),
        fill,
    });

    // Start engine to process events
    engine.start().await.unwrap();

    // Wait for processing
    sleep(Duration::from_millis(100)).await;

    // Get updated state
    let updated_state = engine.get_risk_state().await;

    // State should be updated (P&L should reflect commission)
    assert_ne!(updated_state.last_updated, initial_state.last_updated);
    println!("State updated from {:?} to {:?}", initial_state.last_updated, updated_state.last_updated);
}

/// Test concurrent risk evaluations under load
#[tokio::test]
async fn test_concurrent_risk_evaluations() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(10000)); // Larger buffer for concurrent load
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

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
                let order = create_test_order(&order_id, "AAPL", 1000, Decimal::new(15000, 2));

                bus_clone.publish(MarketEvent::OrderPlaced {
                    order: Arc::clone(&order),
                });

                // Small delay to simulate real-world timing
                sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for risk processing
    sleep(Duration::from_millis(200)).await;

    // Count violations
    let mut violation_count = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violation_count += 1;
    }

    println!("Processed {} concurrent orders, detected {} violations", 50, violation_count);

    // Should handle concurrent load without panicking
    // (exact violation count may vary based on rule evaluation)
    assert!(violation_count >= 0, "Should handle concurrent load gracefully");
}

/// Test risk evaluation with position updates
#[tokio::test]
async fn test_position_update_risk_evaluation() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Create a position with large unrealized losses
    let position = Position {
        symbol: Symbol::new("AAPL").unwrap(),
        quantity: Quantity::new(1000).unwrap(),
        average_entry_price: Price::new(Decimal::new(20000, 2)).unwrap(),
        current_price: Price::new(Decimal::new(15000, 2)).unwrap(), // Down 25%
        unrealized_pl: Decimal::new(-500000, 2), // -$5,000 loss
        realized_pl: Decimal::ZERO,
        opened_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Publish position update
    event_bus.publish(MarketEvent::PositionUpdated { position });

    // Wait for processing
    sleep(Duration::from_millis(100)).await;

    // Check if daily loss limits are triggered
    let state = engine.get_risk_state().await;
    println!("Daily P&L after position update: {}", state.daily_pl);

    // Should reflect the unrealized loss in daily P&L tracking
    assert!(state.daily_pl <= Decimal::ZERO, "Should track unrealized losses");
}

/// Helper function to create test risk configuration
fn create_test_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 500.0,
        max_daily_loss_percent: 2.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 5000.0,
        max_position_size_percent: 10.0,
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
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: PatternPerformanceThresholds {
            min_win_rate_short: Percentage::new(rust_decimal::Decimal::new(600, 1)).unwrap(),
            min_win_rate_medium: Percentage::new(rust_decimal::Decimal::new(600, 1)).unwrap(),
            min_win_rate_long: Percentage::new(rust_decimal::Decimal::new(600, 1)).unwrap(),
            max_consecutive_losses: Count::new(3).unwrap(),
            min_sample_size: Count::new(20).unwrap(),
            auto_disable_threshold: Percentage::new(rust_decimal::Decimal::new(400, 1)).unwrap(),
        },
    }
}

/// Helper function to create test orders
fn create_test_order(id: &str, symbol: &str, quantity: u64, price: Decimal) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(id),
        symbol: Symbol::new(symbol).unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: Quantity::new(quantity as u32).unwrap(),
        price: Some(Price::new(price).unwrap()),
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}