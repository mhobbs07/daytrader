# Testing Strategy for Functional Architecture

**Version:** 1.0  
**Date:** December 16, 2025  
**Philosophy:** Architecture-Driven Design with Validation Testing

---

## Core Philosophy

**CRITICAL PRINCIPLE: Tests validate architecture decisions, they do NOT drive them.**

Your functional pipeline and iterator-based architecture is correct because it:
- Provides composability and reusability
- Enables lazy evaluation and performance optimization
- Separates pure logic from side effects
- Aligns with Rust's zero-cost abstraction philosophy

Tests exist to:
1. **Validate** that the architecture works as designed
2. **Document** behavior through examples
3. **Catch regressions** when refactoring
4. **Verify** edge cases and error handling

Tests do NOT exist to:
- ❌ Dictate your architecture
- ❌ Force you into testability compromises
- ❌ Make you write code in a non-idiomatic way
- ❌ Add unnecessary abstractions for "mockability"

---

## Testing Approach for Functional Pipelines

### 1. Property-Based Testing for Pure Functions

**Why:** Pure functions with well-defined properties are perfect for property-based testing.

```rust
// src/patterns/candlestick.rs

use proptest::prelude::*;
use rust_decimal::Decimal;

// Pure function to test
pub fn calculate_body_size(candle: &Candle) -> Decimal {
    (candle.close - candle.open).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Property: Body size is always non-negative
    proptest! {
        #[test]
        fn body_size_is_non_negative(
            open in any::<f64>().prop_map(|f| Decimal::from_f64_retain(f).unwrap()),
            close in any::<f64>().prop_map(|f| Decimal::from_f64_retain(f).unwrap())
        ) {
            let candle = Candle { open, close, ..Default::default() };
            let body_size = calculate_body_size(&candle);
            prop_assert!(body_size >= Decimal::ZERO);
        }
    }
    
    // Property: Body size is commutative
    proptest! {
        #[test]
        fn body_size_is_commutative(
            price1 in any::<f64>().prop_map(|f| Decimal::from_f64_retain(f).unwrap()),
            price2 in any::<f64>().prop_map(|f| Decimal::from_f64_retain(f).unwrap())
        ) {
            let candle1 = Candle { open: price1, close: price2, ..Default::default() };
            let candle2 = Candle { open: price2, close: price1, ..Default::default() };
            
            prop_assert_eq!(
                calculate_body_size(&candle1),
                calculate_body_size(&candle2)
            );
        }
    }
}
```

**When to use:**
- Pure mathematical functions
- Data transformations
- Validation logic
- Calculations that have mathematical properties (commutativity, associativity, etc.)

---

### 2. Example-Based Testing for Business Logic

**Why:** Some behaviors are best documented with concrete examples.

```rust
// src/patterns/detector.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn hammer_pattern_detected_with_long_lower_wick() {
        // Given: A candle with classic hammer characteristics
        let candle = Candle {
            open: Decimal::new(10050, 2),   // 100.50
            high: Decimal::new(10100, 2),   // 101.00
            low: Decimal::new(9800, 2),     // 98.00
            close: Decimal::new(10080, 2),  // 100.80
            volume: 100000,
            timestamp: Utc::now(),
            symbol: "AAPL".to_string(),
        };
        
        // When: Pattern detection runs
        let result = detect_hammer(&candle);
        
        // Then: Hammer pattern is detected with appropriate confidence
        assert!(result.is_some());
        let pattern = result.unwrap();
        assert_eq!(pattern.name, "Hammer");
        assert_eq!(pattern.signal_type, SignalType::Bullish);
        assert!(pattern.confidence >= 0.7);
    }
    
    #[test]
    fn hammer_not_detected_without_long_lower_wick() {
        let candle = Candle {
            open: Decimal::new(10000, 2),
            high: Decimal::new(10100, 2),
            low: Decimal::new(9950, 2),    // Short lower wick
            close: Decimal::new(10050, 2),
            volume: 100000,
            timestamp: Utc::now(),
            symbol: "AAPL".to_string(),
        };
        
        let result = detect_hammer(&candle);
        assert!(result.is_none());
    }
}
```

**When to use:**
- Domain-specific business rules
- Pattern recognition logic
- Edge cases and boundary conditions
- Regression tests for bugs

---

### 3. Pipeline Testing with Iterator Chains

**Why:** Test the composition of functions, not just individual units.

```rust
// src/patterns/pipeline.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn pipeline_filters_low_confidence_signals() {
        // Given: Multiple candles with varying pattern qualities
        let candles = vec![
            create_strong_hammer(),    // High confidence
            create_weak_hammer(),      // Low confidence
            create_shooting_star(),    // High confidence
            create_doji(),             // Medium confidence
        ];
        
        let min_confidence = Confidence::new(0.7).unwrap();
        
        // When: Pipeline processes candles
        let signals: Vec<_> = candles.into_iter()
            .flat_map(|c| detect_all_patterns(&c))
            .filter(|s| s.confidence >= min_confidence)
            .collect();
        
        // Then: Only high-confidence signals remain
        assert_eq!(signals.len(), 2);
        assert!(signals.iter().all(|s| s.confidence >= min_confidence));
    }
    
    #[test]
    fn pipeline_is_lazy_and_composable() {
        let candles = generate_test_candles(1000);
        
        // Pipeline doesn't execute until collected
        let pipeline = candles.into_iter()
            .flat_map(|c| detect_all_patterns(&c))
            .filter(|s| s.confidence > 0.8)
            .map(|s| enrich_signal(s, &indicators));
        
        // Take only first 10 - should not process all 1000 candles
        let results: Vec<_> = pipeline.take(10).collect();
        
        assert_eq!(results.len(), 10);
        // If this test is fast, it proves lazy evaluation works
    }
}
```

**When to use:**
- Testing iterator chains and transformations
- Validating lazy evaluation behavior
- Ensuring composition works correctly
- Performance characteristics of pipelines

---

### 4. Integration Testing at Boundaries

**Why:** Side effects happen at boundaries - test them there, not in pure logic.

```rust
// tests/integration/data_pipeline.rs

use daytrader::*;

#[tokio::test]
async fn candle_ingestion_to_signal_generation_integration() {
    // Setup: Create test database and event bus
    let db = setup_test_database().await;
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    
    // Given: A candle arrives via WebSocket (simulated)
    let candle = create_test_candle("AAPL", 100.50);
    
    // When: Candle is processed through the full pipeline
    handle_new_candle(candle.clone(), &db, &event_bus).await.unwrap();
    
    // Then: Events are published
    let event = tokio::time::timeout(
        Duration::from_secs(1),
        rx.recv()
    ).await.unwrap().unwrap();
    
    match event {
        MarketEvent::NewCandle { symbol, .. } => {
            assert_eq!(symbol, "AAPL");
        }
        _ => panic!("Expected NewCandle event"),
    }
    
    // And: Candle is stored in database
    let stored = db.get_latest_candle("AAPL").await.unwrap();
    assert_eq!(stored.close, candle.close);
}

#[tokio::test]
async fn pattern_detection_publishes_signals() {
    let db = setup_test_database().await;
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    
    // Given: A candle that forms a hammer pattern
    let candle = create_hammer_candle("AAPL");
    
    // When: Processed through pattern detection
    handle_new_candle(candle, &db, &event_bus).await.unwrap();
    
    // Then: PatternDetected event is published
    let events: Vec<_> = collect_events(&mut rx, Duration::from_millis(500)).await;
    
    let pattern_events: Vec<_> = events.into_iter()
        .filter_map(|e| match e {
            MarketEvent::PatternDetected { signal } => Some(signal),
            _ => None,
        })
        .collect();
    
    assert!(!pattern_events.is_empty());
    assert_eq!(pattern_events[0].pattern_name, "Hammer");
}
```

**When to use:**
- Testing full workflows end-to-end
- Validating side effects (database writes, events)
- Ensuring components integrate correctly
- Testing async behavior and timing

---

### 5. Mock/Stub Only at System Boundaries

**Why:** Don't mock internal pure functions. Mock external systems only.

```rust
// tests/mocks/broker.rs

use async_trait::async_trait;
use daytrader::brokers::*;

/// Mock broker for testing - only mock the external API
pub struct MockBroker {
    responses: Arc<Mutex<VecDeque<BrokerResponse>>>,
}

impl MockBroker {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    
    pub fn expect_place_order(&mut self, response: Result<OrderId>) {
        self.responses.lock().unwrap().push_back(
            BrokerResponse::PlaceOrder(response)
        );
    }
}

#[async_trait]
impl OrderExecutionProvider for MockBroker {
    async fn place_order(&self, order: Order) -> Result<OrderId> {
        let mut responses = self.responses.lock().unwrap();
        match responses.pop_front() {
            Some(BrokerResponse::PlaceOrder(result)) => result,
            _ => panic!("Unexpected place_order call"),
        }
    }
    
    // ... other trait methods
}

// Usage in tests
#[tokio::test]
async fn order_execution_retries_on_timeout() {
    let mut broker = MockBroker::new();
    
    // First attempt times out
    broker.expect_place_order(Err(BrokerError::ConnectionFailed("timeout".into())));
    
    // Second attempt succeeds
    broker.expect_place_order(Ok(OrderId::new("order123")));
    
    let executor = OrderExecutor::new(broker);
    let order = create_test_order();
    
    let result = executor.place_order_with_retry(order).await;
    
    assert!(result.is_ok());
}
```

**What to mock:**
- ✅ External APIs (broker, market data)
- ✅ Database connections (for unit tests)
- ✅ Network calls
- ✅ File system operations

**What NOT to mock:**
- ❌ Pure functions (test them directly)
- ❌ Internal business logic
- ❌ Iterator chains
- ❌ Data transformations

---

## Testing Strategy by Component

### Pattern Detection (Pure Logic)

**Approach:** Property-based + Example-based

```rust
#[cfg(test)]
mod pattern_tests {
    // Property: Valid OHLC relationships
    proptest! {
        #[test]
        fn candle_validation_enforces_ohlc_relationships(
            open in 1.0f64..1000.0,
            high_offset in 0.0f64..10.0,
            low_offset in 0.0f64..10.0,
            close_offset in -10.0f64..10.0
        ) {
            let high = open + high_offset;
            let low = open - low_offset;
            let close = open + close_offset;
            
            let candle = Candle { open, high, low, close, ..Default::default() };
            
            // Property: High is highest, Low is lowest
            prop_assert!(candle.high >= candle.open);
            prop_assert!(candle.high >= candle.close);
            prop_assert!(candle.low <= candle.open);
            prop_assert!(candle.low <= candle.close);
        }
    }
    
    // Example: Specific pattern detection
    #[test]
    fn morning_star_pattern_three_candles() {
        let candles = vec![
            bearish_candle(100.0, 95.0),
            small_body_candle(94.0, 94.5),
            bullish_candle(95.0, 102.0),
        ];
        
        let pattern = detect_morning_star(&candles);
        assert!(pattern.is_some());
    }
}
```

### Risk Management (Business Rules)

**Approach:** Example-based with comprehensive scenarios

```rust
#[cfg(test)]
mod risk_tests {
    #[test]
    fn daily_loss_limit_blocks_order() {
        let config = RiskConfig::default();
        let account = create_account(10000.0);
        let metrics = DailyRiskMetrics {
            total_pl: Decimal::new(-50000, 2), // -$500 loss
            ..Default::default()
        };
        
        let order = create_buy_order(100.0);
        
        let result = validate_order(&order, &account, &config, &metrics);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RiskError::DailyLossLimitExceeded { .. }));
    }
    
    #[test]
    fn pdt_rule_allows_three_day_trades() {
        let config = RiskConfig::default();
        let account = create_account(20000.0); // Below PDT threshold
        let metrics = DailyRiskMetrics {
            day_trades_count: 2, // Already 2 day trades
            ..Default::default()
        };
        
        let order = create_buy_order(100.0);
        
        // 3rd day trade should be allowed
        let result = validate_order(&order, &account, &config, &metrics);
        assert!(result.is_ok());
    }
    
    #[test]
    fn pdt_rule_blocks_fourth_day_trade() {
        let config = RiskConfig::default();
        let account = create_account(20000.0);
        let metrics = DailyRiskMetrics {
            day_trades_count: 3, // Already 3 day trades
            ..Default::default()
        };
        
        let order = create_buy_order(100.0);
        
        // 4th day trade should be blocked
        let result = validate_order(&order, &account, &config, &metrics);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RiskError::PdtLimitReached { .. }));
    }
}
```

### Event Bus (Concurrency)

**Approach:** Integration tests with timing

```rust
#[tokio::test]
async fn event_bus_broadcasts_to_multiple_subscribers() {
    let bus = EventBus::new(100);
    
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();
    let mut rx3 = bus.subscribe();
    
    let event = MarketEvent::NewCandle {
        symbol: "AAPL".to_string(),
        candle: create_test_candle(),
    };
    
    bus.publish(event.clone());
    
    // All subscribers receive the event
    assert_eq!(rx1.recv().await.unwrap(), event);
    assert_eq!(rx2.recv().await.unwrap(), event);
    assert_eq!(rx3.recv().await.unwrap(), event);
}

#[tokio::test]
async fn event_bus_handles_slow_subscribers() {
    let bus = EventBus::new(10); // Small buffer
    let mut rx = bus.subscribe();
    
    // Publish many events without consuming
    for i in 0..20 {
        bus.publish(MarketEvent::test_event(i));
    }
    
    // Slow subscriber eventually catches up
    let received: Vec<_> = collect_events(&mut rx, Duration::from_secs(1)).await;
    
    // Should receive at least some events (may drop due to buffer)
    assert!(!received.is_empty());
}
```

---

## When to Write Tests

### ✅ DO Write Tests For:

1. **Pure business logic functions**
   - Pattern detection algorithms
   - Risk calculations
   - Technical indicators
   - Signal generation

2. **Critical safety features**
   - Risk management rules
   - PDT enforcement
   - Position size limits
   - Daily loss tracking

3. **Integration points**
   - Broker API integration
   - Database operations
   - Event bus behavior
   - WebSocket connection handling

4. **Edge cases and bugs**
   - Boundary conditions
   - Error handling
   - Race conditions
   - Regression tests

### ❌ DON'T Write Tests For:

1. **Trivial getters/setters**
   ```rust
   // Don't test this
   impl Order {
       pub fn symbol(&self) -> &str {
           &self.symbol
       }
   }
   ```

2. **Framework code**
   - Don't test Tokio works
   - Don't test SQLite works
   - Trust your dependencies

3. **Internal implementation details**
   - Don't test private helper functions in isolation
   - Test the public API instead

4. **Configuration parsing**
   - If using a well-tested library like `config`, trust it

---

## Test Organization

```
tests/
├── unit/                          # Unit tests (co-located with code)
│   └── (in src/ files)
├── integration/                   # Integration tests
│   ├── data_pipeline_test.rs
│   ├── pattern_detection_test.rs
│   ├── risk_management_test.rs
│   └── order_execution_test.rs
├── property/                      # Property-based tests
│   ├── candle_properties.rs
│   ├── pattern_properties.rs
│   └── risk_properties.rs
├── fixtures/                      # Test data
│   ├── candles.json
│   ├── patterns.json
│   └── orders.json
└── mocks/                         # Mock implementations
    ├── mock_broker.rs
    └── mock_database.rs
```

---

## Testing Commands

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run property-based tests with more cases
PROPTEST_CASES=10000 cargo test

# Run tests with logging
RUST_LOG=debug cargo test -- --nocapture

# Run specific test
cargo test test_hammer_detection

# Run tests in parallel (default)
cargo test

# Run tests serially (for debugging)
cargo test -- --test-threads=1

# Generate coverage report
cargo tarpaulin --out Html
```

---

## Performance Testing (Benchmarks)

**Why:** Validate that your functional architecture performs well.

```rust
// benches/pattern_detection.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use daytrader::patterns::*;

fn benchmark_pattern_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_detection");
    
    for num_candles in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("detect_all_patterns", num_candles),
            num_candles,
            |b, &n| {
                let candles = generate_test_candles(n);
                b.iter(|| {
                    candles.iter()
                        .flat_map(|c| detect_all_patterns(black_box(c)))
                        .count()
                });
            }
        );
    }
    
    group.finish();
}

fn benchmark_iterator_pipeline(c: &mut Criterion) {
    let candles = generate_test_candles(1000);
    let indicators = create_test_indicators();
    
    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            process_candle_pipeline(
                black_box(candles.clone()),
                black_box(&indicators),
                Confidence::new(0.7).unwrap()
            ).count()
        });
    });
}

criterion_group!(benches, benchmark_pattern_detection, benchmark_iterator_pipeline);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

---

## Summary

### Testing Philosophy

1. **Architecture First**: Design for correctness, composability, and performance
2. **Tests Validate**: Write tests to validate your design works
3. **Functional Style**: Use property-based testing for pure functions
4. **Integration Tests**: Test side effects at boundaries
5. **Mock Sparingly**: Only mock external systems, not internal logic

### Test Coverage Goals

- **Critical paths**: 100% coverage (risk management, PDT enforcement)
- **Business logic**: 80%+ coverage (pattern detection, signals)
- **Integration points**: Representative scenarios covered
- **Pure functions**: Property-based tests where applicable

### What Makes a Good Test

✅ **Good Test:**
- Tests behavior, not implementation
- Clear arrange-act-assert structure
- Descriptive test name
- Fast execution
- Independent (no test order dependency)

❌ **Bad Test:**
- Tests implementation details
- Brittle (breaks on refactoring)
- Slow (unnecessary I/O)
- Coupled to other tests
- Unclear purpose

---

## Conclusion

Your functional pipeline architecture is **correct**. Tests exist to:
- Prove it works
- Document its behavior
- Catch regressions
- Validate edge cases

Tests do NOT exist to dictate how you write code. Write clean, functional, composable code first. Then write tests that validate it works as designed.

**Remember: Good architecture enables good tests. Good tests don't create good architecture.**

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Philosophy:** Architecture-Driven Design