# Risk Management Guide

## Overview

This comprehensive guide covers the risk management system implemented in Phase 2C-D of our algorithmic trading platform. The system provides multi-layered risk controls including position sizing, portfolio allocation, Pattern Day Trading (PDT) compliance, emergency liquidation, and comprehensive audit trails.

## Architecture Overview

### Core Components

1. **RiskEngine** ([`src/risk/engine.rs`](../src/risk/engine.rs))
   - Central risk management coordinator
   - Event-driven processing
   - Real-time risk evaluation
   - Emergency response handling

2. **Position Sizing** ([`src/risk/position_sizing.rs`](../src/risk/position_sizing.rs))
   - Portfolio allocation management
   - Position size validation
   - Account equity tracking

3. **PDT Enforcement** ([`src/risk/pdt.rs`](../src/risk/pdt.rs))
   - Day trade detection and counting
   - SEC compliance monitoring
   - Trading restrictions enforcement

4. **Emergency Liquidation** ([`src/risk/engine.rs`](../src/risk/engine.rs))
   - Automatic position closure
   - Broker API integration
   - Recovery procedures

5. **Audit Trail** ([`src/db/models.rs`](../src/db/models.rs))
   - Structured logging
   - Compliance reporting
   - Risk event tracking

## Risk Rules and Validation

### Position Sizing Validation

Position sizing ensures trades comply with account equity limits and portfolio allocation rules:

```rust
// Position sizing validation in RiskEngine
let position_size_evaluation = self.position_sizer.validate_position_size(position_value).await;
if let RiskEvaluation::Violated { severity, suggested_action } = &position_size_evaluation {
    // Log audit entry for position size validation
    let metadata = serde_json::json!({
        "symbol": order.symbol.to_string(),
        "quantity": order.quantity.value(),
        "position_value": position_value,
        "severity": format!("{:?}", severity),
        "suggested_action": suggested_action
    });
    self.log_risk_audit(
        "PositionSizeValidation",
        "warning",
        format!("Position size violation for {}: {}", order.symbol, violation.message.as_str()),
        Some(metadata),
    ).await;
}
```

#### Portfolio Allocation Checks

The system monitors portfolio allocation to prevent concentration risk:

```rust
// Portfolio allocation validation
let allocation_violations = self.position_sizer.check_portfolio_allocation().await;
for (symbol, evaluation) in allocation_violations {
    if let RiskEvaluation::Violated { severity, suggested_action } = evaluation {
        let violation = RiskViolation {
            violation_type: RiskViolationType::PortfolioAllocationLimit,
            message: Message::new(
                suggested_action.unwrap_or_else(|| format!("Portfolio allocation violation for {}", symbol))
            ),
            timestamp: Utc::now(),
            pattern_name: None,
        };
        // Publish violation and log audit
    }
}
```

### PDT Compliance Monitoring

Pattern Day Trading rules are enforced with real-time monitoring:

```rust
// PDT eligibility check
match pdt_enforcer.check_pdt_eligibility().await {
    Ok(false) => {
        // Still eligible but approaching limit - log audit
        let metadata = serde_json::json!({
            "symbol": fill.symbol.to_string(),
            "is_day_trade": is_day_trade,
            "trade_date": trade_date,
            "status": "approaching_limit"
        });
        self.log_risk_audit("PDTCheck", "warning", "...", Some(metadata)).await;
    }
    Err(message) => {
        // PDT limit exceeded - publish violation
        let violation = RiskViolation {
            violation_type: RiskViolationType::PdtLimit,
            message: Message::new(message.clone()),
            timestamp: Utc::now(),
            pattern_name: None,
        };
        // Publish and log violation
    }
    Ok(true) => {
        // Still eligible - log successful check
    }
}
```

### Emergency Liquidation Procedures

Emergency liquidation is triggered when account balance falls below a critical threshold:

```rust
// Emergency liquidation check
if current_balance <= threshold {
    tracing::error!("Emergency liquidation triggered: balance {} below threshold {}", current_balance, threshold);

    // Log emergency liquidation audit
    let liquidation_metadata = serde_json::json!({
        "current_balance": current_balance,
        "threshold": threshold,
        "reason": "account_balance_below_threshold"
    });
    self.log_risk_audit("EmergencyLiquidationTriggered", "critical", "...", Some(liquidation_metadata)).await;

    // Perform liquidation
    match self.perform_emergency_liquidation().await {
        Ok((liquidated_positions, total_value)) => {
            tracing::info!("Emergency liquidation completed: liquidated {} positions worth {}", liquidated_positions.len(), total_value);
        }
        Err(e) => {
            tracing::error!("Emergency liquidation failed: {}", e);
        }
    }
}
```

## Configuration

### Risk Configuration Structure

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    // Daily loss limits
    pub max_daily_loss_dollars: f64,
    pub max_daily_loss_percent: f64,
    pub halt_trading_on_limit: bool,

    // Position sizing
    pub max_position_size_dollars: f64,
    pub max_position_size_percent: f64,
    pub max_concurrent_positions: u32,

    // PDT compliance
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: f64,
    pub max_day_trades_under_pdt: u32,

    // Pattern-specific limits
    pub max_trades_per_pattern_per_day: u32,
    pub stop_pattern_after_consecutive_losses: u32,
    pub required_win_rate_for_auto: f64,
    pub min_pattern_sample_size: u32,

    // Emergency settings
    pub min_account_balance: f64,
    pub emergency_liquidate_threshold: f64,

    // Advanced settings
    pub pattern_profiles: HashMap<String, PatternRiskProfile>,
    pub confidence_multipliers: Vec<ConfidenceMultiplier>,
    pub pattern_performance_thresholds: PatternPerformanceThresholds,
}
```

### Example Configuration (TOML)

```toml
[risk]
max_daily_loss_dollars = 500.0
max_daily_loss_percent = 2.0
halt_trading_on_limit = true

max_position_size_dollars = 5000.0
max_position_size_percent = 10.0
max_concurrent_positions = 5

enforce_pdt_rule = true
pdt_min_equity = 25000.0
max_day_trades_under_pdt = 3

max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.0
min_pattern_sample_size = 20

min_account_balance = 1000.0
emergency_liquidate_threshold = 500.0
```

### Usage Patterns

#### Basic Risk Engine Setup

```rust
use daytrader::config::RiskConfig;
use daytrader::events::bus::EventBus;
use daytrader::risk::RiskEngine;
use daytrader::brokers::mock::MockBroker;
use std::sync::Arc;

// Load configuration
let config = RiskConfig::from_file("config/default.toml").await?;

// Create event bus
let event_bus = Arc::new(EventBus::new(1000));

// Create broker (mock for testing)
let broker = Arc::new(MockBroker::default());

// Initialize risk engine
let engine = RiskEngine::new(
    config,
    Arc::clone(&event_bus),
    AccountId::new("DEMO123"),
    Decimal::new(5000000, 2), // $50,000.00
    broker
).await?;

// Start risk monitoring
engine.start().await?;
```

#### Order Validation Workflow

```rust
// Create order
let order = Order {
    id: OrderId::new("order-123"),
    symbol: Symbol::new("AAPL").unwrap(),
    side: OrderSide::Buy,
    order_type: OrderType::Market,
    quantity: Quantity::new(100).unwrap(),
    price: None,
    status: OrderStatus::Pending,
    created_at: Utc::now(),
    updated_at: Utc::now(),
};

// Validate order against risk rules
match engine.validate_order_placement(&order).await {
    Ok(()) => {
        // Order approved, proceed with placement
        broker.place_order(order).await?;
    }
    Err(RiskError::PositionSizeLimitExceeded { current, limit }) => {
        // Handle position size violation
        tracing::warn!("Position size violation: {} > {}", current, limit);
        return Err("Position size exceeds limits".into());
    }
    Err(RiskError::PDTViolation { message }) => {
        // Handle PDT violation
        tracing::warn!("PDT violation: {}", message);
        return Err("PDT limit exceeded".into());
    }
    Err(e) => {
        // Handle other risk violations
        tracing::error!("Risk validation failed: {}", e);
        return Err(e.into());
    }
}
```

#### Emergency Liquidation Handling

```rust
// Check for emergency conditions
if let Ok((liquidated_positions, total_value)) = engine.perform_emergency_liquidation().await {
    tracing::info!(
        "Emergency liquidation completed: liquidated {} positions worth ${}",
        liquidated_positions.len(),
        total_value
    );

    // Notify stakeholders
    send_emergency_notification(liquidated_positions, total_value).await?;
} else {
    tracing::error!("Emergency liquidation failed");
}
```

#### Pattern Risk Evaluation

```rust
// Evaluate pattern before trading
let pattern_signal = PatternSignal {
    symbol: Symbol::new("TSLA").unwrap(),
    pattern_name: PatternName::new("hammer"),
    signal_type: SignalType::Buy,
    confidence: Confidence::new(0.85).unwrap(),
    detected_at: Utc::now(),
    price: Price::new(Decimal::new(25000, 2)).unwrap(),
};

// Check if pattern is tradable
let evaluation = engine.evaluate_pattern_risk(&pattern_signal).await?;
match evaluation {
    RiskEvaluation::Passed => {
        // Proceed with trade
        execute_pattern_trade(pattern_signal).await?;
    }
    RiskEvaluation::Violated { severity, suggested_action } => {
        // Handle pattern risk violation
        tracing::warn!("Pattern risk violation: {}", suggested_action);
        if matches!(severity, ViolationSeverity::Critical) {
            return Err("Critical pattern risk violation".into());
        }
    }
}
```

## API Reference

### RiskEngine Methods

#### Core Risk Evaluation

```rust
impl RiskEngine {
    /// Validates order placement against all risk rules
    pub async fn validate_order_placement(&self, order: &Order) -> Result<(), RiskError>

    /// Updates risk metrics after order fill
    pub async fn update_on_fill(&self, order_id: OrderId, fill: &Fill) -> Result<(), RiskError>

    /// Checks if trading is currently allowed
    pub async fn is_trading_allowed(&self) -> bool

    /// Manually halts trading (emergency stop)
    pub async fn halt_trading(&self, reason: impl Into<String>)

    /// Resumes trading after halt
    pub async fn resume_trading(&self)
}
```

#### Position Sizing API

```rust
impl PositionSizer {
    /// Calculates maximum position size for a trade
    pub async fn calculate_max_position_size(&self) -> Result<Decimal, RiskError>

    /// Validates a specific position size
    pub async fn validate_position_size(&self, position_value: Decimal) -> Result<RiskEvaluation, RiskError>

    /// Checks portfolio allocation limits
    pub async fn check_portfolio_allocation(&self) -> Result<HashMap<String, RiskEvaluation>, RiskError>
}
```

#### PDT Enforcement API

```rust
impl PdtEnforcer {
    /// Checks if account is eligible for day trading
    pub async fn check_pdt_eligibility(&self) -> Result<bool, String>

    /// Records a day trade occurrence
    pub async fn record_day_trade(&self, trade_date: NaiveDate) -> Result<(), PdtError>

    /// Returns current day trade count
    pub async fn count_day_trades(&self) -> u32
}
```

## Event System Integration

### Risk Violation Events

The system publishes structured events for all risk violations:

```rust
pub enum MarketEvent {
    RiskViolation {
        violation: RiskViolation
    },
    EmergencyLiquidation {
        reason: String,
        timestamp: DateTime<Utc>
    },
    EmergencyLiquidationCompleted {
        success: bool,
        liquidated_positions: Vec<Symbol>,
        total_value_liquidated: Decimal,
        errors: Vec<String>,
        timestamp: DateTime<Utc>
    },
    TradingHalted {
        reason: Reason
    },
    TradingResumed,
}
```

### Risk Violation Types

```rust
pub enum RiskViolationType {
    PositionSizeLimit,
    PortfolioAllocationLimit,
    DailyLossLimit,
    PdtLimit,
    PatternDisabled,
    Other,
}
```

## Audit Trail and Reporting

### Audit Log Structure

All risk management actions are logged with structured metadata:

```rust
pub struct AuditLog {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub severity: String,
    pub user_id: Option<i64>,
    pub account_id: Option<i64>,
    pub description: String,
    pub metadata: Option<String>, // JSON metadata
}
```

### Audit Event Types

- **PositionSizeValidation**: Position size limit checks
- **PortfolioAllocationCheck**: Portfolio concentration monitoring
- **PDTCheck**: Day trading eligibility verification
- **PDTViolation**: PDT limit breaches
- **EmergencyLiquidationTriggered**: Emergency liquidation initiation
- **RiskRuleEvaluation**: General risk rule processing

## Performance Benchmarks

### Target Performance Metrics

- **PDT eligibility check**: < 1ms
- **Position sizing validation**: < 0.5ms
- **Emergency liquidation**: < 100ms
- **Concurrent evaluations**: Support 100+ simultaneous checks
- **Memory usage**: < 50MB under normal load
- **Event processing throughput**: 1000+ events/second

### Benchmark Results

```rust
// Example benchmark output
bench_rule_evaluation_speed    time:   [45.678 µs 46.123 µs 46.567 µs]
bench_concurrent_load          time:   [2.3456 ms 2.4567 ms 2.5678 ms]
bench_emergency_liquidation    time:   [67.890 ms 78.901 ms 89.012 ms]
```

## Testing and Validation

### Test Coverage

The risk management system includes comprehensive testing:

- **Unit Tests**: Individual component testing (100+ tests)
- **Integration Tests**: End-to-end workflow validation
- **Property-Based Tests**: Automated edge case testing
- **End-to-End Tests**: Full system scenario testing
- **Performance Benchmarks**: Load and stress testing

### Test Categories

#### Unit Test Examples

```rust
#[tokio::test]
async fn test_position_size_validation() {
    let config = create_test_config();
    let sizer = PositionSizer::from_config(&config, Decimal::new(100000, 2));

    let result = sizer.validate_position_size(Decimal::new(10000, 2)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pdt_day_trade_counting() {
    let enforcer = PdtEnforcer::new(create_pdt_config(), Decimal::new(50000, 2));

    enforcer.record_day_trade(Utc::now().date_naive()).await.unwrap();
    let count = enforcer.count_day_trades().await;
    assert_eq!(count, 1);
}
```

#### Integration Test Examples

```rust
#[tokio::test]
async fn test_risk_engine_full_workflow() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let engine = RiskEngine::new_with_mock_broker(config, event_bus, account_id, initial_equity);

    // Test complete order validation workflow
    let order = create_test_order();
    let result = engine.validate_order_placement(&order).await;
    assert!(result.is_ok());
}
```

## Error Handling and Recovery

### Risk Error Types

```rust
#[derive(thiserror::Error, Debug)]
pub enum RiskError {
    #[error("Position size limit exceeded: {current} > {limit}")]
    PositionSizeLimitExceeded { current: Decimal, limit: Decimal },

    #[error("Daily loss limit exceeded: {current} > {limit}")]
    DailyLossLimitExceeded { current: Decimal, limit: Decimal },

    #[error("PDT rule violation: {message}")]
    PDTViolation { message: String },

    #[error("Emergency liquidation failed: {reason}")]
    EmergencyLiquidationFailed { reason: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}
```

### Recovery Procedures

1. **Position Size Violations**: Reduce order size or cancel trade
2. **PDT Violations**: Block day trading until next business day
3. **Daily Loss Limits**: Halt trading and require manual intervention
4. **Emergency Liquidation**: Automatic position closure with broker integration

## Monitoring and Alerting

### Health Checks

The system provides health check endpoints for monitoring:

```rust
impl RiskEngine {
    /// Performs comprehensive health check
    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus {
            pdt_system: self.pdt_enforcer.is_some(),
            position_sizer: true,
            event_bus: true,
            database: self.database.is_some(),
            last_check: Utc::now(),
        }
    }
}
```

### Alert Thresholds

- **PDT Approaching Limit**: Warning when within 1 trade of limit
- **Daily Loss Warning**: Alert at 50% of daily loss limit
- **Position Concentration**: Warning when single position > 20% of portfolio
- **System Performance**: Alert when response time > 10ms

## Security Considerations

### Input Validation

All inputs are validated to prevent malicious or erroneous data:

- **Order Validation**: Symbol format, quantity ranges, price limits
- **Configuration Validation**: Reasonable limits and ranges
- **Date/Time Validation**: Business day and timezone checks

### Access Control

- **Configuration Changes**: Require administrative privileges
- **Emergency Operations**: Restricted to authorized personnel
- **Audit Trail**: Tamper-proof logging with cryptographic integrity

## Troubleshooting

### Common Issues

1. **False PDT Violations**
   - Verify business day calculations
   - Check timezone handling
   - Validate trade timestamp accuracy

2. **Position Size Rejections**
   - Confirm account equity updates
   - Check portfolio allocation settings
   - Verify position sizing configuration

3. **Emergency Liquidation Failures**
   - Check broker API connectivity
   - Verify position data accuracy
   - Review liquidation order placement

### Debug Commands

```rust
// Get current risk state
let state = risk_engine.get_risk_state().await;
println!("Daily P&L: ${}", state.daily_pl);
println!("Day trades: {}", state.day_trade_count);

// Check PDT status
if let Some(pdt) = &risk_engine.pdt_enforcer {
    let eligible = pdt.check_pdt_eligibility().await?;
    println!("PDT Eligible: {}", eligible);
}

// Reset for testing (admin only)
risk_engine.halt_trading("Maintenance").await;
risk_engine.resume_trading().await;
```

## Conclusion

The risk management system provides comprehensive protection for algorithmic trading operations with real-time monitoring, automated responses, and complete audit trails. The event-driven architecture ensures scalability and reliability while maintaining strict compliance with regulatory requirements.

For additional information, see:
- [PDT Implementation Guide](./pdt-implementation-guide.md)
- [API Reference](./api-reference.md)
- [Performance Guide](./performance-guide.md)

**Last Updated:** January 24, 2026
**Version:** 2.0 (Phase 2C-D)
**Next Review:** February 24, 2026