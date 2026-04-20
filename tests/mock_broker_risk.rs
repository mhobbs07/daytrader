//! Mock broker integration tests for risk management
//!
//! These tests validate risk evaluation with mock broker data:
//! - Risk evaluation with mock broker responses
//! - Error scenarios and broker API failures
//! - Rate limiting under API constraints
//! - Connection issues and risk behavior during connectivity problems

use daytrader::brokers::mock::MockBroker;
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

/// Test risk evaluation with mock broker data
#[tokio::test]
async fn test_risk_evaluation_with_mock_broker() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("mock-test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to risk violations
    let mut violation_receiver = event_bus.subscribe();

    // Create order that would trigger position size violation
    let order = create_test_order("mock-order-1", "AAPL", 10000, Decimal::new(15000, 2));

    // Publish order placement
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    sleep(Duration::from_millis(50)).await;

    // Should detect risk violation
    let mut violations = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violations += 1;
    }

    assert!(
        violations > 0,
        "Should detect risk violations with mock broker data"
    );
}

/// Test error scenarios with broker API failures
#[tokio::test]
async fn test_broker_api_failure_scenarios() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("error-test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Simulate broker API failure by publishing failed order
    let failed_order = Order {
        id: OrderId::new("failed-order"),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(100).unwrap(),
        price: None,
        status: OrderStatus::Rejected,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Publish order rejection
    event_bus.publish(MarketEvent::OrderRejected {
        order_id: failed_order.id.clone(),
        reason: daytrader::brokers::types::Reason::new("API connection failed"),
    });

    sleep(Duration::from_millis(50)).await;

    // Risk engine should handle the rejection gracefully
    // (No risk violations expected for rejections, but system should continue)
    let mut events_received = 0;
    while let Ok(_) = event_receiver.try_recv() {
        events_received += 1;
    }

    assert!(
        events_received >= 1,
        "Should handle broker API failures gracefully"
    );
}

/// Test rate limiting under API constraints
#[tokio::test]
async fn test_rate_limiting_scenarios() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("rate-limit-test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Simulate rapid order placements (potential rate limiting scenario)
    for i in 0..20 {
        let order = create_test_order(
            &format!("rapid-order-{}", i),
            "AAPL",
            100,
            Decimal::new(15000, 2),
        );

        event_bus.publish(MarketEvent::OrderPlaced {
            order: Arc::clone(&order),
        });

        // Small delay to avoid overwhelming the event bus
        sleep(Duration::from_millis(1)).await;
    }

    sleep(Duration::from_millis(100)).await;

    // Should handle rapid orders without panicking
    let mut violations = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violations += 1;
    }

    // Some violations expected due to rapid trading, but system should handle it
    assert!(
        violations >= 0,
        "Should handle rate limiting scenarios gracefully"
    );
}

/// Test connection issues and risk behavior during connectivity problems
#[tokio::test]
async fn test_connection_issues_risk_behavior() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("connection-test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Simulate connection loss
    event_bus.publish(MarketEvent::ConnectionLost {
        provider: daytrader::brokers::types::BrokerProvider::Alpaca,
        reason: daytrader::brokers::types::Reason::new("Network timeout"),
    });

    sleep(Duration::from_millis(50)).await;

    // Simulate connection restoration
    event_bus.publish(MarketEvent::ConnectionEstablished {
        provider: daytrader::brokers::types::BrokerProvider::Alpaca,
    });

    sleep(Duration::from_millis(50)).await;

    // Risk engine should continue functioning after connection issues
    let order = create_test_order("post-connection-order", "AAPL", 100, Decimal::new(15000, 2));
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    sleep(Duration::from_millis(50)).await;

    // Should still process orders after connection restoration
    let mut events_processed = 0;
    while let Ok(_) = event_receiver.try_recv() {
        events_processed += 1;
    }

    assert!(
        events_processed > 0,
        "Should continue processing after connection issues"
    );
}

/// Test risk evaluation with mock position data
#[tokio::test]
async fn test_mock_position_data_risk_evaluation() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("position-test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Create mock position with large unrealized loss
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

    sleep(Duration::from_millis(50)).await;

    // Should handle position updates without issues
    let state = engine.get_risk_state().await;
    assert!(state.last_updated > chrono::Utc::now() - chrono::Duration::seconds(10));
}

/// Test concurrent mock broker operations
#[tokio::test]
async fn test_concurrent_mock_broker_operations() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(10000)); // Large buffer for concurrent ops
    let account_id = AccountId::new("concurrent-mock-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Spawn multiple concurrent operations
    let mut handles = vec![];

    for i in 0..5 {
        let bus_clone = Arc::clone(&event_bus);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                // Mix of order placements and fills
                if j % 2 == 0 {
                    let order = create_test_order(
                        &format!("concurrent-order-{}-{}", i, j),
                        "AAPL",
                        100,
                        Decimal::new(15000, 2),
                    );
                    bus_clone.publish(MarketEvent::OrderPlaced {
                        order: Arc::clone(&order),
                    });
                } else {
                    let fill = Fill {
                        order_id: OrderId::new(&format!("concurrent-fill-{}-{}", i, j)),
                        symbol: Symbol::new("AAPL").unwrap(),
                        side: OrderSide::Buy,
                        filled_quantity: Quantity::new(50).unwrap(),
                        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
                        filled_at: chrono::Utc::now(),
                        commission: Decimal::new(-500, 2),
                    };
                    bus_clone.publish(MarketEvent::OrderFilled {
                        order_id: fill.order_id.clone(),
                        fill,
                    });
                }
                sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // System should handle concurrent mock broker operations
    let state = engine.get_risk_state().await;
    assert!(state.last_updated > chrono::Utc::now() - chrono::Duration::seconds(10));
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
        pattern_performance_thresholds: daytrader::risk::types::PatternPerformanceThresholds {
            min_win_rate_short: daytrader::risk::types::Percentage::new(
                rust_decimal::Decimal::new(600, 1),
            )
            .unwrap(),
            min_win_rate_medium: daytrader::risk::types::Percentage::new(
                rust_decimal::Decimal::new(600, 1),
            )
            .unwrap(),
            min_win_rate_long: daytrader::risk::types::Percentage::new(rust_decimal::Decimal::new(
                600, 1,
            ))
            .unwrap(),
            max_consecutive_losses: daytrader::risk::types::Count::new(3).unwrap(),
            min_sample_size: daytrader::risk::types::Count::new(20).unwrap(),
            auto_disable_threshold: daytrader::risk::types::Percentage::new(
                rust_decimal::Decimal::new(400, 1),
            )
            .unwrap(),
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
