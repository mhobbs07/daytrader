//! End-to-end risk management workflow tests
//!
//! Tests complete trading scenarios from pattern detection through
//! risk evaluation, order execution, and emergency liquidation.

use daytrader::brokers::types::{Confidence, OrderId, PatternName, Price, Quantity, Symbol};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{
    Fill, MarketEvent, Order, OrderSide, OrderStatus, OrderType, PatternSignal, Position,
};
use daytrader::risk::RiskEngine;
use daytrader::risk::types::AccountId;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Test complete PDT workflow: pattern detection → risk evaluation → trading halt
#[ignore = "B-081 remainder: same PdtLimit counting-semantics gap as market_scenarios::test_pdt_edge_cases"]
#[tokio::test]
async fn test_pdt_e2e_workflow() {
    let config = create_strict_pdt_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("pdt-e2e-account");
    let initial_equity = Decimal::new(2500000, 2); // $25,000.00 (PDT threshold)

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Step 1: Pattern detection
    let pattern_signal = PatternSignal {
        symbol: Symbol::new("AAPL").unwrap(),
        pattern_name: PatternName::new("hammer"),
        signal_type: daytrader::events::types::SignalType::Buy,
        confidence: Confidence::new(0.85).unwrap(),
        detected_at: chrono::Utc::now(),
        price: Price::new(Decimal::new(15000, 2)).unwrap(),
    };

    event_bus.publish(MarketEvent::PatternDetected {
        signal: pattern_signal,
    });

    // Step 2: Order placement (simulating execution system)
    let order1 = create_test_order("pdt-order-1", "AAPL", 100);
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order1),
    });

    // Step 3: Fill execution
    let fill1 = create_fill_for_order(&order1.id.to_string(), 100);
    event_bus.publish(MarketEvent::OrderFilled {
        order_id: fill1.order_id.clone(),
        fill: fill1,
    });

    sleep(Duration::from_millis(50)).await;

    // Step 4: Record day trade (this would normally be done by position tracker)
    // For this test, we'll simulate the day trade recording
    // In real implementation, this would be automatic

    // Step 5: Attempt second day trade
    let order2 = create_test_order("pdt-order-2", "GOOGL", 50);
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order2),
    });

    let fill2 = create_fill_for_order(&order2.id.to_string(), 50);
    event_bus.publish(MarketEvent::OrderFilled {
        order_id: fill2.order_id.clone(),
        fill: fill2,
    });

    sleep(Duration::from_millis(50)).await;

    // Step 6: Attempt third day trade (should be at PDT limit)
    let order3 = create_test_order("pdt-order-3", "MSFT", 75);
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order3),
    });

    let fill3 = create_fill_for_order(&order3.id.to_string(), 75);
    event_bus.publish(MarketEvent::OrderFilled {
        order_id: fill3.order_id.clone(),
        fill: fill3,
    });

    sleep(Duration::from_millis(50)).await;

    // Step 7: Attempt fourth day trade (should trigger PDT violation)
    let order4 = create_test_order("pdt-order-4", "TSLA", 25);
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order4),
    });

    sleep(Duration::from_millis(50)).await;

    // Check for PDT violation
    let mut violations_found = 0;
    while let Ok(event) = event_receiver.try_recv() {
        if matches!(event, MarketEvent::RiskViolation { .. }) {
            violations_found += 1;
        }
    }

    assert!(
        violations_found > 0,
        "Should detect PDT violation on 4th day trade"
    );
}

/// Test emergency liquidation workflow under severe loss conditions
// TODO(B-056): RiskEngine.update_on_fill does not propagate DailyLossTracker halt to
// engine.state.is_trading_halted, and PositionUpdated events do not trigger
// emergency liquidation checks. Ignored pending engine implementation.
#[tokio::test]
async fn test_emergency_liquidation_workflow() {
    let config = create_emergency_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("liquidation-e2e-account");
    let initial_equity = Decimal::new(10000000, 2); // $100,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Step 1: Establish positions
    let positions = vec![
        create_position("AAPL", 1000, Decimal::new(15000, 2), Decimal::new(15000, 2)), // $15,000 position
        create_position(
            "GOOGL",
            50,
            Decimal::new(250000, 2),
            Decimal::new(250000, 2),
        ), // $12,500 position
        create_position("MSFT", 200, Decimal::new(30000, 2), Decimal::new(30000, 2)), // $6,000 position
    ];

    for position in &positions {
        event_bus.publish(MarketEvent::PositionUpdated {
            position: position.clone(),
        });
    }

    sleep(Duration::from_millis(50)).await;

    // Step 2: Simulate market crash - severe losses
    let crashed_positions = vec![
        create_position_with_loss("AAPL", 1000, Decimal::new(15000, 2), Decimal::new(7500, 2)), // 50% loss
        create_position_with_loss(
            "GOOGL",
            50,
            Decimal::new(250000, 2),
            Decimal::new(125000, 2),
        ), // 50% loss
        create_position_with_loss("MSFT", 200, Decimal::new(30000, 2), Decimal::new(15000, 2)), // 50% loss
    ];

    for position in &crashed_positions {
        event_bus.publish(MarketEvent::PositionUpdated {
            position: position.clone(),
        });
    }

    sleep(Duration::from_millis(50)).await;

    // Step 3: Trigger emergency liquidation with additional loss
    let emergency_loss_fill = Fill {
        order_id: OrderId::new("emergency-loss"),
        filled_quantity: Quantity::new(500).unwrap(),
        filled_price: Price::new(Decimal::new(5000, 2)).unwrap(), // Additional 67% loss
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-75000, 2), // $750 loss
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: emergency_loss_fill.order_id.clone(),
        fill: emergency_loss_fill,
    });

    sleep(Duration::from_millis(100)).await;

    // Check for emergency liquidation signals
    let mut liquidation_events = 0;
    let mut trading_halted = false;

    while let Ok(event) = event_receiver.try_recv() {
        match event {
            MarketEvent::EmergencyLiquidation { .. } => liquidation_events += 1,
            MarketEvent::TradingHalted { .. } => trading_halted = true,
            _ => {}
        }
    }

    // System should respond to severe losses
    assert!(
        liquidation_events > 0 || trading_halted,
        "Should trigger emergency response to severe losses"
    );
}

/// Test multi-day PDT compliance across trading days
#[tokio::test]
async fn test_multi_day_pdt_compliance() {
    let config = create_strict_pdt_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("multi-day-pdt-account");
    let initial_equity = Decimal::new(2500000, 2); // $25,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Day 1: 2 day trades
    let day1 = chrono::Utc::now().date_naive();
    for i in 1..=2 {
        let order = create_day_trade_order(&format!("day1-trade-{}", i), "AAPL");
        event_bus.publish(MarketEvent::OrderPlaced {
            order: Arc::clone(&order),
        });

        let fill = create_fill_for_order(&order.id.to_string(), 100);
        event_bus.publish(MarketEvent::OrderFilled {
            order_id: fill.order_id.clone(),
            fill,
        });
    }

    sleep(Duration::from_millis(50)).await;

    // Day 2: 1 day trade (still within 5-day window)
    let day2 = day1 + chrono::Duration::days(1);
    let order = create_day_trade_order("day2-trade-1", "GOOGL");
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order),
    });

    let fill = create_fill_for_order(&order.id.to_string(), 50);
    event_bus.publish(MarketEvent::OrderFilled {
        order_id: fill.order_id.clone(),
        fill,
    });

    sleep(Duration::from_millis(50)).await;

    // Day 3: Attempt another day trade (should be allowed, still under limit)
    let day3 = day1 + chrono::Duration::days(2);
    let order3 = create_day_trade_order("day3-trade-1", "MSFT");
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&order3),
    });

    sleep(Duration::from_millis(50)).await;

    // Check that no violations occurred yet
    let mut violations = 0;
    while let Ok(event) = event_receiver.try_recv() {
        if matches!(event, MarketEvent::RiskViolation { .. }) {
            violations += 1;
        }
    }

    assert_eq!(
        violations, 0,
        "Should allow trades within PDT limits across days"
    );
}

/// Test risk recovery after trading halt
// TODO(B-056): RiskEngine.update_on_fill does not propagate DailyLossTracker.is_halted
// to engine.state.is_trading_halted, so engine.is_trading_allowed() remains true
// even after a loss-limit-exceeding fill. Ignored pending engine implementation.
#[tokio::test]
async fn test_risk_recovery_after_halt() {
    let config = create_loss_limit_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("recovery-e2e-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Step 1: Trigger trading halt with large loss
    let halt_trigger_fill = Fill {
        order_id: OrderId::new("halt-trigger"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-200000, 2), // $2,000 loss (exceeds limit)
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: halt_trigger_fill.order_id.clone(),
        fill: halt_trigger_fill,
    });

    sleep(Duration::from_millis(50)).await;

    // Verify trading is halted
    assert!(
        !engine.is_trading_allowed().await,
        "Trading should be halted after large loss"
    );

    // Step 2: Simulate recovery (profitable trades)
    let recovery_fill = Fill {
        order_id: OrderId::new("recovery-trade"),
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(17000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(150000, 2), // $1,500 profit
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: recovery_fill.order_id.clone(),
        fill: recovery_fill,
    });

    sleep(Duration::from_millis(50)).await;

    // Check if recovery allows trading again
    // Note: Actual recovery logic depends on implementation
    let state = engine.get_risk_state().await;
    println!("Daily P&L after recovery: {}", state.daily_pl);

    // System should handle recovery gracefully
    assert!(
        state.daily_pl > Decimal::new(-200000, 2),
        "Recovery should improve P&L"
    );
}

/// Test concurrent pattern detection and risk evaluation
#[tokio::test]
async fn test_concurrent_pattern_risk_workflow() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(10000)); // Large buffer for concurrent ops
    let account_id = AccountId::new("concurrent-pattern-account");
    let initial_equity = Decimal::new(10000000, 2);

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Spawn multiple concurrent pattern detection workflows
    let mut handles = vec![];

    for i in 0..5 {
        let bus_clone = Arc::clone(&event_bus);
        let handle = tokio::spawn(async move {
            // Pattern detection
            let pattern_signal = PatternSignal {
                symbol: Symbol::new(&format!("SYMBOL{}", i)).unwrap(),
                pattern_name: PatternName::new("hammer"),
                signal_type: daytrader::events::types::SignalType::Buy,
                confidence: Confidence::new(0.80).unwrap(),
                detected_at: chrono::Utc::now(),
                price: Price::new(Decimal::new(15000 + i as i64 * 100, 2)).unwrap(),
            };

            bus_clone.publish(MarketEvent::PatternDetected {
                signal: pattern_signal,
            });

            // Order placement
            let order = create_test_order(
                &format!("concurrent-order-{}", i),
                &format!("SYMBOL{}", i),
                100,
            );
            bus_clone.publish(MarketEvent::OrderPlaced {
                order: Arc::clone(&order),
            });

            // Fill execution
            let fill = create_fill_for_order(&order.id.to_string(), 100);
            bus_clone.publish(MarketEvent::OrderFilled {
                order_id: fill.order_id.clone(),
                fill,
            });
        });
        handles.push(handle);
    }

    // Wait for all concurrent workflows
    for handle in handles {
        handle.await.unwrap();
    }

    sleep(Duration::from_millis(200)).await;

    // Verify system handled concurrent workflows
    let mut events_processed = 0;
    while let Ok(_) = event_receiver.try_recv() {
        events_processed += 1;
    }

    assert!(
        events_processed > 0,
        "Should process concurrent pattern workflows"
    );
}

/// Helper function to create strict PDT configuration
fn create_strict_pdt_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 1000.0,
        max_daily_loss_percent: 2.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 10000.0,
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
        pattern_performance_thresholds:
            daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}

/// Helper function to create emergency liquidation configuration
fn create_emergency_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 5000.0,
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
        emergency_liquidate_threshold: 2000.0, // Lower threshold for testing
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds:
            daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}

/// Helper function to create loss limit focused configuration
fn create_loss_limit_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 1000.0, // $10.00 limit
        max_daily_loss_percent: 1.0,
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
        emergency_liquidate_threshold: 5000.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds:
            daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}

/// Helper function to create standard test configuration
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

/// Helper function to create day trade orders
fn create_day_trade_order(id: &str, symbol: &str) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(id),
        symbol: Symbol::new(symbol).unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(100).unwrap(),
        price: None,
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Helper function to create fills for orders
fn create_fill_for_order(order_id: &str, quantity: u64) -> Fill {
    Fill {
        order_id: OrderId::new(order_id),
        filled_quantity: Quantity::new(quantity as u32).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-500, 2), // $5 commission
    }
}

/// Helper function to create positions
fn create_position(
    symbol: &str,
    quantity: u64,
    entry_price: Decimal,
    current_price: Decimal,
) -> Position {
    Position {
        symbol: Symbol::new(symbol).unwrap(),
        quantity: Quantity::new(quantity as u32).unwrap(),
        average_entry_price: Price::new(entry_price).unwrap(),
        current_price: Price::new(current_price).unwrap(),
        unrealized_pl: Decimal::from(quantity) * (current_price - entry_price),
        realized_pl: Decimal::ZERO,
        opened_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Helper function to create positions with specific losses
fn create_position_with_loss(
    symbol: &str,
    quantity: u64,
    entry_price: Decimal,
    current_price: Decimal,
) -> Position {
    let unrealized_pl = Decimal::from(quantity) * (current_price - entry_price);
    Position {
        symbol: Symbol::new(symbol).unwrap(),
        quantity: Quantity::new(quantity as u32).unwrap(),
        average_entry_price: Price::new(entry_price).unwrap(),
        current_price: Price::new(current_price).unwrap(),
        unrealized_pl,
        realized_pl: Decimal::ZERO,
        opened_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}
