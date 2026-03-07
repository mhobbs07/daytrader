//! Risk management mock database integration tests
//!
//! Tests the risk management workflow with mock database to avoid compilation issues.

use daytrader::config::RiskConfig;
use daytrader::risk::pdt::PdtConfig;
use daytrader::risk::pdt::PdtEnforcer;
use rust_decimal::Decimal;

/// Test PDT enforcer with mock database (no actual database operations)
#[tokio::test]
async fn test_pdt_enforcer_mock_integration() {
    let config = create_test_risk_config();
    let initial_equity = Decimal::new(2500000, 2); // $25,000.00 (PDT threshold)

    // Create PDT enforcer with mock database
    let pdt_config = PdtConfig::from_config(&config);
    let enforcer = PdtEnforcer::new_without_db(pdt_config, initial_equity);

    // Record some day trades
    let today = chrono::Utc::now().date_naive();
    enforcer.record_day_trade(today).await.unwrap();
    enforcer.record_day_trade(today).await.unwrap();

    // Verify day trade count
    let count = enforcer.count_day_trades().await;
    assert_eq!(count, 2);

    // Check PDT eligibility
    let eligible = enforcer.check_pdt_eligibility().await.unwrap();
    assert!(eligible); // Under limit

    // Record one more trade (at limit for PDT)
    enforcer.record_day_trade(today).await.unwrap();
    let count_after = enforcer.count_day_trades().await;
    assert_eq!(count_after, 3);

    // Should still be eligible (at limit)
    let eligible_after = enforcer.check_pdt_eligibility().await.unwrap();
    assert!(eligible_after);

    // Record fourth trade (over limit)
    enforcer.record_day_trade(today).await.unwrap();
    let result = enforcer.check_pdt_eligibility().await;
    assert!(result.is_err()); // Should be blocked
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
        pattern_performance_thresholds: daytrader::risk::types::PatternPerformanceThresholds::default(),
    }
}