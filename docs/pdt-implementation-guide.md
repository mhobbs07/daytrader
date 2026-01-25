# PDT Implementation Guide

## Overview

This document provides comprehensive guidance on implementing Pattern Day Trading (PDT) compliance in our algorithmic trading platform. Our implementation follows SEC and FINRA regulations while providing robust risk management capabilities.

## Architecture Overview

### Core Components

1. **PdtEnforcer** ([`src/risk/pdt.rs`](../src/risk/pdt.rs))
   - Main enforcement engine
   - Day trade counting and tracking
   - Business day calculations
   - Compliance checking

2. **Risk Engine Integration** ([`src/risk/engine.rs`](../src/risk/engine.rs))
   - Event-driven PDT monitoring
   - Real-time compliance checking
   - Violation handling and reporting

3. **Event System** ([`src/events/types.rs`](../src/events/types.rs))
   - PDT violation events
   - Risk event publishing
   - System-wide notifications

## Implementation Details

### Day Trade Detection

Our system automatically identifies day trades using the following criteria:

```rust
// A day trade occurs when:
// 1. Buy and sell (or sell and buy) the same security
// 2. On the same trading day
// 3. In a margin account
```

#### Business Day Calculation

The PDT rule uses a rolling 5 business day window:

```rust
impl PdtState {
    /// Calculates the cutoff date for the rolling window
    pub fn calculate_cutoff_date(&self, from_date: chrono::NaiveDate, business_days_back: u32) -> chrono::NaiveDate {
        let mut date = from_date;
        let mut days_counted = 0;

        while days_counted < business_days_back {
            date = date.pred_opt().unwrap();
            if self.is_business_day(date) {
                days_counted += 1;
            }
        }
        date
    }

    /// Checks if a date is a business day (Monday-Friday)
    fn is_business_day(&self, date: chrono::NaiveDate) -> bool {
        !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }
}
```

### Position Sizing Validation

Position sizing validation ensures trades comply with account equity limits and portfolio allocation rules:

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

#### Audit Trail Logging

All risk management actions are logged to the audit trail for compliance and debugging:

```rust
// Audit logging method
async fn log_risk_audit(&self, event_type: &str, severity: &str, description: String, metadata: Option<serde_json::Value>) {
    if let Some(ref performance_repo) = self.performance_repo {
        let audit = AuditLog {
            id: 0,
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            severity: severity.to_string(),
            user_id: None,
            account_id: Some(self.state.read().await.account_id.0.parse::<i64>().unwrap_or(0)),
            description,
            metadata: metadata.map(|v| v.to_string()),
        };
        if let Err(e) = performance_repo.store_audit_log(&audit).await {
            tracing::warn!("Failed to log risk audit entry: {}", e);
        }
    }
}
```

### PDT Configuration

Configure PDT enforcement through the `PdtConfig` structure:

```rust
use crate::risk::pdt::{PdtConfig, PdtEnforcer};
use rust_decimal::Decimal;

let config = PdtConfig {
    min_equity_threshold: Decimal::new(2500000, 2), // $25,000.00
    max_day_trades_under_threshold: 3,
    rolling_window_days: 5,
};

let enforcer = PdtEnforcer::new(config, initial_equity);
```

### Real-time Monitoring

The system continuously monitors trading activity:

```rust
// Pre-trade validation
let evaluation = pdt_enforcer.enforce_pdt_limits().await;
match evaluation {
    RiskEvaluation::Passed => {
        // Trade is allowed
    }
    RiskEvaluation::Violated { severity, suggested_action } => {
        // Handle PDT violation
        if severity == ViolationSeverity::Critical {
            // Block the trade
            return Err("PDT limit exceeded");
        }
    }
}
```

## API Reference

### PdtEnforcer Methods

#### Core Functionality

##### `check_pdt_eligibility()`
```rust
pub async fn check_pdt_eligibility(&self) -> Result<bool, String>
```
Checks if account is eligible for day trading based on current state.

**Returns:**
- `Ok(true)` if day trading is allowed
- `Ok(false)` if restricted (shouldn't happen in current implementation)
- `Err(message)` if PDT limit exceeded

**Example:**
```rust
match pdt_enforcer.check_pdt_eligibility().await {
    Ok(true) => println!("Day trading allowed"),
    Ok(false) => println!("Day trading restricted"),
    Err(msg) => println!("PDT violation: {}", msg),
}
```

##### `count_day_trades()`
```rust
pub async fn count_day_trades(&self) -> u32
```
Returns the number of day trades in the current 5-business-day rolling window.

**Example:**
```rust
let count = pdt_enforcer.count_day_trades().await;
println!("Current day trade count: {}", count);
```

##### `record_day_trade()`
```rust
pub async fn record_day_trade(&self, trade_date: chrono::NaiveDate)
```
Records a day trade occurrence for PDT compliance tracking.

**Parameters:**
- `trade_date`: Date when the day trade occurred

**Example:**
```rust
use chrono::Utc;
let today = Utc::now().date_naive();
pdt_enforcer.record_day_trade(today).await;
```

#### Account Management

##### `update_equity()`
```rust
pub async fn update_equity(&self, new_equity: Decimal)
```
Updates the current account equity value for PDT threshold calculations.

##### `meets_equity_requirement()`
```rust
pub async fn meets_equity_requirement(&self) -> bool
```
Returns true if account equity meets the $25,000 PDT threshold.

#### State Management

##### `get_pdt_state()`
```rust
pub async fn get_pdt_state(&self) -> PdtState
```
Returns a read-only copy of the current PDT state.

##### `reset_pdt_window()`
```rust
pub async fn reset_pdt_window(&self)
```
Clears all day trade history (typically used for testing or administrative purposes).

## Integration Examples

### Basic Setup

```rust
use crate::risk::pdt::{PdtEnforcer, PdtConfig};
use rust_decimal::Decimal;

// Initialize PDT enforcer
let config = PdtConfig {
    min_equity_threshold: Decimal::new(2500000, 2), // $25,000
    max_day_trades_under_threshold: 3,
    rolling_window_days: 5,
};

let initial_equity = Decimal::new(5000000, 2); // $50,000
let pdt_enforcer = PdtEnforcer::new(config, initial_equity);
```

### Pre-trade Validation

```rust
async fn validate_day_trade(
    pdt_enforcer: &PdtEnforcer,
    symbol: &str,
    is_closing_position: bool,
) -> Result<(), String> {
    // Only check PDT for opening trades that would close same day
    if !is_closing_position {
        return Ok(());
    }

    match pdt_enforcer.check_pdt_eligibility().await {
        Ok(true) => Ok(()),
        Err(msg) => Err(format!("PDT violation for {}: {}", symbol, msg)),
        _ => Ok(()),
    }
}
```

### Event-Driven Integration

```rust
use crate::events::types::{MarketEvent, RiskViolation, RiskViolationType};

async fn handle_order_fill(
    pdt_enforcer: &PdtEnforcer,
    event_bus: &EventBus,
    order_fill: &OrderFill,
) -> Result<(), Box<dyn std::error::Error>> {
    // Record day trade if applicable
    if is_day_trade(&order_fill) {
        pdt_enforcer.record_day_trade(order_fill.date).await;
        
        // Check if this puts us over the limit
        let current_count = pdt_enforcer.count_day_trades().await;
        let can_trade = pdt_enforcer.check_pdt_eligibility().await.is_ok();
        
        if !can_trade {
            // Publish PDT violation event
            let violation = RiskViolation {
                violation_type: RiskViolationType::PdtLimit,
                message: Message::new(format!(
                    "PDT limit reached: {} day trades in 5 business days", 
                    current_count
                )),
                timestamp: Utc::now(),
                pattern_name: None,
            };
            
            event_bus.publish(MarketEvent::RiskViolation { violation });
        }
    }
    
    Ok(())
}
```

## Error Handling

### PDT-Specific Errors

The system uses structured error handling for PDT violations:

```rust
pub enum RiskError {
    PdtLimitReached {
        current_count: u32,
        account_value: Decimal,
        required_value: Decimal,
    },
    // ... other variants
}
```

### Error Recovery

When PDT limits are reached:

1. **Immediate Actions:**
   - Block further day trades
   - Allow position closing only
   - Notify user/system administrators

2. **Recovery Options:**
   - Increase account equity above $25,000
   - Wait for rolling window to reset
   - Use cash account for trading

## Testing

#### Comprehensive Test Coverage

Phase 2C-D includes extensive testing across multiple dimensions:

- **Unit Tests**: Individual component testing for PDT enforcement, position sizing, and risk rules
- **Integration Tests**: End-to-end workflows testing risk engine interactions
- **Property-Based Tests**: Automated testing with generated inputs for edge cases
- **End-to-End Tests**: Full system testing with realistic market scenarios
- **Performance Benchmarks**: Load testing and performance validation

##### Unit Test Examples

```rust
#[tokio::test]
async fn test_pdt_day_trade_counting() {
    let config = create_test_config();
    let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

    let today = Utc::now().date_naive();

    // Record multiple day trades
    for _ in 0..3 {
        enforcer.record_day_trade(today).await;
    }

    let count = enforcer.count_day_trades().await;
    assert_eq!(count, 3);

    // Should still be eligible (at limit)
    assert!(enforcer.check_pdt_eligibility().await.is_ok());

    // One more trade should trigger violation
    enforcer.record_day_trade(today).await;
    assert!(enforcer.check_pdt_eligibility().await.is_err());
}

#[tokio::test]
async fn test_business_day_calculation() {
    let config = create_test_config();
    let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));
    let state = enforcer.get_pdt_state().await;

    // Test with a Friday date
    let friday = chrono::NaiveDate::from_ymd_opt(2023, 12, 15).unwrap();
    let cutoff = state.calculate_cutoff_date(friday, 5);

    // 5 business days back from Friday should be previous Friday
    let expected = chrono::NaiveDate::from_ymd_opt(2023, 12, 8).unwrap();
    assert_eq!(cutoff, expected);
}
```

##### Integration Test Coverage

```rust
#[tokio::test]
async fn test_risk_engine_position_sizing_integration() {
    let config = create_test_risk_config();
    let event_bus = Arc::new(EventBus::new(1000));
    let account_id = AccountId::new("test-account");
    let initial_equity = Decimal::new(5000000, 2);

    let engine = RiskEngine::new_with_mock_broker(config, event_bus, account_id, initial_equity);

    // Create test order
    let order = create_test_order("AAPL", 100, Decimal::new(15000, 2));

    // Validate order placement
    let result = engine.validate_order_placement(&order).await;
    assert!(result.is_ok());

    // Verify position sizing validation occurred
    // (Additional assertions based on test setup)
}
```

##### Performance Benchmarks

The system includes comprehensive performance benchmarks:

```rust
// Benchmark rule evaluation speed
pub fn bench_rule_evaluation_speed(c: &mut Criterion) {
    // Measures PDT eligibility checks, position sizing calculations, etc.
}

// Benchmark concurrent risk evaluations
fn bench_concurrent_load(c: &mut Criterion) {
    // Tests system performance under high concurrent load
}

// Benchmark emergency liquidation decisions
fn bench_emergency_liquidation_decisions(c: &mut Criterion) {
    // Validates emergency response performance
}
```

**Performance Targets:**
- PDT eligibility check: < 1ms
- Position sizing validation: < 0.5ms
- Emergency liquidation: < 100ms
- Concurrent evaluations: Support 100+ simultaneous checks

## Performance Considerations

### Optimization Strategies

1. **Memory Efficiency:**
   - Use `VecDeque` for rolling window storage
   - Automatic cleanup of old records
   - Minimal memory allocation

2. **Computation Efficiency:**
   - O(1) day trade counting
   - Efficient business day calculations
   - Lock-free read operations where possible

3. **Concurrent Access:**
   - `Arc<RwLock<>>` for thread-safe state management
   - Read-heavy optimization
   - Minimal lock contention

### Performance Benchmarks

Expected performance targets:
- **PDT eligibility check:** < 1ms
- **Day trade recording:** < 0.5ms  
- **State retrieval:** < 0.1ms
- **Business day calculation:** < 0.1ms

## Compliance Verification

### Audit Trail

The system maintains a complete audit trail:

```rust
pub struct DayTradeRecord {
    pub date: chrono::NaiveDate,
    pub count: u32,
}
```

### Regulatory Reporting

For regulatory compliance:

1. **Daily Reports:** Day trade counts per account
2. **Violation Reports:** PDT limit breaches
3. **Equity Reports:** Account equity levels
4. **Audit Logs:** All PDT-related actions

## Troubleshooting

### Common Issues

1. **Incorrect Day Trade Counting**
   - Verify business day logic
   - Check timezone handling
   - Validate rolling window calculation

2. **False PDT Violations**
   - Confirm equity updates
   - Verify account type (margin vs cash)
   - Check configuration values

3. **Performance Issues**
   - Monitor lock contention
   - Review cleanup frequency
   - Optimize database queries (if applicable)

### Debug Commands

```rust
// Get detailed PDT state
let state = pdt_enforcer.get_pdt_state().await;
println!("Equity: ${}", state.current_equity);
println!("Day trades: {}", state.day_trade_history.len());

// Reset for testing
pdt_enforcer.reset_pdt_window().await;

// Force equity update
pdt_enforcer.update_equity(Decimal::new(3000000, 2)).await;
```

## Security Considerations

### Input Validation

- Validate all date inputs
- Sanitize equity values
- Verify account permissions

### Access Control

- Restrict PDT reset operations
- Audit all configuration changes
- Monitor equity modifications

### Data Protection

- Encrypt sensitive account data
- Secure audit trail storage
- Implement backup procedures

## Conclusion

Our PDT implementation provides robust, compliant, and performant pattern day trading enforcement. The event-driven architecture ensures real-time monitoring while maintaining system flexibility and reliability.

For additional support or questions, refer to:
- [PDT Pattern Day Trading Rules](./pdt-pattern-day-trading-rules.md)
- [Risk Management Guide](./risk-management-guide.md) - Comprehensive risk system documentation including position sizing, emergency liquidation, and audit trails
- [Risk Management API Reference](./api-reference.md)
- [Troubleshooting Guide](./troubleshooting.md)

**Last Updated:** January 24, 2026
**Version:** 2.0 (Phase 2C-D)
**Next Review:** February 24, 2026