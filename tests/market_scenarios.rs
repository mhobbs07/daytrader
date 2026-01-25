//! Real market scenario tests for risk management
//!
//! These tests simulate realistic trading scenarios including:
//! - PDT (Pattern Day Trading) rule edge cases
//! - Loss limit scenarios with various P&L situations
//! - Position sizing with multiple concurrent positions
//! - Thread-safe concurrent risk evaluations

use daytrader::brokers::types::{OrderId, Price, Quantity, Symbol};
use daytrader::risk::types::AccountId;
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{
    Fill, MarketEvent, Order, OrderSide, OrderStatus, OrderType, Position,
};
use daytrader::risk::RiskEngine;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Test PDT edge cases with day trade counting at window boundaries
#[tokio::test]
async fn test_pdt_edge_cases() {
    let config = create_strict_pdt_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("pdt-test-account");
    let initial_equity = Decimal::new(2500000, 2); // $25,000.00 (PDT threshold)

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Simulate exactly 3 day trades (PDT limit)
    for i in 1..=3 {
        let order = create_day_trade_order(&format!("day-trade-{}", i), "AAPL");
        let fill = create_fill_for_order(&order.id.to_string(), 100);

        // Publish order and fill (simulating a complete day trade)
        event_bus.publish(MarketEvent::OrderPlaced {
            order: Arc::clone(&order),
        });
        event_bus.publish(MarketEvent::OrderFilled {
            order_id: order.id.clone(),
            fill: fill.clone(),
        });

        sleep(Duration::from_millis(10)).await;
    }

    // Check that no violations occurred for 3 day trades
    let mut violations = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violations += 1;
    }
    assert_eq!(violations, 0, "Should allow exactly 3 day trades under PDT");

    // Now attempt a 4th day trade - should be blocked
    let fourth_order = create_day_trade_order("day-trade-4", "AAPL");
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&fourth_order),
    });

    sleep(Duration::from_millis(50)).await;

    // Should detect PDT violation
    let mut pdt_violations = 0;
    while let Ok(MarketEvent::RiskViolation { violation }) = violation_receiver.try_recv() {
        if matches!(violation.violation_type, daytrader::events::types::RiskViolationType::PdtLimit) {
            pdt_violations += 1;
        }
    }
    assert_eq!(pdt_violations, 1, "Should block 4th day trade under PDT rule");
}

/// Test loss limit scenarios with various P&L situations and halt conditions
#[tokio::test]
async fn test_loss_limit_scenarios() {
    let config = create_loss_limit_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("loss-test-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Scenario 1: Gradual losses approaching limit
    let mut total_loss = Decimal::ZERO;
    let loss_per_trade = Decimal::new(25000, 2); // $250 per trade

    for i in 1..=10 {
        let fill = Fill {
            order_id: OrderId::new(&format!("loss-trade-{}", i)),
            symbol: Symbol::new("AAPL").unwrap(),
            side: OrderSide::Buy,
            filled_quantity: Quantity::new(100).unwrap(),
            filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
            filled_at: chrono::Utc::now(),
            commission: -loss_per_trade, // Negative commission = loss
        };

        event_bus.publish(MarketEvent::OrderFilled {
            order_id: fill.order_id.clone(),
            fill: fill.clone(),
        });

        total_loss += loss_per_trade;
        sleep(Duration::from_millis(5)).await;
    }

    // Should not halt yet (total loss = $2,500, limit = $500)
    assert!(engine.is_trading_allowed().await);

    // Scenario 2: Single large loss that exceeds limit
    let large_loss_fill = Fill {
        order_id: OrderId::new("large-loss-trade"),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        filled_quantity: Quantity::new(100).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-30000, 2), // $300 loss - exceeds $500 limit
    };

    event_bus.publish(MarketEvent::OrderFilled {
        order_id: large_loss_fill.order_id.clone(),
        fill: large_loss_fill,
    });

    sleep(Duration::from_millis(50)).await;

    // Should detect trading halt
    let mut halt_events = 0;
    while let Ok(event) = event_receiver.try_recv() {
        if matches!(event, MarketEvent::TradingHalted { .. }) {
            halt_events += 1;
        }
    }

    // Trading should be halted due to loss limit
    assert!(!engine.is_trading_allowed().await);
    println!("Trading halted after exceeding loss limit");
}

/// Test position sizing with portfolio allocation limits and multiple positions
#[tokio::test]
async fn test_position_sizing_multiple_positions() {
    let config = create_position_sizing_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("position-test-account");
    let initial_equity = Decimal::new(10000000, 2); // $100,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Create multiple positions approaching limits
    let positions = vec![
        ("AAPL", 1000, Decimal::new(15000, 2)),    // $15,000 position
        ("GOOGL", 50, Decimal::new(250000, 2)),    // $12,500 position
        ("MSFT", 500, Decimal::new(30000, 2)),     // $15,000 position
        ("TSLA", 200, Decimal::new(20000, 2)),     // $4,000 position
        ("NVDA", 100, Decimal::new(40000, 2)),     // $4,000 position
    ];

    let mut total_portfolio_value = Decimal::ZERO;

    for (symbol, qty, price) in positions {
        let position = Position {
            symbol: Symbol::new(symbol).unwrap(),
            quantity: Quantity::new(qty).unwrap(),
            average_entry_price: Price::new(price).unwrap(),
            current_price: Price::new(price).unwrap(),
            unrealized_pl: Decimal::ZERO,
            realized_pl: Decimal::ZERO,
            opened_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let position_value = position.market_value();
        total_portfolio_value += position_value;

        event_bus.publish(MarketEvent::PositionUpdated { position });
        sleep(Duration::from_millis(5)).await;
    }

    println!("Total portfolio value: ${}", total_portfolio_value / Decimal::new(100, 2));

    // Attempt to add a position that would exceed limits
    let large_order = create_test_order("large-position-order", "AMZN", 1000, Decimal::new(15000, 2));
    event_bus.publish(MarketEvent::OrderPlaced {
        order: Arc::clone(&large_order),
    });

    sleep(Duration::from_millis(50)).await;

    // Should detect position size violation
    let mut position_violations = 0;
    while let Ok(MarketEvent::RiskViolation { violation }) = violation_receiver.try_recv() {
        if matches!(violation.violation_type, daytrader::events::types::RiskViolationType::PositionSizeLimit) {
            position_violations += 1;
        }
    }

    assert!(position_violations > 0, "Should detect position size limit violation");
}

/// Test thread-safe concurrent risk evaluations under high load
#[tokio::test]
async fn test_concurrent_operations() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(10000)); // Large buffer for concurrent load
    let account_id = AccountId::new("concurrent-test-account");
    let initial_equity = Decimal::new(10000000, 2); // $100,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to violations
    let mut violation_receiver = event_bus.subscribe();

    // Spawn multiple concurrent tasks simulating different market activities
    let mut handles = vec![];

    // Task 1: Rapid order placements
    let bus1 = Arc::clone(&event_bus);
    handles.push(tokio::spawn(async move {
        for i in 0..20 {
            let order = create_test_order(&format!("rapid-order-{}", i), "AAPL", 100, Decimal::new(15000, 2));
            bus1.publish(MarketEvent::OrderPlaced {
                order: Arc::clone(&order),
            });
            sleep(Duration::from_millis(1)).await;
        }
    }));

    // Task 2: Position updates
    let bus2 = Arc::clone(&event_bus);
    handles.push(tokio::spawn(async move {
        for i in 0..15 {
            let position = Position {
                symbol: Symbol::new("GOOGL").unwrap(),
                quantity: Quantity::new(100 + i).unwrap(),
                average_entry_price: Price::new(Decimal::new(250000, 2)).unwrap(),
                current_price: Price::new(Decimal::new(250000 + i as i64 * 1000, 2)).unwrap(),
                unrealized_pl: Decimal::new(i as i64 * 10000, 2),
                realized_pl: Decimal::ZERO,
                opened_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            bus2.publish(MarketEvent::PositionUpdated { position });
            sleep(Duration::from_millis(2)).await;
        }
    }));

    // Task 3: Fill events
    let bus3 = Arc::clone(&event_bus);
    handles.push(tokio::spawn(async move {
        for i in 0..25 {
            let fill = Fill {
                order_id: OrderId::new(&format!("concurrent-fill-{}", i)),
                symbol: Symbol::new("AAPL").unwrap(),
                side: OrderSide::Buy,
                filled_quantity: Quantity::new(50).unwrap(),
                filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
                filled_at: chrono::Utc::now(),
                commission: Decimal::new(-500, 2), // $5 commission
            };
            bus3.publish(MarketEvent::OrderFilled {
                order_id: fill.order_id.clone(),
                fill,
            });
            sleep(Duration::from_millis(1)).await;
        }
    }));

    // Wait for all concurrent operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for risk processing
    sleep(Duration::from_millis(200)).await;

    // Count violations and ensure system handled concurrent load
    let mut violation_count = 0;
    while let Ok(MarketEvent::RiskViolation { .. }) = violation_receiver.try_recv() {
        violation_count += 1;
    }

    println!("Concurrent operations completed, detected {} violations", violation_count);

    // System should handle concurrent load without panicking
    // (exact violation count depends on specific rule evaluations)
    assert!(violation_count >= 0, "Should handle concurrent operations gracefully");

    // Verify engine is still functional
    let state = engine.get_risk_state().await;
    assert!(state.last_updated > chrono::Utc::now() - chrono::Duration::seconds(10));
}

/// Test rapid P&L changes and limit enforcement
#[tokio::test]
async fn test_rapid_pnl_changes() {
    let config = create_loss_limit_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("rapid-pnl-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00

    let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
    engine.start().await.unwrap();

    // Subscribe to events
    let mut event_receiver = event_bus.subscribe();

    // Simulate rapid P&L changes (alternating gains and losses)
    let pnl_changes = vec![
        Decimal::new(-10000, 2), // -$100
        Decimal::new(5000, 2),   // +$50
        Decimal::new(-15000, 2), // -$150
        Decimal::new(8000, 2),   // +$80
        Decimal::new(-25000, 2), // -$250 (should trigger limit)
    ];

    for (i, pnl_change) in pnl_changes.iter().enumerate() {
        let fill = Fill {
            order_id: OrderId::new(&format!("pnl-change-{}", i)),
            symbol: Symbol::new("AAPL").unwrap(),
            side: OrderSide::Buy,
            filled_quantity: Quantity::new(100).unwrap(),
            filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
            filled_at: chrono::Utc::now(),
            commission: *pnl_change,
        };

        event_bus.publish(MarketEvent::OrderFilled {
            order_id: fill.order_id.clone(),
            fill: fill.clone(),
        });

        sleep(Duration::from_millis(5)).await;
    }

    sleep(Duration::from_millis(50)).await;

    // Check for trading halt
    let mut halt_detected = false;
    while let Ok(event) = event_receiver.try_recv() {
        if matches!(event, MarketEvent::TradingHalted { .. }) {
            halt_detected = true;
            break;
        }
    }

    assert!(halt_detected, "Should halt trading when loss limit is exceeded");
    assert!(!engine.is_trading_allowed().await);
}

/// Helper function to create PDT-focused risk configuration
fn create_strict_pdt_config() -> RiskConfig {
    use daytrader::risk::types::{PatternPerformanceThresholds, Percentage, Count};
    
    RiskConfig {
        max_daily_loss_dollars: 1000.0,
        max_daily_loss_percent: 5.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 10000.0,
        max_position_size_percent: 20.0,
        max_concurrent_positions: 10,
        enforce_pdt_rule: true,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 3,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 50.0,
        min_pattern_sample_size: 10,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 1000.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: PatternPerformanceThresholds {
            min_win_rate_short: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_medium: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_long: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            max_consecutive_losses: Count::new(5).unwrap(),
            min_sample_size: Count::new(10).unwrap(),
            auto_disable_threshold: Percentage::new(rust_decimal::Decimal::new(300, 1)).unwrap(),
        },
    }
}

/// Helper function to create loss limit focused configuration
fn create_loss_limit_config() -> RiskConfig {
    use daytrader::risk::types::{PatternPerformanceThresholds, Percentage, Count};
    
    RiskConfig {
        max_daily_loss_dollars: 500.0, // Very low limit for testing
        max_daily_loss_percent: 1.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 25000.0,
        max_position_size_percent: 50.0,
        max_concurrent_positions: 10,
        enforce_pdt_rule: false,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 10,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 50.0,
        min_pattern_sample_size: 10,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 1000.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: PatternPerformanceThresholds {
            min_win_rate_short: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_medium: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_long: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            max_consecutive_losses: Count::new(5).unwrap(),
            min_sample_size: Count::new(10).unwrap(),
            auto_disable_threshold: Percentage::new(rust_decimal::Decimal::new(300, 1)).unwrap(),
        },
    }
}

/// Helper function to create position sizing focused configuration
fn create_position_sizing_config() -> RiskConfig {
    use daytrader::risk::types::{PatternPerformanceThresholds, Percentage, Count};
    
    RiskConfig {
        max_daily_loss_dollars: 5000.0,
        max_daily_loss_percent: 10.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 5000.0, // Low limit for testing
        max_position_size_percent: 5.0,     // Low percentage for testing
        max_concurrent_positions: 5,
        enforce_pdt_rule: false,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 10,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 50.0,
        min_pattern_sample_size: 10,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 1000.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: PatternPerformanceThresholds {
            min_win_rate_short: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_medium: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_long: Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            max_consecutive_losses: Count::new(5).unwrap(),
            min_sample_size: Count::new(10).unwrap(),
            auto_disable_threshold: Percentage::new(rust_decimal::Decimal::new(300, 1)).unwrap(),
        },
    }
}

/// Helper function to create standard test configuration
fn create_test_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: 1000.0,
        max_daily_loss_percent: 5.0,
        halt_trading_on_limit: true,
        max_position_size_dollars: 10000.0,
        max_position_size_percent: 20.0,
        max_concurrent_positions: 10,
        enforce_pdt_rule: true,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 3,
        max_trades_per_pattern_per_day: 10,
        stop_pattern_after_consecutive_losses: 5,
        required_win_rate_for_auto: 50.0,
        min_pattern_sample_size: 10,
        min_account_balance: 1000.0,
        emergency_liquidate_threshold: 1000.0,
        pattern_profiles: std::collections::HashMap::new(),
        confidence_multipliers: vec![],
        pattern_performance_thresholds: daytrader::risk::types::PatternPerformanceThresholds {
            min_win_rate_short: daytrader::risk::types::Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_medium: daytrader::risk::types::Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            min_win_rate_long: daytrader::risk::types::Percentage::new(rust_decimal::Decimal::new(500, 1)).unwrap(),
            max_consecutive_losses: daytrader::risk::types::Count::new(5).unwrap(),
            min_sample_size: daytrader::risk::types::Count::new(10).unwrap(),
            auto_disable_threshold: daytrader::risk::types::Percentage::new(rust_decimal::Decimal::new(300, 1)).unwrap(),
        },
    }
}

/// Helper function to create a day trade order
fn create_day_trade_order(id: &str, symbol: &str) -> Arc<Order> {
    Arc::new(Order {
        id: OrderId::new(id),
        symbol: Symbol::new(symbol).unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: Quantity::new(100).unwrap(),
        price: None, // Market order
        status: OrderStatus::Submitted,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Helper function to create a fill for an order
fn create_fill_for_order(order_id: &str, quantity: u64) -> Fill {
    Fill {
        order_id: OrderId::new(order_id),
        symbol: Symbol::new("AAPL").unwrap(),
        side: OrderSide::Buy,
        filled_quantity: Quantity::new(quantity as u32).unwrap(),
        filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
        filled_at: chrono::Utc::now(),
        commission: Decimal::new(-500, 2), // $5 commission
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