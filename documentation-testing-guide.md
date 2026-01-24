# Documentation Testing Guide

## Overview

This guide provides comprehensive instructions for adding documentation tests (doc tests) and usage examples to the Day Trading Platform codebase. Doc tests ensure that code examples in documentation remain accurate and functional.

## Documentation Test Standards

### When to Add Doc Tests

Add doc tests to **all public APIs** that:
- Are part of the stable public interface
- Have complex usage patterns
- Demonstrate important functionality
- Could be misused without proper examples

### Doc Test Requirements

1. **Compilable**: All examples must compile without errors
2. **Runnable**: Examples should execute successfully
3. **Minimal**: Use only necessary imports and code
4. **Realistic**: Show practical, real-world usage
5. **Well-commented**: Explain what the example demonstrates

## Adding Doc Tests to Existing Code

### Pattern for Type Constructors

```rust
/// Creates a new price with validation.
///
/// # Examples
///
/// Basic price creation:
/// ```
/// use daytrader::brokers::types::Price;
/// use rust_decimal::Decimal;
///
/// let price = Price::new(Decimal::new(15050, 2)).unwrap();
/// assert_eq!(price.value(), Decimal::new(15050, 2));
/// ```
///
/// Price validation (rejection of invalid values):
/// ```
/// use daytrader::brokers::types::Price;
/// use rust_decimal::Decimal;
///
/// // Zero prices are rejected
/// assert!(Price::new(Decimal::ZERO).is_err());
///
/// // Negative prices are rejected
/// assert!(Price::new(Decimal::new(-100, 0)).is_err());
/// ```
pub fn new(value: Decimal) -> Result<Self, &'static str> {
    // Implementation
}
```

### Pattern for Complex Operations

```rust
/// Detects patterns in the provided candle data.
///
/// This method analyzes recent price action to identify technical patterns
/// with confidence scoring based on multiple factors.
///
/// # Arguments
///
/// * `candles` - Recent candle data (newest first, minimum required candles)
/// * `params` - Detection parameters controlling sensitivity
///
/// # Returns
///
/// Returns `Some(PatternResult)` if a pattern is detected with sufficient confidence,
/// or `None` if no pattern meets the criteria.
///
/// # Examples
///
/// Basic pattern detection:
/// ```
/// use daytrader::patterns::traits::PatternDetector;
/// use daytrader::patterns::candlestick::HammerDetector;
/// use daytrader::patterns::types::PatternParams;
/// use daytrader::events::types::Candle;
/// use daytrader::brokers::types::{Symbol, Price};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// // Create a hammer detector
/// let detector = HammerDetector::new();
///
/// // Create sample candle data showing a hammer pattern
/// let candles = vec![
///     Candle {
///         symbol: Symbol::new("AAPL").unwrap(),
///         timestamp: Utc::now(),
///         open: Price::new(Decimal::new(10000, 2)).unwrap(),
///         high: Price::new(Decimal::new(10200, 2)).unwrap(),
///         low: Price::new(Decimal::new(9500, 2)).unwrap(),  // Long lower wick
///         close: Price::new(Decimal::new(10100, 2)).unwrap(),
///         volume: 1000,
///     }
/// ];
///
/// // Detect patterns
/// let params = PatternParams::new().with_min_confidence(0.5);
/// let result = detector.detect(&candles, &params);
///
/// // Should detect a hammer pattern
/// assert!(result.is_ok());
/// assert!(result.unwrap().is_some());
/// ```
///
/// Handling insufficient data:
/// ```
/// use daytrader::patterns::traits::PatternDetector;
/// use daytrader::patterns::candlestick::HammerDetector;
/// use daytrader::patterns::types::PatternParams;
///
/// let detector = HammerDetector::new();
/// let empty_candles: Vec<Candle> = vec![];
/// let params = PatternParams::default();
///
/// let result = detector.detect(&empty_candles, &params);
/// assert!(result.is_err()); // Insufficient data error
/// ```
pub fn detect(
    &self,
    candles: &[Candle],
    params: &PatternParams,
) -> Result<Option<PatternResult>, PatternError> {
    // Implementation
}
```

### Pattern for Error Types

```rust
/// Pattern detection errors with detailed context.
///
/// This enum provides specific error types for different failure modes
/// in pattern detection operations.
///
/// # Examples
///
/// Handling insufficient data:
/// ```
/// use daytrader::patterns::types::PatternError;
///
/// // This would typically come from a detector call
/// let error = PatternError::InsufficientData {
///     required: 3,
///     available: 1,
/// };
///
/// match error {
///     PatternError::InsufficientData { required, available } => {
///         println!("Need {} candles, got {}", required, available);
///     }
///     _ => println!("Other error type"),
/// }
/// ```
///
/// Handling invalid parameters:
/// ```
/// use daytrader::patterns::types::PatternError;
///
/// let error = PatternError::InvalidParameters {
///     message: "Confidence must be between 0.0 and 1.0".to_string(),
/// };
///
/// assert_eq!(error.to_string(), "Invalid pattern parameters: Confidence must be between 0.0 and 1.0");
/// ```
#[derive(Clone, Debug, thiserror::Error)]
pub enum PatternError {
    // Error variants...
}
```

## Module-Level Documentation Examples

### Core Module Documentation

```rust
//! # Pattern Recognition Engine
//!
//! This module provides a comprehensive pattern recognition system for technical analysis
//! of financial markets. It implements various candlestick patterns and technical indicators
//! with confidence scoring and risk management integration.
//!
//! ## Architecture
//!
//! The pattern recognition system follows functional programming principles:
//! - Pure functions for pattern detection algorithms
//! - Immutable data structures and zero-copy processing
//! - Trait-based design for extensibility
//! - Comprehensive error handling with typed errors
//!
//! ## Key Components
//!
//! - [`PatternDetector`]: Core trait for all pattern detection algorithms
//! - [`PatternRegistry`]: Dynamic pattern registration and execution
//! - [`PatternResult`]: Structured output from pattern detection
//! - [`PatternContext`]: Additional context and metadata
//!
//! ## Usage
//!
//! ```rust
//! use daytrader::patterns::{PatternRegistry, PatternParams};
//! use daytrader::events::types::Candle;
//!
//! // Create registry and register patterns
//! let mut registry = PatternRegistry::new();
//! // Register pattern detectors...
//!
//! // Analyze market data
//! let candles: Vec<Candle> = get_market_data();
//! let params = PatternParams::default();
//!
//! // Detect all patterns
//! let results = registry.detect_all(&candles, Some(&params)).await;
//!
//! for result in results {
//!     println!("Detected: {} with confidence {:.2}",
//!              result.pattern_name.as_str(),
//!              result.confidence.value());
//! }
//! ```
//!
//! ## Performance Characteristics
//!
//! - Pattern detection: < 1ms per pattern
//! - Memory usage: Zero allocations in hot paths
//! - Thread safety: Full concurrent processing support
//! - Scalability: O(1) complexity for most algorithms
//!
//! ## Error Handling
//!
//! All pattern detection operations return `Result<T, PatternError>` with specific
//! error types for different failure modes. Errors are designed to provide actionable
//! information for debugging and user feedback.
//!
//! ## Testing
//!
//! The module includes comprehensive testing:
//! - Unit tests for individual pattern algorithms
//! - Integration tests for full workflow validation
//! - Property-based tests for mathematical correctness
//! - Performance benchmarks for timing validation
```

## Running Doc Tests

### Local Testing

```bash
# Run all doc tests
cargo test --doc

# Run doc tests for specific package
cargo test --doc --package daytrader

# Run doc tests with output
cargo test --doc -- --nocapture

# Run specific doc test
cargo test --doc pattern_name
```

### CI/CD Integration

Add to GitHub Actions workflow:

```yaml
- name: Run documentation tests
  run: cargo test --doc

- name: Check documentation builds
  run: cargo doc --no-deps
```

## Best Practices for Doc Tests

### 1. Keep Examples Focused
```rust
// Good: Focused on one concept
/// ```rust
/// let price = Price::new(dec!(150.50)).unwrap();
/// assert_eq!(price.value(), dec!(150.50));
/// ```

// Bad: Too many concepts
/// ```rust
/// let price = Price::new(dec!(150.50)).unwrap();
/// let order = Order::new(price, 100).unwrap();
/// let filled = execute_order(order).await.unwrap();
/// assert_eq!(filled.quantity, 100);
/// ```
```

### 2. Use Realistic Data
```rust
// Good: Realistic market data
/// ```rust
/// let candle = Candle {
///     symbol: Symbol::new("AAPL").unwrap(),
///     timestamp: Utc::now(),
///     open: Price::new(dec!(150.00)).unwrap(),
///     high: Price::new(dec!(152.00)).unwrap(),
///     low: Price::new(dec!(149.00)).unwrap(),
///     close: Price::new(dec!(151.50)).unwrap(),
///     volume: 1000000,
/// };
/// ```

// Bad: Unrealistic or placeholder data
/// ```rust
/// let candle = Candle {
///     symbol: Symbol::new("TEST").unwrap(),
///     timestamp: Utc::now(),
///     open: Price::new(dec!(1.00)).unwrap(),
///     high: Price::new(dec!(1.00)).unwrap(),
///     low: Price::new(dec!(1.00)).unwrap(),
///     close: Price::new(dec!(1.00)).unwrap(),
///     volume: 1,
/// };
/// ```
```

### 3. Handle Errors Appropriately
```rust
// Good: Show both success and error cases
/// ```rust
/// // Valid price
/// let price = Price::new(dec!(150.50)).unwrap();
/// assert_eq!(price.value(), dec!(150.50));
///
/// // Invalid price (should fail)
/// assert!(Price::new(dec!(-10.00)).is_err());
/// ```

// Bad: Only show success cases
/// ```rust
/// let price = Price::new(dec!(150.50)).unwrap();
/// // No error handling examples
/// ```
```

### 4. Import Only What's Needed
```rust
// Good: Minimal imports
/// ```rust
/// use daytrader::brokers::types::Price;
/// use rust_decimal::Decimal;
///
/// let price = Price::new(Decimal::new(15050, 2)).unwrap();
/// ```

// Bad: Unnecessary imports
/// ```rust
/// use daytrader::*;
/// use std::collections::*;
/// use chrono::*;
///
/// let price = Price::new(Decimal::new(15050, 2)).unwrap();
/// ```
```

## Common Doc Test Patterns

### Builder Pattern Examples
```rust
/// Creates a pattern registry with custom configuration.
///
/// # Examples
///
/// Basic registry creation:
/// ```rust
/// use daytrader::patterns::PatternRegistry;
///
/// let registry = PatternRegistry::new();
/// assert_eq!(registry.pattern_count(), 0);
/// ```
///
/// Registry with custom configuration:
/// ```rust
/// use daytrader::patterns::{PatternRegistry, RegistryConfig};
///
/// let config = RegistryConfig {
///     max_parallel_patterns: 5,
///     global_min_confidence: 0.7,
///     ..Default::default()
/// };
///
/// let registry = PatternRegistry::with_config(config);
/// // Registry configured with custom settings
/// ```
```

### Async Function Examples
```rust
/// Detects patterns across all registered detectors.
///
/// This method runs pattern detection in parallel for better performance
/// with real-time market data processing.
///
/// # Examples
///
/// ```rust,no_run
/// use daytrader::patterns::PatternRegistry;
/// use daytrader::events::types::Candle;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let registry = PatternRegistry::new();
/// // Register patterns...
///
/// let candles: Vec<Candle> = get_market_data();
/// let results = registry.detect_all(&candles, None).await;
///
/// println!("Detected {} patterns", results.len());
/// # Ok(())
/// # }
/// ```
```

### Error Propagation Examples
```rust
/// Validates and processes a trading order.
///
/// # Examples
///
/// Successful order processing:
/// ```rust
/// use daytrader::execution::OrderProcessor;
/// use daytrader::brokers::types::{Order, OrderType, Symbol, Price, Quantity};
/// use rust_decimal::Decimal;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let processor = OrderProcessor::new();
///
/// let order = Order {
///     id: "order-123".into(),
///     symbol: Symbol::new("AAPL").unwrap(),
///     side: OrderSide::Buy,
///     order_type: OrderType::Limit,
///     quantity: Quantity::new(100).unwrap(),
///     limit_price: Some(Price::new(Decimal::new(15000, 2)).unwrap()),
///     // ... other fields
/// };
///
/// match processor.process_order(order).await {
///     Ok(confirmed_order) => println!("Order confirmed: {}", confirmed_order.id),
///     Err(e) => println!("Order failed: {}", e),
/// }
/// # Ok(())
/// # }
/// ```
```

## Quality Assurance

### Doc Test Checklist

- [ ] All public functions have doc tests
- [ ] Examples compile and run successfully
- [ ] Examples use realistic data
- [ ] Error cases are demonstrated
- [ ] Examples are minimal but complete
- [ ] Cross-references are accurate
- [ ] No outdated examples after API changes

### Maintenance

- **Update Examples**: When APIs change, update corresponding doc tests
- **Test Regularly**: Run doc tests in CI/CD pipeline
- **Review Coverage**: Audit for missing examples on new public APIs
- **Version Control**: Doc tests are part of the API contract

## Integration with Development Workflow

### Pull Request Requirements

- New public APIs require doc tests
- Modified APIs need updated examples
- Doc tests must pass in CI/CD
- Examples reviewed for accuracy and clarity

### Code Review Guidelines

- Verify examples match current API
- Check that examples are educational
- Ensure error handling is demonstrated
- Confirm examples use current best practices