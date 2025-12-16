# Rust Design Patterns & Performance Guidelines

**Version:** 1.0  
**Date:** December 16, 2025  
**Focus:** Functional programming, zero-cost abstractions, and performance optimization

---

## Core Design Principles

### 1. Functional Programming with Higher-Order Functions

**Principle:** Push side effects to the boundaries, keep core logic pure and composable.

```rust
// ❌ BAD: Imperative style with side effects throughout
pub fn process_candle(candle: Candle, db: &Database) -> Result<()> {
    // Side effect: database write
    db.insert_candle(&candle)?;
    
    // Side effect: pattern detection with I/O
    let patterns = detect_patterns(&candle)?;
    
    // Side effect: more database writes
    for pattern in patterns {
        db.insert_pattern(&pattern)?;
    }
    
    Ok(())
}

// ✅ GOOD: Functional style with side effects at boundaries
pub fn process_candle(candle: Candle) -> impl Iterator<Item = PatternSignal> {
    // Pure function: no side effects
    detect_patterns(candle)
        .filter(|signal| signal.confidence > 0.7)
        .map(|signal| enrich_signal(signal))
}

// Side effects only at the boundary
pub async fn handle_candle(candle: Candle, db: &Database) -> Result<()> {
    // Pure computation
    let signals: Vec<_> = process_candle(candle).collect();
    
    // Side effect: single batch write at boundary
    db.batch_insert_signals(&signals).await?;
    
    Ok(())
}
```

### 2. Lazy Evaluation with Iterators

**Principle:** Use iterators for lazy, composable data transformations.

```rust
// ❌ BAD: Eager evaluation with intermediate allocations
pub fn filter_and_transform_candles(candles: Vec<Candle>) -> Vec<ProcessedCandle> {
    let mut filtered = Vec::new();
    for candle in candles {
        if candle.volume > 1000 {
            filtered.push(candle);
        }
    }
    
    let mut transformed = Vec::new();
    for candle in filtered {
        transformed.push(transform_candle(candle));
    }
    
    transformed
}

// ✅ GOOD: Lazy evaluation with zero intermediate allocations
pub fn filter_and_transform_candles(
    candles: impl Iterator<Item = Candle>
) -> impl Iterator<Item = ProcessedCandle> {
    candles
        .filter(|c| c.volume > 1000)
        .map(transform_candle)
}

// Collect only at the boundary
let results: Vec<_> = filter_and_transform_candles(candles.into_iter()).collect();
```

### 3. Function Composition

**Principle:** Build complex operations from simple, composable functions.

```rust
// Pure, composable functions
fn calculate_sma(prices: &[Decimal], period: usize) -> Option<Decimal> {
    if prices.len() < period {
        return None;
    }
    
    let sum: Decimal = prices.iter().rev().take(period).sum();
    Some(sum / Decimal::from(period))
}

fn calculate_rsi(prices: &[Decimal], period: usize) -> Option<Decimal> {
    // Pure calculation
    // ...
}

fn is_oversold(rsi: Decimal) -> bool {
    rsi < Decimal::new(30, 0)
}

fn is_overbought(rsi: Decimal) -> bool {
    rsi > Decimal::new(70, 0)
}

// Compose functions
pub fn generate_signal(candles: &[Candle]) -> Option<Signal> {
    let prices: Vec<_> = candles.iter().map(|c| c.close).collect();
    
    let sma = calculate_sma(&prices, 20)?;
    let rsi = calculate_rsi(&prices, 14)?;
    let current_price = candles.last()?.close;
    
    // Functional composition
    let signal_type = match (current_price > sma, is_oversold(rsi), is_overbought(rsi)) {
        (true, true, false) => SignalType::Buy,
        (false, false, true) => SignalType::Sell,
        _ => return None,
    };
    
    Some(Signal {
        signal_type,
        confidence: calculate_confidence(sma, rsi, current_price),
        timestamp: Utc::now(),
    })
}
```

---

## Error Handling with thiserror

### Newtype Pattern for Domain Errors

```rust
// src/utils/errors.rs

use thiserror::Error;
use rust_decimal::Decimal;

/// Application result type
pub type Result<T> = std::result::Result<T, AppError>;

/// Top-level application error
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Broker error: {0}")]
    Broker(#[from] BrokerError),
    
    #[error("Risk violation: {0}")]
    Risk(#[from] RiskError),
    
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    
    #[error("Pattern detection error: {0}")]
    Pattern(#[from] PatternError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Broker-specific errors
#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Order rejected: {reason}")]
    OrderRejected { reason: String },
    
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: Decimal, available: Decimal },
    
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    
    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimitExceeded { retry_after: std::time::Duration },
    
    #[error("API error: {0}")]
    ApiError(String),
}

/// Risk management errors
#[derive(Debug, Error)]
pub enum RiskError {
    #[error("Daily loss limit exceeded: current loss ${current_loss}, limit ${limit}")]
    DailyLossLimitExceeded { current_loss: Decimal, limit: Decimal },
    
    #[error("Position size too large: ${order_size} exceeds limit of ${limit}")]
    PositionSizeTooLarge { order_size: Decimal, limit: Decimal },
    
    #[error("PDT limit reached: {current_count} day trades, account value ${account_value} (need ${required_value})")]
    PdtLimitReached {
        current_count: u32,
        account_value: Decimal,
        required_value: Decimal,
    },
    
    #[error("Pattern disabled: {pattern}")]
    PatternDisabled { pattern: String },
    
    #[error("Account below minimum balance: ${current} (minimum: ${minimum})")]
    BelowMinimumBalance { current: Decimal, minimum: Decimal },
}

/// Database errors
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Query failed: {0}")]
    QueryFailed(String),
    
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
    
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// Pattern detection errors
#[derive(Debug, Error)]
pub enum PatternError {
    #[error("Insufficient data: need {required} candles, have {available}")]
    InsufficientData { required: usize, available: usize },
    
    #[error("Invalid candle data: {0}")]
    InvalidCandleData(String),
    
    #[error("Calculation error: {0}")]
    CalculationError(String),
}
```

### Usage Example

```rust
use crate::utils::errors::{Result, BrokerError, RiskError};

pub fn place_order(order: Order, account: &Account) -> Result<OrderId> {
    // Risk validation
    validate_risk(&order, account)?;  // Returns Result<(), RiskError>
    
    // Broker submission
    submit_to_broker(&order)?;  // Returns Result<OrderId, BrokerError>
    
    Ok(order_id)
}

fn validate_risk(order: &Order, account: &Account) -> Result<()> {
    if account.buying_power < order.total_value() {
        return Err(RiskError::InsufficientFunds {
            required: order.total_value(),
            available: account.buying_power,
        }.into());
    }
    
    Ok(())
}
```

---

## Newtype Pattern for Type Safety

### Domain-Specific Types

```rust
// src/types/mod.rs

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Newtype for stock symbols
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Result<Self, &'static str> {
        let s = s.into();
        if s.is_empty() || s.len() > 10 {
            return Err("Invalid symbol length");
        }
        Ok(Self(s.to_uppercase()))
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for prices (always positive)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Price(Decimal);

impl Price {
    pub fn new(value: Decimal) -> Result<Self, &'static str> {
        if value <= Decimal::ZERO {
            return Err("Price must be positive");
        }
        Ok(Self(value))
    }
    
    pub fn value(&self) -> Decimal {
        self.0
    }
}

/// Newtype for quantities (always positive)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Quantity(u32);

impl Quantity {
    pub fn new(value: u32) -> Result<Self, &'static str> {
        if value == 0 {
            return Err("Quantity must be positive");
        }
        Ok(Self(value))
    }
    
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Newtype for confidence scores (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !(0.0..=1.0).contains(&value) {
            return Err("Confidence must be between 0.0 and 1.0");
        }
        Ok(Self(value))
    }
    
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Newtype for order IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(String);

impl OrderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

### Usage with Type Safety

```rust
use crate::types::{Symbol, Price, Quantity, OrderId};

pub struct Order {
    pub id: OrderId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub price: Option<Price>,  // None for market orders
}

impl Order {
    pub fn new_market_order(
        symbol: Symbol,
        side: OrderSide,
        quantity: Quantity,
    ) -> Self {
        Self {
            id: OrderId::new(uuid::Uuid::new_v4().to_string()),
            symbol,
            side,
            quantity,
            price: None,
        }
    }
    
    pub fn new_limit_order(
        symbol: Symbol,
        side: OrderSide,
        quantity: Quantity,
        price: Price,
    ) -> Self {
        Self {
            id: OrderId::new(uuid::Uuid::new_v4().to_string()),
            symbol,
            side,
            quantity,
            price: Some(price),
        }
    }
    
    // Type-safe calculations
    pub fn total_value(&self) -> Decimal {
        let price = self.price.map(|p| p.value()).unwrap_or(Decimal::ZERO);
        price * Decimal::from(self.quantity.value())
    }
}

// Compile-time type safety
let symbol = Symbol::new("AAPL")?;
let quantity = Quantity::new(100)?;
let price = Price::new(Decimal::new(15000, 2))?;  // $150.00

let order = Order::new_limit_order(symbol, OrderSide::Buy, quantity, price);

// ❌ This won't compile: type mismatch
// let order = Order::new_limit_order("AAPL", 100, 150.0);
```

---

## Avoiding Dynamic Dispatch

### Static Dispatch with Generics

```rust
// ❌ BAD: Dynamic dispatch with trait objects
pub struct OrderExecutor {
    broker: Box<dyn BrokerProvider>,  // Dynamic dispatch
}

impl OrderExecutor {
    pub async fn execute(&self, order: Order) -> Result<OrderId> {
        self.broker.place_order(order).await  // Virtual function call
    }
}

// ✅ GOOD: Static dispatch with generics
pub struct OrderExecutor<B: BrokerProvider> {
    broker: B,  // Concrete type, no vtable
}

impl<B: BrokerProvider> OrderExecutor<B> {
    pub async fn execute(&self, order: Order) -> Result<OrderId> {
        self.broker.place_order(order).await  // Direct function call, inlined
    }
}

// Usage
let alpaca = AlpacaProvider::new(api_key, api_secret);
let executor = OrderExecutor { broker: alpaca };
```

### Enum Dispatch for Known Types

```rust
// When you have a fixed set of types, use enums instead of trait objects

// ❌ BAD: Dynamic dispatch
pub enum BrokerType {
    Alpaca(Box<dyn BrokerProvider>),
    InteractiveBrokers(Box<dyn BrokerProvider>),
}

// ✅ GOOD: Enum dispatch (zero-cost)
pub enum Broker {
    Alpaca(AlpacaProvider),
    InteractiveBrokers(IbkrProvider),
}

impl Broker {
    pub async fn place_order(&self, order: Order) -> Result<OrderId> {
        match self {
            Broker::Alpaca(provider) => provider.place_order(order).await,
            Broker::InteractiveBrokers(provider) => provider.place_order(order).await,
        }
    }
}

// Compiler can optimize this to direct calls
```

### Compile-Time Polymorphism

```rust
// Use const generics and type-level programming

pub trait PatternDetector {
    const NAME: &'static str;
    fn detect(&self, candles: &[Candle]) -> Option<PatternSignal>;
}

pub struct HammerDetector;

impl PatternDetector for HammerDetector {
    const NAME: &'static str = "Hammer";
    
    fn detect(&self, candles: &[Candle]) -> Option<PatternSignal> {
        // Detection logic
    }
}

// Generic function with static dispatch
pub fn detect_pattern<P: PatternDetector>(
    detector: &P,
    candles: &[Candle],
) -> Option<PatternSignal> {
    detector.detect(candles)
}

// Compiler generates specialized code for each detector type
let hammer = HammerDetector;
let signal = detect_pattern(&hammer, &candles);  // No virtual call
```

---

## Performance Optimization Patterns

### 1. Inline Hot Paths

```rust
// Mark hot functions for inlining
#[inline]
pub fn calculate_body_size(candle: &Candle) -> Decimal {
    (candle.close - candle.open).abs()
}

#[inline(always)]  // Force inline for critical paths
pub fn is_bullish(candle: &Candle) -> bool {
    candle.close > candle.open
}
```

### 2. Avoid Allocations in Hot Loops

```rust
// ❌ BAD: Allocates on every iteration
pub fn process_candles(candles: &[Candle]) -> Vec<Signal> {
    let mut signals = Vec::new();
    for candle in candles {
        let signal = detect_pattern(candle);  // Allocates String inside
        if let Some(s) = signal {
            signals.push(s);
        }
    }
    signals
}

// ✅ GOOD: Pre-allocate and reuse
pub fn process_candles(candles: &[Candle]) -> Vec<Signal> {
    let mut signals = Vec::with_capacity(candles.len() / 10);  // Estimate
    for candle in candles {
        if let Some(signal) = detect_pattern(candle) {
            signals.push(signal);
        }
    }
    signals
}
```

### 3. Use SmallVec for Small Collections

```rust
use smallvec::{SmallVec, smallvec};

// Most patterns use 1-3 candles, avoid heap allocation
pub type CandleWindow = SmallVec<[Candle; 3]>;

pub fn get_recent_candles(candles: &[Candle], count: usize) -> CandleWindow {
    candles.iter()
        .rev()
        .take(count)
        .cloned()
        .collect()
}
```

### 4. Const Functions for Compile-Time Computation

```rust
// Compute at compile time
pub const fn calculate_period_multiplier(period: usize) -> f64 {
    2.0 / (period as f64 + 1.0)
}

// Used at compile time
const EMA_12_MULTIPLIER: f64 = calculate_period_multiplier(12);
const EMA_26_MULTIPLIER: f64 = calculate_period_multiplier(26);
```

### 5. Zero-Copy with Cow

```rust
use std::borrow::Cow;

pub fn normalize_symbol(symbol: &str) -> Cow<str> {
    if symbol.chars().all(|c| c.is_uppercase()) {
        Cow::Borrowed(symbol)  // No allocation
    } else {
        Cow::Owned(symbol.to_uppercase())  // Allocate only if needed
    }
}
```

---

## Functional Patterns for Side Effects

### Reader Pattern for Configuration

```rust
// Pure functions that take configuration as parameter
pub struct RiskConfig {
    pub max_daily_loss: Decimal,
    pub max_position_size: Decimal,
}

// Pure function
pub fn validate_order(order: &Order, account: &Account, config: &RiskConfig) -> Result<()> {
    if order.total_value() > config.max_position_size {
        return Err(RiskError::PositionSizeTooLarge {
            order_size: order.total_value(),
            limit: config.max_position_size,
        }.into());
    }
    Ok(())
}

// Side effects at boundary
pub async fn handle_order(order: Order, config: &RiskConfig, db: &Database) -> Result<()> {
    let account = db.get_account().await?;
    
    // Pure validation
    validate_order(&order, &account, config)?;
    
    // Side effect: database write
    db.insert_order(&order).await?;
    
    Ok(())
}
```

### State Pattern with Immutable Updates

```rust
// Immutable state updates
#[derive(Clone)]
pub struct TradingState {
    pub positions: Vec<Position>,
    pub daily_pl: Decimal,
    pub trades_today: u32,
}

impl TradingState {
    // Pure function: returns new state
    pub fn with_new_position(mut self, position: Position) -> Self {
        self.positions.push(position);
        self
    }
    
    // Pure function: returns new state
    pub fn with_updated_pl(mut self, pl: Decimal) -> Self {
        self.daily_pl += pl;
        self
    }
}

// Functional state updates
let state = TradingState::default();
let state = state.with_new_position(position);
let state = state.with_updated_pl(profit);
```

---

## Async Patterns

### Avoid Blocking in Async Context

```rust
// ❌ BAD: Blocking in async function
pub async fn process_data(data: Vec<u8>) -> Result<()> {
    let result = expensive_computation(data);  // Blocks executor
    Ok(result)
}

// ✅ GOOD: Use spawn_blocking for CPU-intensive work
pub async fn process_data(data: Vec<u8>) -> Result<()> {
    let result = tokio::task::spawn_blocking(move || {
        expensive_computation(data)
    }).await??;
    Ok(result)
}
```

### Batch Operations

```rust
// ❌ BAD: Individual async operations
pub async fn save_signals(signals: Vec<Signal>, db: &Database) -> Result<()> {
    for signal in signals {
        db.insert_signal(&signal).await?;  // N database calls
    }
    Ok(())
}

// ✅ GOOD: Batch operation
pub async fn save_signals(signals: Vec<Signal>, db: &Database) -> Result<()> {
    db.batch_insert_signals(&signals).await?;  // 1 database call
    Ok(())
}
```

---

## Complete Example: Pattern Detection Pipeline

```rust
// src/patterns/pipeline.rs

use crate::types::{Symbol, Confidence};
use crate::utils::errors::{Result, PatternError};

/// Pure function: no side effects
#[inline]
fn calculate_body_size(candle: &Candle) -> Decimal {
    (candle.close - candle.open).abs()
}

/// Pure function: no side effects
#[inline]
fn calculate_wick_ratio(candle: &Candle) -> Decimal {
    let body = calculate_body_size(candle);
    let lower_wick = candle.open.min(candle.close) - candle.low;
    if body.is_zero() {
        Decimal::ZERO
    } else {
        lower_wick / body
    }
}

/// Pure pattern detection
pub fn detect_hammer(candle: &Candle) -> Option<PatternSignal> {
    let body_size = calculate_body_size(candle);
    let total_range = candle.high - candle.low;
    let wick_ratio = calculate_wick_ratio(candle);
    
    if body_size / total_range < Decimal::new(30, 2)
        && wick_ratio >= Decimal::new(2, 0)
    {
        Some(PatternSignal {
            pattern_name: "Hammer".to_string(),
            signal_type: SignalType::Bullish,
            confidence: Confidence::new(0.7).unwrap(),
            detected_at: Utc::now(),
            price: candle.close,
        })
    } else {
        None
    }
}

/// Composable pattern detection pipeline
pub fn detect_all_patterns(candle: &Candle) -> impl Iterator<Item = PatternSignal> {
    [
        detect_hammer(candle),
        detect_shooting_star(candle),
        detect_doji(candle),
    ]
    .into_iter()
    .flatten()  // Filter out None values
}

/// Filter by confidence threshold
pub fn filter_by_confidence(
    signals: impl Iterator<Item = PatternSignal>,
    min_confidence: Confidence,
) -> impl Iterator<Item = PatternSignal> {
    signals.filter(move |s| s.confidence >= min_confidence)
}

/// Enrich signals with additional data (pure function)
pub fn enrich_signal(signal: PatternSignal, indicators: &Indicators) -> EnrichedSignal {
    EnrichedSignal {
        signal,
        rsi: indicators.rsi,
        macd: indicators.macd,
        sma: indicators.sma,
    }
}

/// Complete pipeline: pure until the boundary
pub fn process_candle_pipeline(
    candle: Candle,
    indicators: Indicators,
    min_confidence: Confidence,
) -> impl Iterator<Item = EnrichedSignal> {
    detect_all_patterns(&candle)
        .filter(move |s| s.confidence >= min_confidence)
        .map(move |s| enrich_signal(s, &indicators))
}

/// Side effects only at the boundary
pub async fn handle_new_candle(
    candle: Candle,
    indicators: Indicators,
    config: &Config,
    db: &Database,
) -> Result<()> {
    // Pure computation
    let signals: Vec<_> = process_candle_pipeline(
        candle,
        indicators,
        config.min_confidence,
    ).collect();
    
    // Side effect: single batch write
    if !signals.is_empty() {
        db.batch_insert_signals(&signals).await?;
    }
    
    Ok(())
}
```

---

## Performance Checklist

### Compile-Time Optimizations

- [ ] Use `#[inline]` for hot functions
- [ ] Use `const fn` for compile-time computation
- [ ] Use generics instead of trait objects
- [ ] Use enum dispatch for known types
- [ ] Enable LTO in release profile

### Runtime Optimizations

- [ ] Pre-allocate collections with `with_capacity`
- [ ] Use `SmallVec` for small collections
- [ ] Avoid allocations in hot loops
- [ ] Use `Cow` for conditional cloning
- [ ] Batch database operations
- [ ] Use `spawn_blocking` for CPU-intensive work

### Memory Optimizations

- [ ] Use references instead of clones
- [ ] Use `Arc` for shared immutable data
- [ ] Use `Rc` for single-threaded shared data
- [ ] Avoid `Box<dyn Trait>` in hot paths
- [ ] Use stack allocation when possible

---

## Cargo.toml Optimizations

```toml
[profile.release]
opt-level = 3              # Maximum optimization
lto = "fat"                # Link-time optimization
codegen-units = 1          # Better optimization, slower compile
strip = true               # Remove debug symbols
panic = "abort"            # Smaller binary, faster panic

[profile.dev]
opt-level = 0              # Fast compilation
debug = true               # Debug symbols
incremental = true         # Faster recompilation

[profile.bench]
inherits = "release"
debug = true               # Debug symbols for profiling
```

---

## Summary

### Key Principles

1. **Functional Core, Imperative Shell**
   - Pure functions in the core
   - Side effects at boundaries

2. **Static Dispatch**
   - Use generics, not trait objects
   - Use enum dispatch for known types

3. **Type Safety**
   - Newtype pattern for domain types
   - thiserror for structured errors

4. **Zero-Cost Abstractions**
   - Iterators for lazy evaluation
   - Inline hot functions
   - Const functions for compile-time computation

5. **Performance**
   - Avoid allocations in hot paths
   - Batch operations
   - Profile before optimizing

This approach provides maximum performance while maintaining code clarity and type safety.

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Next Review:** After Phase 1 implementation
