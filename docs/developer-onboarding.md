# Developer Onboarding Guide

Welcome to the Day Trading Platform! This guide will help you get started as a developer on this high-performance algorithmic trading system.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Environment Setup](#environment-setup)
3. [Project Overview](#project-overview)
4. [Development Workflow](#development-workflow)
5. [Code Organization](#code-organization)
6. [Key Concepts](#key-concepts)
7. [First Contributions](#first-contributions)
8. [Testing](#testing)
9. [Deployment](#deployment)
10. [Resources](#resources)

## Prerequisites

### Required Knowledge

- **Rust Programming**: Intermediate level required
  - Ownership and borrowing
  - Traits and generics
  - Async/await patterns
  - Error handling with `Result<T, E>`

- **Financial Markets**: Basic understanding
  - Candlestick patterns
  - Order types (market, limit, stop)
  - Risk management concepts
  - Pattern Day Trading (PDT) rules

- **Systems Programming**: Familiarity with
  - Concurrent programming
  - Network programming
  - Database operations
  - Performance optimization

### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Rust**: 1.75+ (install via [rustup](https://rustup.rs/))
- **Git**: Latest version
- **SQLite**: 3.35+ (usually pre-installed)
- **Memory**: 8GB RAM minimum
- **Disk Space**: 2GB free space

### Development Tools

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.75+
cargo --version  # Should be 1.75+

# Install additional tools
rustup component add rustfmt
rustup component add clippy
cargo install cargo-audit
cargo install cargo-tarpaulin  # For code coverage
```

## Environment Setup

### 1. Clone the Repository

```bash
git clone <repository-url>
cd daytrader
```

### 2. Set Up Development Environment

```bash
# Copy environment template
cp .env.example .env

# Edit environment variables
nano .env
```

**Required environment variables:**
```bash
# Alpaca API credentials (for paper trading)
ALPACA_API_KEY=your_paper_api_key
ALPACA_API_SECRET=your_paper_api_secret

# Logging level
RUST_LOG=daytrader=debug

# Database path (optional, defaults to ./data/daytrader.db)
DATABASE_URL=./data/daytrader.db
```

### 3. Initialize the Database

```bash
# Run database setup script
./scripts/setup_db.sh

# Verify database creation
ls -la data/
sqlite3 data/daytrader.db ".tables"
```

### 4. Build the Project

```bash
# Check for issues
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy

# Build in debug mode
cargo build

# Build optimized release
cargo build --release
```

### 5. Run Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_pattern_detection

# Run benchmarks
cargo bench
```

### 6. Run the Application

```bash
# Development mode
cargo run

# With specific config
cargo run -- --config config/custom.toml

# Release mode
cargo run --release
```

## Project Overview

### What We're Building

The Day Trading Platform is a high-performance system for automated pattern-based stock trading with the following features:

- **Real-time Pattern Detection**: Candlestick pattern recognition using mathematical algorithms
- **Risk Management**: Comprehensive position sizing and loss limits with PDT compliance
- **Event-Driven Architecture**: Async communication between components
- **Broker Integration**: Alpaca API support with extensible broker abstraction
- **Performance Focus**: Sub-millisecond pattern detection with memory efficiency

### Current Status (Phase 3A-2 Complete)

- ✅ **Core Infrastructure**: Event system, logging, configuration
- ✅ **Pattern Recognition**: 5 basic candlestick patterns implemented
- ✅ **Risk Management**: Position limits, PDT rules, loss thresholds
- ✅ **Broker Integration**: Alpaca REST and WebSocket APIs
- ✅ **Database Layer**: SQLite with time-series optimization
- 🔄 **Next Phase**: Technical indicators and advanced patterns

### Architecture Principles

1. **Functional Programming**: Pure functions, immutable data, zero-copy operations
2. **Type Safety**: Strong typing with newtype pattern for domain values
3. **Async First**: All I/O operations are asynchronous
4. **Memory Safe**: Zero unsafe code, compile-time guarantees
5. **Testable**: Comprehensive testing with mocks and integration tests

## Development Workflow

### Git Workflow

We follow a structured Git workflow with feature branches and pull requests:

```bash
# Create feature branch
git checkout -b feature/add-new-pattern

# Make changes with descriptive commits
git add src/patterns/
git commit -m "feat: add morning star pattern detection

- Implement morning star candlestick pattern
- Add confidence scoring for 3-candle patterns
- Include comprehensive tests"

# Push and create PR
git push origin feature/add-new-pattern
```

### Branch Naming Convention

- `feature/description`: New features
- `bugfix/description`: Bug fixes
- `refactor/description`: Code refactoring
- `docs/description`: Documentation updates
- `test/description`: Test improvements

### Pull Request Process

1. **Create PR**: Use descriptive title and detailed description
2. **Code Review**: At least one approval required
3. **CI Checks**: All tests must pass
4. **Security Audit**: `cargo audit` must pass
5. **Merge**: Squash merge with descriptive commit message

### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

**Types:**
- `feat`: New features
- `fix`: Bug fixes
- `docs`: Documentation
- `style`: Code style changes
- `refactor`: Code refactoring
- `test`: Testing
- `chore`: Maintenance

## Code Organization

### Directory Structure

```
daytrader/
├── src/                    # Source code
│   ├── main.rs            # Application entry point
│   ├── lib.rs             # Library exports
│   ├── brokers/           # Broker integrations
│   │   ├── mod.rs
│   │   ├── traits.rs      # Broker trait definitions
│   │   ├── alpaca/        # Alpaca implementation
│   │   └── mock.rs        # Mock broker for testing
│   ├── patterns/          # Pattern recognition
│   │   ├── mod.rs
│   │   ├── traits.rs      # Pattern detector interface
│   │   ├── registry.rs    # Pattern registration system
│   │   ├── candlestick.rs # Single-candle patterns
│   │   ├── engulfing.rs   # Two-candle patterns
│   │   └── utils.rs       # Pattern utilities
│   ├── risk/              # Risk management
│   │   ├── mod.rs
│   │   ├── engine.rs      # Risk engine implementation
│   │   ├── rules.rs       # Risk rules and validation
│   │   └── pdt.rs         # PDT rule monitoring
│   ├── events/            # Event system
│   │   ├── mod.rs
│   │   ├── bus.rs         # Event bus implementation
│   │   └── types.rs       # Event type definitions
│   ├── db/                # Database layer
│   │   ├── mod.rs
│   │   └── schema.sql     # Database schema
│   ├── config/            # Configuration
│   │   └── mod.rs
│   └── utils/             # Shared utilities
│       ├── mod.rs
│       ├── error_context.rs
│       ├── logging.rs
│       ├── metrics.rs
│       ├── performance_monitor.rs
│       └── retry.rs
├── config/                # Configuration files
├── scripts/               # Utility scripts
├── examples/              # Usage examples
├── benches/               # Performance benchmarks
├── tests/                 # Integration tests
└── docs/                  # Documentation
```

### Module Responsibilities

- **`brokers/`**: External API integrations (Alpaca, future brokers)
- **`patterns/`**: Candlestick pattern detection algorithms
- **`risk/`**: Position sizing, loss limits, PDT compliance
- **`events/`**: Inter-component communication
- **`db/`**: Data persistence and retrieval
- **`config/`**: Configuration loading and validation
- **`utils/`**: Shared functionality and cross-cutting concerns

## Key Concepts

### Domain Types (Newtypes)

```rust
// Strong typing prevents mixing incompatible values
pub struct Symbol(String);      // Stock symbol with validation
pub struct Price(Decimal);      // Monetary values with precision
pub struct Quantity(u32);       // Share quantities
pub struct Confidence(f64);     // 0.0 to 1.0 confidence scores

// Usage
let symbol = Symbol::new("AAPL")?;
let price = Price::new(Decimal::new(15050, 2))?; // $150.50
let confidence = Confidence::new(0.85)?;
```

### Error Handling

```rust
// Result-based error handling throughout
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Pattern detection error: {0}")]
    Pattern(#[from] PatternError),

    #[error("Broker API error: {0}")]
    Broker(#[from] BrokerError),
}

// Usage
fn process_data(data: &Data) -> Result<ProcessedData> {
    let validated = validate_data(data)?;
    let result = process_validated(validated)?;
    Ok(result)
}
```

### Async Patterns

```rust
// All I/O is async
async fn fetch_market_data(symbol: &Symbol) -> Result<Vec<Candle>> {
    let client = reqwest::Client::new();
    let response = client.get(&format!("{}/market-data", BASE_URL))
        .query(&[("symbol", symbol.as_str())])
        .send()
        .await?;

    let candles = response.json().await?;
    Ok(candles)
}

// Concurrent processing
async fn analyze_multiple_symbols(symbols: Vec<String>) -> Result<()> {
    let tasks: Vec<_> = symbols.into_iter()
        .map(|symbol| tokio::spawn(async move {
            analyze_symbol(&symbol).await
        }))
        .collect();

    for task in tasks {
        let result = task.await??;
        process_result(result).await?;
    }

    Ok(())
}
```

### Event-Driven Architecture

```rust
// Components communicate via events
#[derive(Debug, Clone)]
pub enum MarketEvent {
    PatternDetected { signal: PatternSignal },
    OrderPlaced { order: Order },
    RiskViolation { violation: RiskViolation },
}

// Event publishing
event_bus.publish(MarketEvent::PatternDetected { signal });

// Event subscription
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    match event {
        MarketEvent::PatternDetected { signal } => {
            handle_pattern_signal(signal).await?;
        }
        // Handle other events...
    }
}
```

## First Contributions

### Finding Tasks

1. **Check GitHub Issues**: Look for "good first issue" or "help wanted" labels
2. **Review TODO Comments**: Search for `TODO`, `FIXME`, or `XXX` in code
3. **Performance Improvements**: Profile and optimize slow code paths
4. **Test Coverage**: Add tests for uncovered code

### Example: Adding a New Pattern

```rust
// 1. Implement the PatternDetector trait
pub struct MyNewPattern;

impl PatternDetector for MyNewPattern {
    fn name(&self) -> &'static str { "my_new_pattern" }

    fn min_candles_required(&self) -> usize { 3 }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>> {
        // Implementation here
        Ok(None)
    }
}

// 2. Add to pattern registry
registry.register("my_new_pattern", Box::new(MyNewPattern::new()))?;

// 3. Add comprehensive tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_new_pattern_detection() {
        // Test implementation
    }
}
```

### Code Review Checklist

- [ ] **Functionality**: Code works as intended
- [ ] **Tests**: Comprehensive test coverage
- [ ] **Documentation**: Public APIs documented
- [ ] **Style**: Follows Rust and project conventions
- [ ] **Performance**: No obvious performance issues
- [ ] **Security**: No security vulnerabilities
- [ ] **Error Handling**: Proper error handling and propagation

## Testing

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_pattern_detection_success() {
        let detector = HammerDetector::new();
        let candles = create_test_hammer_candles();
        let params = PatternParams::default();

        let result = detector.detect(&candles, &params);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[rstest]
    #[case(0, 1)]  // Empty candles
    #[case(1, 1)]  // Insufficient candles
    fn test_insufficient_data(#[case] candle_count: usize, #[case] expected_available: usize) {
        let detector = HammerDetector::new();
        let candles = create_test_candles(candle_count);
        let params = PatternParams::default();

        let result = detector.detect(&candles, &params);
        assert!(matches!(
            result,
            Err(PatternError::InsufficientData { available, .. }) if available == expected_available
        ));
    }
}
```

### Integration Testing

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use daytrader::brokers::MockBroker;

    #[tokio::test]
    async fn test_complete_trading_workflow() {
        // Setup components
        let broker = Arc::new(MockBroker::new());
        let risk_engine = create_test_risk_engine();
        let pattern_registry = create_test_pattern_registry().await;

        // Simulate market data
        let candles = generate_test_candles();

        // Test pattern detection
        let patterns = pattern_registry.detect_all(&candles, None).await;
        assert!(!patterns.is_empty());

        // Test risk validation
        for pattern in patterns {
            if let Ok(order) = create_order_from_pattern(&pattern) {
                let validation_result = risk_engine.validate_order(&order).await;
                // Assert validation logic
            }
        }
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn pattern_confidence_always_valid(confidence in 0.0..1.0) {
        let confidence = Confidence::new(confidence).unwrap();
        prop_assert!(confidence.value() >= 0.0 && confidence.value() <= 1.0);
    }

    #[test]
    fn price_always_positive(value in 0.01f64..1000000.0) {
        let decimal_value = Decimal::from_f64(value).unwrap();
        let price = Price::new(decimal_value).unwrap();
        prop_assert!(price.value() > Decimal::ZERO);
    }
}
```

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_pattern_detection

# Integration tests only
cargo test --test integration

# With coverage
cargo tarpaulin --out Html

# Benchmarks
cargo bench
```

## Deployment

### Development Deployment

```bash
# Debug build
cargo build
./target/debug/daytrader

# With logging
RUST_LOG=debug ./target/debug/daytrader
```

### Production Deployment

```bash
# Optimized build
cargo build --release

# Run with production config
./target/release/daytrader --config config/production.toml

# Or use environment variables
ALPACA_API_KEY=prod_key \
ALPACA_API_SECRET=prod_secret \
./target/release/daytrader
```

### Docker Deployment

```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y sqlite3 ca-certificates
COPY --from=builder /app/target/release/daytrader /usr/local/bin/
CMD ["daytrader"]
```

## Resources

### Documentation

- [API Reference](api-reference.md) - Complete API documentation
- [Examples Guide](examples-guide.md) - Usage examples and patterns
- [Performance Guide](performance-guide.md) - Performance optimization
- [Troubleshooting Guide](troubleshooting.md) - Common issues and solutions
- [Security Guidelines](SECURITY.md) - Security best practices

### External Resources

- **Rust Documentation**: [The Rust Programming Language](https://doc.rust-lang.org/book/)
- **Async Rust**: [Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/)
- **Tokio**: [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- **Trading Patterns**: [Candlestick Patterns Guide](https://www.investopedia.com/terms/c/candlestick.asp)

### Community

- **GitHub Issues**: Report bugs and request features
- **GitHub Discussions**: Ask questions and discuss ideas
- **Code Reviews**: Learn from reviewing others' code

### Development Tools

```bash
# Essential tools
cargo install cargo-edit      # Easy dependency management
cargo install cargo-watch     # Auto-rebuild on changes
cargo install cargo-expand    # Macro expansion
cargo install cargo-udeps     # Unused dependencies

# Performance tools
cargo install flamegraph      # Flame graph profiling
cargo install heaptrack       # Memory profiling

# Development helpers
cargo install cargo-tree      # Dependency tree
cargo install cargo-modules   # Module structure analysis
```

## Getting Help

### When You're Stuck

1. **Check Documentation**: Search existing docs first
2. **Search Issues**: Look for similar problems on GitHub
3. **Ask Questions**: Use GitHub discussions for questions
4. **Debug Together**: Share minimal reproduction cases

### Code Review Guidelines

**For Authors:**
- Write clear commit messages
- Include tests for new functionality
- Update documentation
- Consider edge cases

**For Reviewers:**
- Be constructive and specific
- Focus on code quality and correctness
- Suggest improvements, don't dictate
- Approve when standards are met

### Performance Expectations

- **Pattern Detection**: < 1ms per pattern
- **Memory Usage**: < 100MB for normal operation
- **Test Coverage**: > 80% for new code
- **Build Time**: < 30 seconds for incremental builds

Welcome aboard! The Day Trading Platform is a complex but rewarding project. Take your time to understand the codebase, ask questions, and contribute meaningfully. Happy coding! 🚀</content>
</xai:function_call">The developer onboarding guide has been created successfully. It provides comprehensive information for new developers to:

- Set up their development environment
- Understand the project structure and architecture
- Learn the development workflow and best practices
- Get familiar with key concepts and patterns
- Start contributing with examples
- Understand testing and deployment processes
- Access resources and get help

This guide serves as a complete introduction for developers joining the project.