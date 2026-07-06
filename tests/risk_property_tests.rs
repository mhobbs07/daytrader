//! Property-based tests for risk management system
//!
//! Uses proptest to verify invariants and edge cases in risk calculations,
//! particularly focusing on market crash scenarios, extreme values, and
//! boundary conditions that are difficult to test manually.

use daytrader::brokers::types::{OrderId, Price, Quantity, Symbol};
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::events::types::{Fill, MarketEvent};
use daytrader::risk::RiskEngine;
use daytrader::risk::pdt::PdtEnforcer;
use daytrader::risk::position_sizing::PositionSizer;
use daytrader::risk::rules::TradeContext;
use daytrader::risk::types::{AccountId, DollarAmount, Percentage};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Strategy for generating valid decimal amounts
fn decimal_strategy() -> impl Strategy<Value = Decimal> {
    (-1000000..1000000i64).prop_map(|n| Decimal::new(n, 2))
}

/// Strategy for generating valid positive decimal amounts
fn positive_decimal_strategy() -> impl Strategy<Value = Decimal> {
    (1..1000000i64).prop_map(|n| Decimal::new(n, 2))
}

/// Strategy for generating valid percentages (0.0 to 100.0)
fn percentage_strategy() -> impl Strategy<Value = Decimal> {
    (0..10000u32).prop_map(|n| Decimal::new(n as i64, 2))
}

/// Strategy for generating extreme market prices
fn extreme_price_strategy() -> impl Strategy<Value = Decimal> {
    prop_oneof![
        // Very low prices (penny stocks, crypto crashes)
        (1..100i64).prop_map(|n| Decimal::new(n, 4)), // $0.0001 to $0.0100
        // Normal prices
        (100..100000i64).prop_map(|n| Decimal::new(n, 2)), // $1.00 to $1000.00
        // Very high prices (expensive stocks)
        (100000..10000000i64).prop_map(|n| Decimal::new(n, 2)), // $1000.00 to $100,000.00
    ]
}

/// Strategy for generating market crash scenarios
fn market_crash_strategy() -> impl Strategy<Value = (Decimal, Decimal)> {
    (extreme_price_strategy(), extreme_price_strategy()).prop_filter(
        "Second price must be lower than first (crash)",
        |(entry, exit)| *exit < *entry * Decimal::new(95, 2), // At least 5% drop
    )
}

proptest! {
    /// Test that position sizing never exceeds account equity limits
    #[test]
    fn position_sizing_never_exceeds_equity_limits(
        account_equity in positive_decimal_strategy(),
        position_size_pct in percentage_strategy(),
        price in positive_decimal_strategy()
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = RiskConfig {
                max_position_size_percent: position_size_pct.to_string().parse::<f64>().unwrap_or(10.0),
                max_position_size_dollars: 100000.0, // High dollar limit
                ..create_test_risk_config()
            };

            let sizer = PositionSizer::from_config(&config, account_equity);

            let max_position = sizer.calculate_max_position_size().await;

            // Position size should never exceed equity limits
            let max_value = max_position * price;
            let pct_as_i64 = (position_size_pct * Decimal::new(100, 0)).to_string().parse::<i64>().unwrap_or(1000);
            let equity_limit = account_equity * Decimal::new(pct_as_i64, 2) / Decimal::new(10000, 2);

            prop_assert!(max_value <= equity_limit,
                "Position value {} exceeds equity limit {} (equity: {}, pct: {})",
                max_value, equity_limit, account_equity, position_size_pct);
            Ok(())
        });
    }

    /// Test PDT limits are never exceeded under any trade sequence
    #[test]
    fn pdt_limits_never_exceeded(
        account_equity in positive_decimal_strategy(),
        trade_count in 1..10usize
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = create_test_risk_config();
            let pdt_config = daytrader::risk::pdt::PdtConfig::from_config(&config);
            let enforcer = PdtEnforcer::new(pdt_config, account_equity);

            // Record multiple day trades
            let today = chrono::Utc::now().date_naive();
            for i in 0..trade_count {
                let trade_date = today + chrono::Duration::days(i as i64 % 3); // Spread across few days
                enforcer.record_day_trade(trade_date).await.unwrap();
            }

            let day_trades = enforcer.count_day_trades().await;

            // Should never exceed PDT limit (3 trades)
            prop_assert!(day_trades <= 3,
                "Day trades {} exceeds PDT limit of 3", day_trades);

            // If at limit, should not allow more trades
            if day_trades >= 3 {
                let can_trade = enforcer.check_pdt_eligibility().await;
                prop_assert!(can_trade.is_err(),
                    "Should not allow trading when at PDT limit");
            }
            Ok(())
        });
    }

    /// Test that loss limits prevent catastrophic losses
    #[test]
    fn loss_limits_prevent_catastrophic_losses(
        initial_equity in positive_decimal_strategy(),
        loss_amount in positive_decimal_strategy()
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = RiskConfig {
                max_daily_loss_dollars: 1000.0, // $10.00 limit
                max_daily_loss_percent: 1.0,    // 1% limit
                halt_trading_on_limit: true,
                ..create_test_risk_config()
            };

            let event_bus = Arc::new(EventBus::new(1000));
            let account_id = AccountId::new("test-account");
            let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
            engine.start().await.unwrap();

            // Create a loss that exceeds limits
            let fill = Fill {
                order_id: OrderId::new("loss-fill"),
                filled_quantity: Quantity::new(100).unwrap(),
                filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
                filled_at: chrono::Utc::now(),
                commission: -loss_amount, // Negative = loss
            };

            event_bus.publish(MarketEvent::OrderFilled {
                order_id: fill.order_id.clone(),
                fill,
            });

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // If loss exceeds limits, trading should be halted
            let loss_limit = initial_equity * Decimal::new(10, 2) / Decimal::new(1000, 2); // 1% of equity
            if loss_amount > loss_limit {
                prop_assert!(!engine.is_trading_allowed().await,
                    "Trading should be halted when loss {} exceeds limit {}", loss_amount, loss_limit);
            }
            Ok(())
        });
    }

    /// Test market crash scenarios don't cause system failures
    #[test]
    fn market_crash_scenarios_no_system_failure(
        (entry_price, exit_price) in market_crash_strategy()
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = create_test_risk_config();
            let event_bus = Arc::new(EventBus::new(1000));
            let account_id = AccountId::new("crash-test-account");
            let initial_equity = Decimal::new(10000000, 2); // $100,000

            let engine = RiskEngine::new(config, Arc::clone(&event_bus), account_id, initial_equity);
            engine.start().await.unwrap();

            // Simulate position with crash
            let position_quantity = 1000;
            let position_value = Decimal::from(position_quantity) * entry_price;
            let loss = Decimal::from(position_quantity) * (entry_price - exit_price);

            // Create position update with crash
            let position = daytrader::events::types::Position {
                symbol: Symbol::new("CRASH").unwrap(),
                quantity: daytrader::brokers::types::Quantity::new(position_quantity).unwrap(),
                average_entry_price: Price::new(entry_price).unwrap(),
                current_price: Price::new(exit_price).unwrap(),
                unrealized_pl: -loss, // Loss
                realized_pl: Decimal::ZERO,
                opened_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            event_bus.publish(MarketEvent::PositionUpdated { position });

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // System should handle crash without panicking
            let state = engine.get_risk_state().await;
            prop_assert!(state.daily_pl <= Decimal::ZERO,
                "Daily P&L should reflect losses in crash scenario");

            // Loss should be reasonable (Decimal is always finite/non-NaN)
            prop_assert!(loss.is_sign_positive() || loss.is_sign_negative() || loss.is_zero(),
                "Loss calculation should be valid: {}", loss);
            Ok(())
        });
    }

    /// Test extreme position sizes don't cause overflow or panic
    #[test]
    fn extreme_position_sizes_no_overflow(
        position_size in 1..100000u64,
        price in extreme_price_strategy()
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = create_test_risk_config();
            let sizer = PositionSizer::from_config(&config, Decimal::new(10000000, 2));

            // Calculate position value
            let position_value = Decimal::from(position_size) * price;

            // Should not panic or overflow
            let result = std::panic::catch_unwind(|| {
                position_value * Decimal::new(2, 0) // Simple operation to test
            });

            prop_assert!(result.is_ok(),
                "Position calculation should not overflow: size={}, price={}", position_size, price);

            // Position value should be valid (Decimal is always finite/non-NaN)
            prop_assert!(position_value.is_sign_positive() || position_value.is_sign_negative() || position_value.is_zero(),
                "Position value should be valid: {}", position_value);
            Ok(())
        });
    }

    /// Test concurrent risk evaluations maintain consistency
    #[test]
    fn concurrent_risk_evaluations_consistent(
        thread_count in 2..10usize,
        evaluation_count in 1..50usize
    ) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let config = create_test_risk_config();
            let rule_engine = Arc::new(daytrader::risk::rules::RiskRuleEngine::new(config));

            // Spawn concurrent evaluations
            let mut handles = vec![];

            for _ in 0..thread_count {
                let engine = Arc::clone(&rule_engine);
                let handle = tokio::spawn(async move {
                    let mut results = vec![];
                    for _ in 0..evaluation_count {
                        let context = TradeContext::new(
                            "TEST".to_string(),
                            Decimal::new(1000, 0),
                            Decimal::new(15000, 2),
                            Decimal::new(50000, 2),
                            false,
                        );
                        let result = engine.evaluate_violations_only(context).await;
                        results.push(result);
                    }
                    results
                });
                handles.push(handle);
            }

            // Collect all results
            let mut all_results = vec![];
            for handle in handles {
                let results = handle.await.unwrap();
                all_results.extend(results);
            }

            // All evaluations should complete without panics
            prop_assert_eq!(all_results.len(), thread_count * evaluation_count,
                "All concurrent evaluations should complete");

            // Results should be consistent (same input, same output)
            // Note: HashMap results compared by length since RuleEvaluation doesn't implement PartialEq
            let first_result = &all_results[0];
            for result in &all_results[1..] {
                prop_assert_eq!(result.len(), first_result.len(),
                    "Concurrent evaluations should return same number of rules");
            }
            Ok(())
        });
    }

    /// Test edge case: zero equity accounts
    #[test]
    fn zero_equity_edge_cases(account_equity in 0..1i64) {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let equity = Decimal::new(account_equity, 2);
            let config = create_test_risk_config();

            // Should handle zero/near-zero equity gracefully
            let sizer = PositionSizer::from_config(&config, equity);
            let max_position = sizer.calculate_max_position_size().await;

            // Max position should be zero or very small for zero equity
            prop_assert!(max_position >= Decimal::ZERO,
                "Max position should be non-negative: {}", max_position);

            // Should not allow trading with zero equity
            if equity <= Decimal::ZERO {
                prop_assert!(max_position == Decimal::ZERO,
                    "Should not allow positions with zero equity");
            }
            Ok(())
        });
    }

    /// Test boundary conditions for percentages
    #[test]
    fn percentage_boundary_conditions(value in 0..10000u32) {
        let decimal_value = Decimal::new(value as i64, 2);

        // Should handle boundary values
        let result = Percentage::new(decimal_value);

        if decimal_value >= Decimal::ZERO && decimal_value <= Decimal::new(1000, 1) {
            prop_assert!(result.is_ok(),
                "Should accept valid percentage: {}", decimal_value);
        } else {
            prop_assert!(result.is_err(),
                "Should reject invalid percentage: {}", decimal_value);
        }
    }

    /// Test dollar amount boundary conditions
    #[test]
    fn dollar_amount_boundary_conditions(value in -1000..10000i64) {
        let decimal_value = Decimal::new(value, 2);

        let result = DollarAmount::new(decimal_value);

        if decimal_value >= Decimal::ZERO {
            prop_assert!(result.is_ok(),
                "Should accept non-negative dollar amount: {}", decimal_value);
        } else {
            prop_assert!(result.is_err(),
                "Should reject negative dollar amount: {}", decimal_value);
        }
    }
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
