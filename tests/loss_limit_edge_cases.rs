//! Edge case tests for daily loss limit enforcement
//!
//! These tests focus on boundary conditions in loss limit calculations:
//! - Floating-point precision issues in P&L calculations
//! - Rapid P&L changes and limit enforcement timing
//! - Daily reset at exact market open time (9:30 AM ET)
//! - Risk evaluation with partial order fills

use daytrader::risk::limits::{DailyLossLimits, DailyLossTracker};
use daytrader::risk::types::{DollarAmount, Percentage};
use rust_decimal::Decimal;
use chrono::{DateTime, NaiveTime, TimeZone, Utc};

/// Test floating-point precision issues in P&L calculations
#[tokio::test]
async fn test_floating_point_precision_edge_cases() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(), // $500.00
        max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),     // 2.00%
    };

    let tracker = DailyLossTracker::new(limits);

    // Test with very small decimal amounts that could cause precision issues
    let tiny_loss = Decimal::new(1, 4); // $0.0001
    let evaluation = tracker.update_daily_pnl(-tiny_loss).await;
    assert!(evaluation.is_passed());

    // Accumulate many tiny losses
    for _ in 0..10000 {
        let evaluation = tracker.update_daily_pnl(-tiny_loss).await;
        assert!(evaluation.is_passed()); // Should not trigger limit
    }

    let state = tracker.get_daily_state().await;
    let expected_loss = tiny_loss * Decimal::from(10000);
    assert_eq!(state.realized_pl, -expected_loss);

    // Test boundary at exactly the limit
    let exact_limit_loss = Decimal::new(50000, 2); // $500.00
    let evaluation = tracker.update_daily_pnl(-exact_limit_loss).await;

    match evaluation {
        daytrader::risk::types::RiskEvaluation::Violated { .. } => {
            // Should violate at exactly the limit
        }
        _ => panic!("Expected violation at exact limit"),
    }
}

/// Test rapid P&L changes and limit enforcement timing
#[tokio::test]
async fn test_rapid_pnl_changes() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(10000, 2)).unwrap(), // $100.00
        max_loss_percent: Percentage::new(Decimal::new(100, 2)).unwrap(),     // 1.00%
    };

    let tracker = DailyLossTracker::new(limits);

    // Simulate rapid alternating gains and losses
    let changes = vec![
        Decimal::new(-2500, 2), // -$25.00
        Decimal::new(1500, 2),  // +$15.00
        Decimal::new(-3000, 2), // -$30.00
        Decimal::new(1000, 2),  // +$10.00
        Decimal::new(-2500, 2), // -$25.00 (should trigger limit)
    ];

    let mut violation_triggered = false;

    for change in changes {
        let evaluation = tracker.update_daily_pnl(change).await;

        if matches!(evaluation, daytrader::risk::types::RiskEvaluation::Violated { .. }) {
            violation_triggered = true;
            break;
        }
    }

    assert!(violation_triggered, "Should trigger violation with accumulated losses");

    // After violation, should be halted
    assert!(tracker.should_halt_trading().await);

    // Further losses should still be blocked
    let result = tracker.check_daily_loss_limit(Decimal::new(-100, 0)).await;
    assert!(result.is_err());
}

/// Test daily reset timing at exact market open time
#[tokio::test]
async fn test_daily_reset_timing() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
        max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    };

    let tracker = DailyLossTracker::new(limits);

    // Add some losses
    tracker.update_daily_pnl(Decimal::new(-10000, 2)).await;

    let state_before = tracker.get_daily_state().await;
    assert_eq!(state_before.realized_pl, Decimal::new(-10000, 2));

    // Test reset timing - should reset at exactly 9:30 AM ET (14:30 UTC)
    let market_open_utc = Utc.with_ymd_and_hms(2023, 12, 15, 14, 30, 0).unwrap();
    let should_reset = tracker.should_reset_daily(market_open_utc);
    assert!(should_reset, "Should reset at market open time");

    // One second before market open - should not reset
    let before_open = Utc.with_ymd_and_hms(2023, 12, 15, 14, 29, 59).unwrap();
    let should_not_reset = tracker.should_reset_daily(before_open);
    assert!(!should_not_reset, "Should not reset before market open");

    // One second after market open - should not reset (exact time only)
    let after_open = Utc.with_ymd_and_hms(2023, 12, 15, 14, 30, 1).unwrap();
    let should_not_reset_after = tracker.should_reset_daily(after_open);
    assert!(!should_not_reset_after, "Should not reset after market open");

    // Perform actual reset
    tracker.reset_daily_limits(Decimal::new(1000000, 2)).await;

    let state_after = tracker.get_daily_state().await;
    assert_eq!(state_after.realized_pl, Decimal::ZERO);
    assert_eq!(state_after.unrealized_pl, Decimal::ZERO);
    assert!(!state_after.is_halted);
    assert_eq!(state_after.starting_balance, Decimal::new(1000000, 2));
}

/// Test partial fills and incremental P&L updates
#[tokio::test]
async fn test_partial_fills_risk_evaluation() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(20000, 2)).unwrap(), // $200.00
        max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),     // 2.00%
    };

    let tracker = DailyLossTracker::new(limits);

    // Simulate partial fills of a large order
    let total_order_quantity = Decimal::new(1000, 0);
    let order_price = Decimal::new(15000, 2); // $150.00
    let total_order_value = total_order_quantity * order_price;

    // Partial fills at different prices (simulating market movement)
    let partial_fills = vec![
        (Decimal::new(200, 0), Decimal::new(14900, 2)), // 200 shares @ $149.00
        (Decimal::new(300, 0), Decimal::new(14800, 2)), // 300 shares @ $148.00
        (Decimal::new(250, 0), Decimal::new(14700, 2)), // 250 shares @ $147.00
        (Decimal::new(250, 0), Decimal::new(14600, 2)), // 250 shares @ $146.00
    ];

    let mut total_realized_pl = Decimal::ZERO;

    for (quantity, fill_price) in partial_fills {
        // Calculate P&L for this fill (assuming we bought at order_price and sold at fill_price)
        let fill_pl = (fill_price - order_price) * quantity;
        total_realized_pl += fill_pl;

        // Check if this fill would violate limits before applying
        let can_fill = tracker.check_daily_loss_limit(fill_pl).await.unwrap();
        assert!(can_fill, "Partial fill should be allowed within limits");

        // Apply the fill
        tracker.update_daily_pnl(fill_pl).await;
    }

    // Verify accumulated P&L
    let state = tracker.get_daily_state().await;
    assert_eq!(state.realized_pl, total_realized_pl);

    // Should be negative (losses)
    assert!(total_realized_pl < Decimal::ZERO);

    // But should not have triggered halt
    assert!(!tracker.should_halt_trading().await);
}

/// Test boundary conditions at exactly the loss limits
#[tokio::test]
async fn test_loss_limit_boundary_conditions() {
    // Test with very tight limits to test boundaries
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(1000, 2)).unwrap(), // $10.00
        max_loss_percent: Percentage::new(Decimal::new(10, 1)).unwrap(),     // 1.0%
    };

    let tracker = DailyLossTracker::new(limits);

    // Set starting balance
    tracker.reset_daily_limits(Decimal::new(1000000, 2)).await; // $10,000.00

    // Test dollar limit boundary
    let dollar_limit = Decimal::new(1000, 2); // Exactly $10.00

    // Just under limit should be allowed
    let can_trade_under = tracker.check_daily_loss_limit(-dollar_limit + Decimal::new(1, 2)).await.unwrap();
    assert!(can_trade_under);

    // Exactly at limit should be blocked
    let can_trade_at = tracker.check_daily_loss_limit(-dollar_limit).await.unwrap();
    assert!(!can_trade_at);

    // Test percentage limit boundary (1% of $10,000 = $100)
    let percent_limit = Decimal::new(10000, 2); // $100.00

    // Just under percent limit should be allowed
    let can_trade_percent_under = tracker.check_daily_loss_limit(-percent_limit + Decimal::new(1, 2)).await.unwrap();
    assert!(can_trade_percent_under);

    // Exactly at percent limit should be blocked
    let can_trade_percent_at = tracker.check_daily_loss_limit(-percent_limit).await.unwrap();
    assert!(!can_trade_percent_at);
}

/// Test unrealized P&L tracking with position updates
#[tokio::test]
async fn test_unrealized_pnl_tracking() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
        max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    };

    let tracker = DailyLossTracker::new(limits);

    // Simulate position with changing unrealized P&L
    let unrealized_changes = vec![
        Decimal::new(-5000, 2),  // -$50.00
        Decimal::new(-2000, 2),  // Additional -$20.00
        Decimal::new(8000, 2),   // +$80.00 (recovered some)
        Decimal::new(-3000, 2),  // -$30.00
        Decimal::new(15000, 2),  // +$150.00 (now in profit)
    ];

    for change in unrealized_changes {
        let evaluation = tracker.update_unrealized_pnl(change).await;
        assert!(evaluation.is_passed()); // Should not violate with these amounts
    }

    let state = tracker.get_daily_state().await;
    let expected_unrealized = Decimal::new(2000, 2); // Sum of all changes
    assert_eq!(state.unrealized_pl, expected_unrealized);

    // Total P&L should be unrealized only (no realized P&L)
    assert_eq!(state.total_pl(), expected_unrealized);
}

/// Test mixed realized and unrealized P&L interactions
#[tokio::test]
async fn test_mixed_realized_unrealized_pnl() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(30000, 2)).unwrap(), // $300.00
        max_loss_percent: Percentage::new(Decimal::new(300, 2)).unwrap(),     // 3.00%
    };

    let tracker = DailyLossTracker::new(limits);
    tracker.reset_daily_limits(Decimal::new(1000000, 2)).await; // $10,000.00

    // Add realized losses
    tracker.update_daily_pnl(Decimal::new(-10000, 2)).await; // -$100.00

    // Add unrealized losses
    tracker.update_unrealized_pnl(Decimal::new(-5000, 2)).await; // Additional -$50.00

    let state = tracker.get_daily_state().await;
    assert_eq!(state.realized_pl, Decimal::new(-10000, 2));
    assert_eq!(state.unrealized_pl, Decimal::new(-5000, 2));
    assert_eq!(state.total_pl(), Decimal::new(-15000, 2)); // -$150.00 total

    // Should still be allowed to trade (under $300 limit)
    let can_trade = tracker.check_daily_loss_limit(Decimal::new(-5000, 2)).await.unwrap();
    assert!(can_trade);

    // Add more losses to approach limit
    tracker.update_daily_pnl(Decimal::new(-14000, 2)).await; // Additional -$140.00

    let state = tracker.get_daily_state().await;
    assert_eq!(state.total_pl(), Decimal::new(-29000, 2)); // -$290.00 total

    // Should still be just under limit
    let can_trade_still = tracker.check_daily_loss_limit(Decimal::new(-900, 2)).await.unwrap();
    assert!(can_trade_still);

    // One more dollar should push over limit
    let can_trade_over = tracker.check_daily_loss_limit(Decimal::new(-1100, 2)).await.unwrap();
    assert!(!can_trade_over);
}

/// Test concurrent access to loss tracker state
#[tokio::test]
async fn test_concurrent_loss_tracking() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(100000, 2)).unwrap(),
        max_loss_percent: Percentage::new(Decimal::new(500, 2)).unwrap(),
    };

    let tracker = DailyLossTracker::new(limits);

    // Spawn multiple tasks updating P&L concurrently
    let mut handles = vec![];

    for i in 0..10 {
        let tracker_clone = std::sync::Arc::new(tracker.clone());
        let handle = tokio::spawn(async move {
            for j in 0..5 {
                let pnl_change = Decimal::new(-100 * (i + j) as i64, 2);
                let _ = tracker_clone.update_daily_pnl(pnl_change).await;
                let _ = tracker_clone.check_daily_loss_limit(Decimal::new(-50, 0)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    for handle in handles {
        handle.await.unwrap();
    }

    // System should still be functional and consistent
    let state = tracker.get_daily_state().await;
    assert!(state.realized_pl <= Decimal::ZERO); // Should have accumulated losses

    // Should not have triggered halt (limits are high enough)
    assert!(!tracker.should_halt_trading().await);
}

/// Test precision edge cases with very small balances
#[tokio::test]
async fn test_small_balance_precision() {
    let limits = DailyLossLimits {
        max_loss_dollars: DollarAmount::new(Decimal::new(100, 2)).unwrap(), // $1.00
        max_loss_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),  // 10.00%
    };

    let tracker = DailyLossTracker::new(limits);
    tracker.reset_daily_limits(Decimal::new(1000, 2)).await; // $10.00 balance

    // Percentage limit = 10% of $10.00 = $1.00 (same as dollar limit)

    // Very small loss that rounds differently
    let tiny_loss = Decimal::new(1, 3); // $0.001
    let evaluation = tracker.update_daily_pnl(-tiny_loss).await;
    assert!(evaluation.is_passed());

    // Accumulate tiny losses
    for _ in 0..999 {
        let evaluation = tracker.update_daily_pnl(-tiny_loss).await;
        assert!(evaluation.is_passed());
    }

    // Should now be very close to limit
    let state = tracker.get_daily_state().await;
    let accumulated_loss = tiny_loss * Decimal::from(999);
    assert_eq!(state.realized_pl, -accumulated_loss);

    // Next tiny loss should not trigger violation yet
    let evaluation = tracker.update_daily_pnl(-tiny_loss).await;
    assert!(evaluation.is_passed());

    // But a larger loss should trigger
    let evaluation = tracker.update_daily_pnl(Decimal::new(-50, 2)).await;
    match evaluation {
        daytrader::risk::types::RiskEvaluation::Violated { .. } => {
            // Should violate
        }
        _ => panic!("Expected violation"),
    }
}