# Day Trading Platform: Coding Style and Architectural Requirements

## Overview

This document summarizes the established coding style and architectural requirements for the Day Trading Platform, a high-performance pattern-based stock trading system built in Rust. The system emphasizes functional programming principles, type safety, and real-time performance.

## Core Architectural Principles

### 1. Functional Programming Approach
- **Pure Functions**: All pattern detection algorithms are pure functions with immutable inputs
- **Zero-Copy Design**: Efficient memory usage with reference-based processing
- **Iterator Pipelines**: Functional data processing throughout the codebase
- **Immutable Data**: All pattern detection works with immutable data structures
- **Composable Design**: Pattern detectors can be easily combined and extended

### 2. Type Safety First
- **Newtype Pattern**: Strong typing for all domain concepts (PatternName, Confidence, Price, etc.)
- **Result Types**: Comprehensive error handling with typed errors
- **Input Validation**: Validation at API boundaries with detailed error messages
- **Compile-Time Guarantees**: Leverage Rust's type system to prevent runtime errors

### 3. Performance-First Design
- **Sub-millisecond Latency**: Pattern detection must complete in < 1ms per pattern
- **Zero Allocations**: Hot path processing with no memory allocation
- **Thread Safety**: Full concurrent processing support
- **Streaming Processing**: Process candles as they arrive, not in batches
- **Incremental Updates**: Update indicators incrementally rather than recalculating

### 4. Event-Driven Architecture
- **Pattern Detection Events**: Publish `PatternDetected` events to the event bus
- **Signal Generation Events**: Publish `TradingSignal` events for downstream processing
- **Event Integration**: Seamless integration with existing event-driven systems
- **Real-Time Processing**: Support for live market data streaming

## Coding Style Guidelines

### Code Structure
```rust
//! Module-level documentation explaining purpose and design
//!
//! This module implements X functionality using functional programming principles.
//! All functions are pure with immutable inputs and comprehensive error handling.

use crate::module::types::{DomainType, ErrorType};
use rust_decimal::Decimal;

/// Struct documentation with design rationale
#[derive(Debug, Clone)]
pub struct PatternDetector {
    // Fields with clear naming
}

/// Implementation with comprehensive documentation
impl PatternDetector {
    /// Creates a new detector with default configuration
    pub fn new() -> Self {
        Self { /* initialization */ }
    }

    /// Core detection method - pure function with immutable inputs
    ///
    /// # Arguments
    /// * `candles` - Immutable slice of candle data (newest first)
    /// * `params` - Detection parameters
    ///
    /// # Returns
    /// Pattern result or None if not detected
    pub fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        // Input validation first
        if candles.is_empty() {
            return Err(PatternError::InsufficientData {
                required: 1,
                available: 0,
            });
        }

        // Pure functional processing
        let result = self.validate_pattern(candles)?;

        if !result {
            return Ok(None);
        }

        // Build immutable context
        let context = self.build_context(candles);

        // Calculate confidence using pure functions
        let confidence = self.calculate_confidence(candles, &context)?;

        // Return result only if above threshold
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        Ok(Some(PatternResult::with_context(
            PatternName::new("pattern_name"),
            SignalType::Buy,
            confidence,
            candles[0].close,
            context,
        )))
    }

    // Private helper methods for separation of concerns
    fn validate_pattern(&self, candles: &[Candle]) -> Result<bool, PatternError> {
        // Mathematical validation logic
        Ok(true)
    }

    fn build_context(&self, candles: &[Candle]) -> PatternContext {
        // Immutable context construction
        PatternContext::new(candles.len())
    }

    fn calculate_confidence(
        &self,
        candles: &[Candle],
        context: &PatternContext,
    ) -> Result<Confidence, PatternError> {
        // Multi-factor confidence calculation
        let mut score = 50.0; // Base score

        // Mathematical factors
        // Volume confirmation
        // Trend context
        // Pattern metrics

        Confidence::new(score / 100.0)
            .map_err(|e| PatternError::InvalidParameters {
                message: e.to_string()
            })
    }
}
```

### Error Handling Pattern
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("Insufficient data: required {required}, available {available}")]
    InsufficientData { required: usize, available: usize },

    #[error("Invalid parameters: {message}")]
    InvalidParameters { message: String },

    #[error("Data quality issue: {issue}")]
    DataQuality { issue: String },
}

// Usage in functions
pub fn process_data(data: &[Candle]) -> Result<PatternResult, PatternError> {
    if data.len() < 3 {
        return Err(PatternError::InsufficientData {
            required: 3,
            available: data.len(),
        });
    }

    // Process data...
    Ok(result)
}
```

### Trait-Based Design
```rust
/// Core trait for all pattern detectors
pub trait PatternDetector: Send + Sync + Debug {
    /// Unique pattern name
    fn name(&self) -> &'static str;

    /// Minimum candles required
    fn min_candles_required(&self) -> usize;

    /// Core detection method - pure function
    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError>;

    /// Default implementation for optional methods
    fn category(&self) -> PatternCategory {
        PatternCategory::Candlestick
    }

    fn validate_params(&self, params: &PatternParams) -> Result<(), PatternError> {
        // Default validation
        Ok(())
    }
}
```

### Testing Approach
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_pattern_detection() {
        let detector = PatternDetector::new();
        let candles = create_test_candles();

        let result = detector.detect(&candles, &PatternParams::default());

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[rstest]
    #[case(vec![], 0)] // Edge case: empty data
    #[case(vec![single_candle()], 1)] // Edge case: insufficient data
    fn test_insufficient_data(#[case] candles: Vec<Candle>, #[case] expected_available: usize) {
        let detector = PatternDetector::new();

        let result = detector.detect(&candles, &PatternParams::default());

        assert!(matches!(
            result,
            Err(PatternError::InsufficientData { available, .. }) if available == expected_available
        ));
    }
}
```

## Module Organization

### Core Modules
- **`patterns/`**: Pattern detection algorithms and traits
  - `traits.rs`: Core interfaces (PatternDetector, etc.)
  - `types.rs`: Domain types (PatternResult, PatternContext, etc.)
  - `registry.rs`: Pattern management and execution
  - `candlestick.rs`: Single-candle patterns
  - `engulfing.rs`: Two-candle engulfing patterns
  - `multi_candle.rs`: Multi-candle patterns

- **`risk/`**: Risk management system
  - `engine.rs`: Core risk evaluation
  - `rules.rs`: Risk rule definitions
  - `types.rs`: Risk-related types

- **`events/`**: Event-driven architecture
  - `bus.rs`: Event publishing/subscription
  - `types.rs`: Event definitions

- **`brokers/`**: Broker abstraction layer
  - `traits.rs`: Broker interfaces
  - `types.rs`: Trading types (Order, Position, etc.)
  - `alpaca/`: Alpaca broker implementation

### File Structure Standards
```
src/
├── module_name/
│   ├── mod.rs           # Module exports and documentation
│   ├── types.rs         # Domain types and data structures
│   ├── traits.rs        # Trait definitions
│   ├── implementation.rs # Core implementation
│   └── tests.rs         # Unit tests (when large)
```

## Performance Requirements

### Latency Targets
- Pattern detection: < 1ms per pattern
- Signal generation: < 10ms end-to-end
- Order execution: < 100ms from signal to order placement

### Memory Usage
- Zero allocations in hot paths
- Reference-based processing
- Efficient caching strategies
- Memory footprint < 100MB for 50 symbols

### Concurrency
- Thread-safe pattern detection
- Parallel processing support
- Async trait implementations where needed
- Lock-free data structures where possible

## Quality Standards

### Code Quality
- `#![forbid(unsafe_code)]` - No unsafe code allowed
- `#![warn(missing_docs, rust_2018_idioms)]` - Comprehensive documentation required
- Clippy linting with pedantic settings
- 100% compilation success rate

### Testing Standards
- Architecture-driven testing (tests validate design, not drive it)
- Unit tests for all public APIs
- Integration tests for system interactions
- Property-based testing for mathematical algorithms
- Performance benchmarking for all critical paths

### Documentation Requirements
- Module-level documentation explaining design rationale
- Function documentation with parameters and return values
- Error type documentation with examples
- Completion reports for each development phase
- Architecture documentation for complex systems

## Development Workflow

### Phase-Based Development
1. **Planning Phase**: Create detailed technical specifications
2. **Implementation Phase**: Architecture-driven development
3. **Testing Phase**: Comprehensive validation of design decisions
4. **Documentation Phase**: Completion reports and architecture updates

### Code Review Standards
- All code reviewed for functional purity
- Performance implications assessed
- Error handling completeness verified
- Type safety guarantees confirmed
- Documentation accuracy validated

### Continuous Integration
- Clean compilation required
- All tests passing
- Performance benchmarks within targets
- Documentation generation successful
- Clippy warnings addressed

## Key Design Patterns

### Newtype Pattern
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternName(String);

impl PatternName {
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        // Validation logic
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

### Builder Pattern for Complex Objects
```rust
#[derive(Debug)]
pub struct PatternRegistryBuilder {
    config: RegistryConfig,
    detectors: Vec<Box<dyn PatternDetector>>,
}

impl PatternRegistryBuilder {
    pub fn new() -> Self { /* ... */ }

    pub fn with_config(mut self, config: RegistryConfig) -> Self {
        self.config = config;
        self
    }

    pub fn add_detector(mut self, detector: Box<dyn PatternDetector>) -> Self {
        self.detectors.push(detector);
        self
    }

    pub fn build(self) -> Result<PatternRegistry, RegistryError> {
        // Validation and construction
        Ok(PatternRegistry { /* ... */ })
    }
}
```

### Result-Based Error Handling
```rust
pub type Result<T> = std::result::Result<T, PatternError>;

pub fn detect_pattern(candles: &[Candle]) -> Result<Option<PatternResult>> {
    // All errors handled explicitly
    validate_input(candles)?;
    let pattern = analyze_candles(candles)?;
    let confidence = calculate_confidence(&pattern)?;

    Ok(Some(pattern))
}
```

## Conclusion

This codebase exemplifies a sophisticated approach to building high-performance, type-safe trading systems using Rust's functional programming capabilities. The emphasis on pure functions, comprehensive error handling, and performance optimization creates a robust foundation for real-time financial applications.

Key success factors:
- Architecture-driven development with clear design principles
- Rigorous type safety preventing runtime errors
- Performance-first design meeting sub-millisecond requirements
- Comprehensive testing validating architectural decisions
- Extensive documentation enabling knowledge transfer

The established patterns and standards ensure maintainable, reliable code that can scale to production trading environments.