# DayTrader — Idiomatic Rust Coding Standards

**Version**: 1.0  
**Scope**: All Rust code in `src/` and `daytrader/src-tauri/src/`  
**Authority**: These rules are machine-verified by `./scripts/verify_agent_work.sh`. Violations block commits.

This document is the single source of truth for coding conventions in the DayTrader project. Every rule has a rationale, compliant examples, non-compliant counter-examples, and references to real files in this codebase.

---

## Table of Contents

1. [Newtype Pattern](#1-newtype-pattern)
2. [Immutability](#2-immutability)
3. [Functional Paradigms and Pattern Matching](#3-functional-paradigms-and-pattern-matching)
4. [Error Propagation with `thiserror`](#4-error-propagation-with-thiserror)
5. [Static Dispatch by Default](#5-static-dispatch-by-default)
6. [Trait Boundaries on All Public APIs](#6-trait-boundaries-on-all-public-apis)
7. [Async and Concurrency](#7-async-and-concurrency)
8. [Documentation Standards](#8-documentation-standards)

---

## 1. Newtype Pattern

### Rule

**ALL domain values that are primitives MUST be wrapped in newtypes.** Never use `f64`, `u32`, `i64`, `String`, or `bool` as public API parameters or struct fields for domain concepts.

**Rationale**: Raw primitives allow passing a `max_daily_loss_dollars: f64` where `win_rate: f64` is expected. The compiler cannot catch this. Newtypes make the type system enforce domain invariants at zero runtime cost.

### Existing Newtypes in This Codebase

| Newtype | Inner type | Location | Validation |
|---------|-----------|----------|-----------|
| `Symbol` | `String` | `src/brokers/types.rs` | Non-empty, ≤10 chars, uppercase |
| `Price` | `Decimal` | `src/brokers/types.rs` | Strictly positive |
| `Quantity` | `u32` | `src/brokers/types.rs` | Strictly positive (>0) |
| `Confidence` | `f64` | `src/brokers/types.rs` | 0.0–1.0 inclusive |
| `OrderId` | `String` | `src/brokers/types.rs` | Infallible |
| `PatternName` | `String` | `src/brokers/types.rs` | Infallible |
| `Reason` | `String` | `src/brokers/types.rs` | Infallible |
| `Message` | `String` | `src/brokers/types.rs` | Infallible |
| `Notes` | `String` | `src/brokers/types.rs` | Infallible |
| `DollarAmount` | `Decimal` | `src/risk/types.rs` | Non-negative |
| `Percentage` | `Decimal` | `src/risk/types.rs` | 0.0–100.0 inclusive |
| `Count` | `u32` | `src/risk/types.rs` | Infallible |
| `AccountId` | `String` | `src/risk/types.rs` | Infallible |
| `CorrelationId` | `String` | `src/utils/error_context.rs` | Infallible |

### Missing Newtypes (Must Be Created)

| Needed as | Use instead | Notes |
|-----------|-------------|-------|
| `RiskMultiplier(f64)` | — | New, range 0.0–10.0. Display as `{}x` |
| `WinRate(f64)` | `Percentage` | Use `Percentage` — same semantics |
| `WindowDays(u32)` | — | New, infallible. Display as `{} days` |
| `PerformanceScore(f64)` | — | New, range 0.0–1.0 |
| `VolumeRatio(f64)` | — | New, range 0.0–100.0 |
| `LatencyMs(f64)` | — | New, non-negative. Display as `{}ms` |
| `TradeCount(u32)` | `Count` | Use `Count` — same semantics |

### Newtype Construction Rules

Every newtype MUST:

1. Implement `new()` that returns `Result<Self, &'static str>` for validated types, or an infallible constructor for unvalidated identifiers.
2. Expose a `value()` accessor for numeric types or `as_str()` / `into_inner()` for string types.
3. Derive `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize` where applicable (omit `Copy` for heap-allocated types; omit `Eq`/`Ord` for `f64`-based types).
4. Implement `Display`.
5. Implement `From`/`TryFrom` conversions for ergonomic use at call sites.

### Canonical Example — `DollarAmount` (`src/risk/types.rs`)

```rust
/// Newtype for dollar amounts (risk limits, balances)
///
/// Ensures dollar amounts are non-negative and uses Decimal for precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DollarAmount(Decimal);

impl DollarAmount {
    /// Creates a new dollar amount
    ///
    /// # Errors
    ///
    /// Returns an error if the value is negative
    pub fn new(value: Decimal) -> Result<Self, &'static str> {
        if value < Decimal::ZERO {
            return Err("Dollar amount cannot be negative");
        }
        Ok(Self(value))
    }

    /// Returns the dollar amount value
    pub fn value(&self) -> Decimal { self.0 }

    /// Returns the amount as f64 (use sparingly, prefer Decimal)
    pub fn as_f64(&self) -> f64 {
        self.0.to_string().parse().unwrap_or(0.0)
    }
}

impl fmt::Display for DollarAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
```

### Template for a New Newtype

```rust
/// Newtype for <description>
///
/// <Invariant being enforced>
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RiskMultiplier(f64);

impl RiskMultiplier {
    /// Creates a new risk multiplier
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not in the range 0.0–10.0
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !(0.0..=10.0).contains(&value) {
            return Err("RiskMultiplier must be between 0.0 and 10.0");
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> f64 { self.0 }

    /// Returns the identity multiplier (1.0)
    pub fn one() -> Self { Self(1.0) }

    /// Returns true if this multiplier has no effect
    pub fn is_identity(&self) -> bool { (self.0 - 1.0).abs() < f64::EPSILON }
}

impl fmt::Display for RiskMultiplier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x", self.0)
    }
}

impl TryFrom<f64> for RiskMultiplier {
    type Error = &'static str;
    fn try_from(v: f64) -> Result<Self, Self::Error> { Self::new(v) }
}

impl From<RiskMultiplier> for f64 {
    fn from(r: RiskMultiplier) -> f64 { r.0 }
}
```

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — domain field uses newtype
pub struct PatternRiskProfile {
    pub min_confidence_threshold: Confidence,
    pub risk_multiplier: RiskMultiplier,
}

// ❌ NON-COMPLIANT — raw primitives allow type confusion
pub struct PatternRiskProfile {
    pub min_confidence_threshold: f64,  // Is this 0.0-1.0 or 0.0-100.0?
    pub risk_multiplier: f64,           // Compiler won't catch swapped args
}
```

```rust
// ✅ COMPLIANT — function signature uses newtypes
pub fn evaluate_pattern(
    symbol: &Symbol,
    confidence: Confidence,
    win_rate: Percentage,
) -> Result<RiskEvaluation, RiskError> { ... }

// ❌ NON-COMPLIANT — raw primitives; callers can pass any f64
pub fn evaluate_pattern(
    symbol: &str,
    confidence: f64,
    win_rate: f64,
) -> Result<RiskEvaluation, String> { ... }
```

---

## 2. Immutability

### Rules

1. **All bindings are `let` (immutable) by default.** Use `let mut` only when mutation is genuinely required and cannot be avoided with functional combinators.
2. **Prefer `&T` over `&mut T`** in function signatures wherever the function does not need to mutate.
3. **Struct fields must not be `pub mut`.** There is no `pub mut` in Rust — do not use `Cell<T>` or `RefCell<T>` as a workaround to create mutably-accessible public fields. Mutation goes through explicit `&mut self` methods.
4. **Never use `Cell`, `RefCell`, or interior mutability** unless crossing async boundaries where `Arc<Mutex<T>>` is the documented pattern (see §7).

### Acceptable Uses of `let mut`

The following patterns genuinely require mutation and are allowed:

- **Accumulating into a `Vec` inside a local function scope** before returning (but prefer `.collect()` when possible).
- **`EventBus` internal state** — broadcast channel subscriptions require mutable access.
- **`PatternRegistry::register()`** — inserting into the pattern map requires `&mut self`.
- **EMA / RSI calculations** in `src/patterns/indicators.rs` — the rolling `prev_ema` accumulator is inherently stateful and `.scan()` is the idiomatic form (see §3).

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — immutable binding, functional style
let total_pnl: Decimal = positions
    .iter()
    .map(|p| p.unrealized_pnl())
    .sum();

// ❌ NON-COMPLIANT — unnecessary mutation
let mut total_pnl = Decimal::ZERO;
for p in &positions {
    total_pnl += p.unrealized_pnl();
}
```

```rust
// ✅ COMPLIANT — read-only borrow
pub fn calculate_pnl(&self, current_price: Price) -> Decimal { ... }

// ❌ NON-COMPLIANT — unneeded mutable borrow
pub fn calculate_pnl(&mut self, current_price: Price) -> Decimal { ... }
```

```rust
// ✅ COMPLIANT — mutation via explicit method
impl RiskEngine {
    pub fn update_equity(&mut self, new_equity: DollarAmount) {
        self.current_equity = new_equity;
    }
}

// ❌ NON-COMPLIANT — public field with interior mutation workaround
pub struct RiskEngine {
    pub current_equity: Cell<f64>,  // Bypasses borrow checker, hides mutation
}
```

---

## 3. Functional Paradigms and Pattern Matching

### Iterator Chains Over Imperative Loops

**Rule**: Prefer iterator chains over imperative `for` loops when building collections or computing aggregates.

The full set of approved combinators:
`.map()`, `.filter()`, `.filter_map()`, `.fold()`, `.flat_map()`, `.zip()`, `.enumerate()`, `.windows()`, `.chunks()`, `.scan()`, `.take_while()`, `.skip_while()`, `.any()`, `.all()`, `.position()`, `.find_map()`, `.sum()`, `.product()`, `.collect()`, `.count()`, `.min()`, `.max()`, `.min_by()`, `.max_by()`, `.flatten()`, `.chain()`, `.peekable()`, `.inspect()`, `.retain()`

#### Transformation Table

| Imperative pattern | Idiomatic replacement |
|--------------------|-----------------------|
| Loop pushing to `Vec` | `.map().collect()` |
| Loop filtering into `Vec` | `.filter().collect()` or `.filter_map().collect()` |
| Loop accumulating scalar | `.fold()` or `.sum()` / `.product()` |
| Loop over sliding window | `.windows(n).map(...)` |
| Stateful loop (EMA, RSI) | `.scan(initial, \|state, item\| ...)` |
| Pairing consecutive items | `.windows(2)` or `.zip()` |
| Early return on condition | `.find()`, `.any()`, `.all()`, `.find_map()` |
| Building a `HashMap` | `.map(|(k, v)| ...).collect::<HashMap<_, _>>()` |
| Pruning a collection | `.retain(\|x\| ...)` |

#### Anti-Pattern: For-Loop in `sma()` (`src/patterns/indicators.rs:79`)

```rust
// ❌ NON-COMPLIANT — imperative loop in src/patterns/indicators.rs:77-86
let mut result = Vec::with_capacity(prices.len().saturating_sub(period - 1));
for i in (period - 1)..prices.len() {
    let sum: Decimal = prices[i.saturating_sub(period - 1)..=i]
        .iter()
        .sum();
    result.push(sum / Decimal::from(period));
}
Ok(result)

// ✅ COMPLIANT — idiomatic replacement with .windows()
let period_decimal = Decimal::from(period);
Ok(prices
    .windows(period)
    .map(|window| window.iter().sum::<Decimal>() / period_decimal)
    .collect())
```

#### Anti-Pattern: Stateful EMA Loop (`src/patterns/indicators.rs:117-121`)

```rust
// ❌ NON-COMPLIANT — explicit mutation of prev_ema
let mut prev_ema = first_sma;
for &price in &prices[period..] {
    let current_ema = (price * multiplier) + (prev_ema * (Decimal::ONE - multiplier));
    result.push(current_ema);
    prev_ema = current_ema;
}

// ✅ COMPLIANT — .scan() carries state without explicit mutation
let emas: Vec<Decimal> = prices[period..]
    .iter()
    .scan(first_sma, |prev, &price| {
        let ema = price * multiplier + *prev * (Decimal::ONE - multiplier);
        *prev = ema;
        Some(ema)
    })
    .collect();
```

### Pattern Matching Rules

**Rule**: Use `match` exhaustively. Every `match` arm must be explicit — no catch-all `_` unless the variant truly does not matter and the reason is documented.

**Rule**: When matching `Option`/`Result`, prefer the combinator form over `match` where it is cleaner:

| Instead of `match` | Prefer |
|--------------------|--------|
| `match opt { Some(v) => f(v), None => default }` | `opt.map(f).unwrap_or(default)` |
| `match opt { Some(v) => Ok(v), None => Err(e) }` | `opt.ok_or_else(\|\| e)` |
| `match result { Ok(v) => g(v), Err(e) => Err(e) }` | `result.and_then(g)` or `result.map(g)?` |
| `match result { Ok(v) => v, Err(_) => default }` | `result.unwrap_or(default)` |

```rust
// ✅ COMPLIANT — exhaustive match, no catch-all
match risk_error {
    RiskError::DailyLossLimitExceeded { current_loss, limit } => {
        tracing::warn!("Loss limit hit: {} / {}", current_loss, limit);
        halt_trading()
    }
    RiskError::PositionSizeTooLarge { order_size, limit } => {
        reduce_size(order_size, limit)
    }
    RiskError::PdtLimitReached { current_count, .. } => {
        block_day_trades(current_count)
    }
    RiskError::PatternDisabled { pattern } => disable_pattern(pattern),
    RiskError::BelowMinimumBalance { current, minimum } => {
        emergency_halt(current, minimum)
    }
}

// ❌ NON-COMPLIANT — silently swallows new variants
match risk_error {
    RiskError::DailyLossLimitExceeded { .. } => halt_trading(),
    _ => {} // New variants added in the future will be silently ignored
}
```

### No `unwrap()` in Production

**Rule**: Never call `.unwrap()` or `.expect()` outside `#[cfg(test)]` blocks.

```rust
// ✅ COMPLIANT — propagate with ?
pub fn process_order(order_id: &str) -> Result<(), OrderError> {
    let id = OrderId::new(order_id);
    let order = self.tracker.get(&id).ok_or(OrderError::NotFound { id })?;
    order.validate()?;
    Ok(())
}

// ❌ NON-COMPLIANT — panics in production
pub fn process_order(order_id: &str) -> Result<(), OrderError> {
    let order = self.tracker.get(order_id).unwrap(); // panics if missing
    order.validate().unwrap();                        // panics on error
    Ok(())
}
```

### Expression-Oriented Style

```rust
// ✅ COMPLIANT — expression-oriented
let size_limit = if is_day_trade {
    config.max_position_size_dollars
} else {
    config.max_position_size_dollars * RiskMultiplier::new(2.0).unwrap()
};

// ❌ NON-COMPLIANT — statement-oriented with mutation
let size_limit;
if is_day_trade {
    size_limit = config.max_position_size_dollars;
} else {
    size_limit = config.max_position_size_dollars * 2.0;
}
```

---

## 4. Error Propagation with `thiserror`

### Rules

1. **Every module that can fail must define its own error enum** using `#[derive(thiserror::Error)]`. No `Box<dyn std::error::Error>` in public APIs.
2. **Error variants must carry typed data**, not raw `String` messages, wherever the caller may need to branch on the data.
3. **Use `#[from]`** for automatic conversions where a lower-level error maps cleanly to a higher-level variant.
4. **`Result<_, String>` is NEVER acceptable** in production code.
5. **The `?` operator** is the primary error propagation mechanism.

### Error Hierarchy

```
AppError
├── BrokerError     (src/utils/errors.rs) — connection, auth, order rejection, rate limits
├── RiskError       (src/utils/errors.rs) — loss limits, PDT, position sizing, pattern state
├── DatabaseError   (src/utils/errors.rs) — connection, queries, transactions, SQLite
├── PatternError    (src/utils/errors.rs) — insufficient data, invalid candles, calculation
├── OrderError      (src/orders/types.rs) — not found, validation, placement, tracking
├── ExecutionError  (src/execution/mod.rs) — strategy, signal, rule evaluation
└── PositionError   (src/positions/tracker.rs) — not found, P&L, broker sync
```

### Errors to Add (Current Tech Debt)

These error types do not yet exist and must be created:

| Error type | Replaces | Location |
|------------|----------|----------|
| `ConfigError` | `config::ConfigError` from external crate | `src/utils/errors.rs` |
| `RiskEngineError` | `Box<dyn std::error::Error + Send + Sync>` in 11 methods | `src/utils/errors.rs` |
| `RepositoryError` | `Box<dyn std::error::Error + Send + Sync>` in all `PdtRepositoryTrait` methods | `src/utils/errors.rs` |

### Known Violations to Fix

| Location | Violation | Fix |
|----------|-----------|-----|
| `src/risk/limits.rs:190` | `Result<bool, String>` | Change to `Result<bool, RiskError>` |
| `src/risk/pdt.rs:315` | `Result<bool, String>` | Change to `Result<bool, RiskError>` |
| `src/db/models.rs:423` | `Result<TradingSignal, String>` | Change to `Result<TradingSignal, DatabaseError>` |
| `src/risk/engine.rs` (11 methods) | `Box<dyn std::error::Error + Send + Sync>` | Change to `Result<_, RiskEngineError>` |
| `src/db/traits.rs` (11 methods) | `Box<dyn std::error::Error + Send + Sync>` | Change to `Result<_, RepositoryError>` |
| `src/infrastructure/repositories.rs` (20+ methods) | `Box<dyn std::error::Error + Send + Sync>` | Change to `Result<_, RepositoryError>` |

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — typed error with structured data
#[derive(Debug, thiserror::Error)]
pub enum RiskEngineError {
    #[error("Risk rule evaluation failed: {0}")]
    RuleEvaluation(String),
    #[error("Signal recording failed: {0}")]
    SignalRecording(String),
    #[error(transparent)]
    Risk(#[from] RiskError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
}

pub async fn evaluate_trade(
    &self,
    context: TradeContext,
) -> Result<RiskEvaluation, RiskEngineError> {
    let rule_result = self.rule_engine.evaluate(&context)?;  // ? propagates
    self.record_signal(&context).await?;                     // ? propagates
    Ok(rule_result)
}

// ❌ NON-COMPLIANT — erased type loses context; callers cannot branch
pub async fn evaluate_trade(
    &self,
    context: TradeContext,
) -> Result<RiskEvaluation, Box<dyn std::error::Error + Send + Sync>> {
    let rule_result = self.rule_engine.evaluate(&context)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(rule_result)
}
```

```rust
// ✅ COMPLIANT — typed error propagation
pub fn check_daily_loss_limit(
    &self,
    current_loss: DollarAmount,
) -> Result<bool, RiskError> {
    if current_loss > self.limit {
        return Err(RiskError::DailyLossLimitExceeded {
            current_loss: current_loss.value(),
            limit: self.limit.value(),
        });
    }
    Ok(true)
}

// ❌ NON-COMPLIANT — src/risk/limits.rs:190
pub fn check_daily_loss_limit(
    &self,
    current_loss: f64,
) -> Result<bool, String> {                          // String error — never acceptable
    if current_loss > self.limit {
        return Err(format!("Loss limit exceeded: {} > {}", current_loss, self.limit));
    }
    Ok(true)
}
```

### Using `#[from]` for Conversion Chains

```rust
// ✅ COMPLIANT — automatic conversion with #[from]
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Record not found: {entity} id={id}")]
    NotFound { entity: &'static str, id: String },
    #[error(transparent)]
    Database(#[from] DatabaseError),   // DatabaseError auto-converts with ?
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),         // sqlx::Error auto-converts with ?
}

// In AppError:
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),  // RepositoryError -> AppError with ?
    // ...
}
```

---

## 5. Static Dispatch by Default

### Rules

1. **Use generics with trait bounds** (`fn foo<T: Trait>(x: T)`) for ALL new APIs.
2. **Only use `dyn Trait`** when ALL three conditions hold:
   - (a) the trait is object-safe, AND
   - (b) the set of implementors is open or runtime-determined, AND
   - (c) the cost of monomorphization would be genuinely prohibitive (document the justification in a comment).

### Justified `dyn` Uses in This Codebase

These uses are architecturally justified and must NOT be converted to static dispatch:

| Location | Type | Justification |
|----------|------|---------------|
| `PatternRegistry` | `Arc<dyn PatternDetector>` | Heterogeneous collection of runtime-registered plugins; the set of patterns is open and user-extensible |
| `ExecutionEngine` | `Arc<dyn ExecutionStrategy>` | Runtime-swappable strategy; the strategy can be changed via Tauri command without recompilation |
| `PositionTracker` | `Arc<dyn OrderExecutionProvider>` | Dependency injection boundary; allows `MockBroker` in tests without generic explosion |
| `RiskRuleEngine` | `Vec<Box<dyn RiskRule>>` | Composable rule objects built at startup from config; heterogeneous rule list |

Each justified use MUST have a comment:

```rust
// JUSTIFIED: Arc<dyn PatternDetector> is required because patterns are
// runtime-registered plugins. The set of implementors is open (user-extensible)
// and storing them in a DashMap requires type erasure.
patterns: DashMap<PatternName, Arc<dyn PatternDetector>>,
```

### Unjustified `dyn` Uses (Must Fix)

| Location | Current | Fix |
|----------|---------|-----|
| `src/risk/engine.rs` (11 methods) | `Box<dyn std::error::Error + Send + Sync>` as return type | Replace with `RiskEngineError` |
| `src/db/traits.rs` (11 methods) | `Box<dyn std::error::Error + Send + Sync>` as return type | Replace with `RepositoryError` |
| Function parameters `&dyn Trait` where set is closed | `&dyn MarketDataProvider` in unit test helpers | Use `&T where T: MarketDataProvider` |

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — static dispatch; zero runtime overhead, full type info
pub fn backtest<B: MarketDataProvider>(
    broker: &B,
    symbols: &[Symbol],
    config: &RiskConfig,
) -> Result<BacktestReport, AppError> { ... }

// ❌ NON-COMPLIANT — dynamic dispatch where type set is closed
pub fn backtest(
    broker: &dyn MarketDataProvider,  // Only Alpaca or Mock; no runtime benefit
    symbols: &[Symbol],
    config: &RiskConfig,
) -> Result<BacktestReport, AppError> { ... }
```

```rust
// ✅ COMPLIANT — generic struct, monomorphized at compile time
pub struct OrderEngine<B: BrokerTrait> {
    broker: Arc<B>,
}

// ❌ NON-COMPLIANT — dynamic dispatch in a struct where the type is always known
pub struct OrderEngine {
    broker: Arc<dyn BrokerTrait>,  // Unnecessarily erased
}
```

---

## 6. Trait Boundaries on All Public APIs

### Rules

1. **Every `pub fn` that accepts a concrete struct where a trait would work MUST use a generic bound.**
2. **Use `impl Trait` syntax** in function arguments for single-use, non-generic contexts: `fn foo(x: impl Display)`.
3. **Use explicit `<T: Trait>` syntax** when the type appears in multiple positions, in `where` clauses for readability, or when the type needs to be named in the return type.
4. **Traits must be defined for every major behavior boundary**: broker access, data storage, risk evaluation, pattern detection, order execution. Concrete types appear ONLY at the composition root (`main.rs` / `daytrader/src-tauri/src/main.rs`).

### Major Trait Boundaries

| Boundary | Trait | Concrete implementations |
|----------|-------|--------------------------|
| Market data | `MarketDataProvider` | `AlpacaBroker`, `MockBroker` |
| Order execution | `OrderExecutionProvider` | `AlpacaBroker`, `MockBroker` |
| PDT storage | `PdtRepositoryTrait` | `PdtRepository` (SQLite) |
| Pattern detection | `PatternDetector` | 10 candlestick implementations |
| Execution strategy | `ExecutionStrategy` | `DefaultStrategy`, `ConservativeStrategy` |
| Risk evaluation | `RiskRule` | `DailyLossRule`, `PositionSizeRule`, `PdtRule`, etc. |

### Current Partial Compliance

`RiskEngine::new()` in `src/risk/engine.rs` already uses a trait for the broker parameter:

```rust
pub fn new(
    config: RiskConfig,
    event_bus: EventBus,           // ← should become impl EventPublisher (RF-E-001)
    account_id: AccountId,
    initial_equity: Decimal,
    broker: Arc<dyn MarketDataProvider>,  // ✅ using trait
) -> Self
```

The `event_bus: EventBus` parameter is a current violation — it should become `event_bus: impl EventPublisher` once the `EventPublisher` trait is defined (tracked as RF-E-001).

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — trait bound, testable with any implementor
pub async fn execute_signal<B>(
    signal: &PatternSignal,
    broker: &B,
    config: &RiskConfig,
) -> Result<OrderId, ExecutionError>
where
    B: OrderExecutionProvider + MarketDataProvider,
{ ... }

// ❌ NON-COMPLIANT — concrete type, cannot mock in tests
pub async fn execute_signal(
    signal: &PatternSignal,
    broker: &AlpacaBroker,   // Hard dependency on specific implementation
    config: &RiskConfig,
) -> Result<OrderId, ExecutionError> { ... }
```

```rust
// ✅ COMPLIANT — impl Trait for single-use parameter
pub fn log_event(event: impl fmt::Display) {
    tracing::info!("{}", event);
}

// ✅ COMPLIANT — explicit <T: Trait> when T appears in multiple positions
pub fn convert_and_validate<T: TryFrom<RawOrder>>(
    raw: RawOrder,
) -> Result<T, T::Error> {
    T::try_from(raw)
}
```

### Defining a New Trait

When adding a behavior boundary, define the trait first, implement it for the concrete type, then write the function accepting the trait:

```rust
// 1. Define the trait in traits.rs
pub trait EventPublisher: Send + Sync {
    fn publish(&self, event: MarketEvent) -> Result<(), BrokerError>;
}

// 2. Implement for the concrete type
impl EventPublisher for EventBus {
    fn publish(&self, event: MarketEvent) -> Result<(), BrokerError> {
        self.send(event).map_err(|e| BrokerError::ApiError(e.to_string()))
    }
}

// 3. Function accepts the trait, not the concrete type
pub fn new(event_bus: impl EventPublisher) -> Self { ... }
```

---

## 7. Async and Concurrency

### Rules

1. **Never block inside an async context.** Use `.await` for async I/O or `tokio::task::spawn_blocking` for CPU-bound work.
2. **Shared state across async tasks uses `Arc<Mutex<T>>`** (for short-lived locks) or `Arc<RwLock<T>>` (for read-heavy data).
3. **Never hold a lock across an `.await` point.** Acquire, use, drop — all before the next `.await`.
4. **Use `tokio::sync::Mutex`** for async-aware locking. Use `std::sync::Mutex` only for sync contexts where the lock is never held across a yield point.
5. **`DashMap` for concurrent hash maps** — the established project-wide pattern. Do not use `Arc<RwLock<HashMap<K, V>>>` where `DashMap` suffices.
6. **EventBus subscribers MUST handle `RecvError::Lagged`** — the broadcast channel has a fixed capacity (10,000); lagged receivers will miss events. Log the lag count and re-subscribe if required.

### Lock Discipline

```rust
// ✅ COMPLIANT — lock dropped before .await
async fn update_and_notify(&self) -> Result<(), AppError> {
    {
        let mut state = self.state.lock().await;
        state.update();
        // lock dropped here at end of block
    }
    self.event_bus.publish(StateUpdated).await?;  // no lock held here
    Ok(())
}

// ❌ NON-COMPLIANT — lock held across .await; will deadlock under contention
async fn update_and_notify(&self) -> Result<(), AppError> {
    let mut state = self.state.lock().await;
    state.update();
    self.event_bus.publish(StateUpdated).await?;  // lock still held — DEADLOCK RISK
    Ok(())
}
```

### Mutex Selection

```rust
// ✅ COMPLIANT — tokio mutex for async context
use tokio::sync::Mutex;

pub struct ExecutionEngine {
    state: Arc<Mutex<ExecutionState>>,  // held across async boundaries
}

// ✅ COMPLIANT — std mutex for sync-only context
use std::sync::Mutex;

pub struct MetricsRegistry {
    counters: Mutex<HashMap<String, u64>>,  // only accessed synchronously
}

// ❌ NON-COMPLIANT — std mutex locked inside async fn
async fn process(&self) {
    let _guard = self.std_mutex.lock().unwrap();  // blocks the Tokio thread
    do_work().await;  // lock held across .await — starvation and deadlock risk
}
```

### EventBus Lag Handling

```rust
// ✅ COMPLIANT — handles Lagged error
loop {
    match receiver.recv().await {
        Ok(event) => handle_event(event),
        Err(RecvError::Lagged(n)) => {
            tracing::warn!("Event subscriber lagged by {} messages; some events were missed", n);
            // Continue receiving; next recv() will return the oldest available message
        }
        Err(RecvError::Closed) => break,
    }
}

// ❌ NON-COMPLIANT — panics on lag
let event = receiver.recv().await.unwrap();  // panics if lagged
```

### Spawning Tasks

```rust
// ✅ COMPLIANT — CPU-bound work off the async runtime
let result = tokio::task::spawn_blocking(|| {
    expensive_computation()
}).await?;

// ❌ NON-COMPLIANT — CPU-bound work blocks the Tokio thread pool
async fn process_candles(&self) {
    let result = expensive_computation();  // blocks the runtime thread
}
```

---

## 8. Documentation Standards

### Rules

1. **Every `pub` item must have a doc comment** (`///`). Private items should be documented when non-obvious.
2. **Public functions must document**:
   - Purpose (first line, imperative mood: "Calculates...", "Returns...", "Creates...")
   - `# Arguments` — each parameter with its meaning
   - `# Returns` — what is returned on success
   - `# Errors` — every error variant that can be returned
   - `# Panics` — if the function can panic (should be rare; see §3)
   - `# Examples` — at least one runnable `doctest`
3. **Modules must have a module-level doc comment** (`//!`) explaining the module's responsibility and listing key types.
4. **Crate-level `//!` docs** in `lib.rs` must list all public modules and their roles.

### Module Doc Comment Template

```rust
//! <Component name> — <one-sentence responsibility>
//!
//! This module provides <what it provides>. Key types:
//!
//! - [`KeyType`] — <what it does>
//! - [`AnotherType`] — <what it does>
//!
//! # Architecture
//!
//! <How this module fits into the system>
//!
//! # Examples
//!
//! ```
//! use daytrader::<module>::<Type>;
//!
//! let x = <Type>::new(...)?;
//! ```
```

### Function Doc Comment Template

```rust
/// <Imperative verb phrase describing what the function does>
///
/// <Optional longer description>
///
/// # Arguments
///
/// * `param1` — <what it represents>
/// * `param2` — <what it represents>
///
/// # Returns
///
/// <What is returned on Ok, or what the return value represents>
///
/// # Errors
///
/// Returns [`ErrorType::Variant`] if <condition>.
/// Returns [`ErrorType::OtherVariant`] if <other condition>.
///
/// # Panics
///
/// This function does not panic. (or: Panics if <condition>.)
///
/// # Examples
///
/// ```
/// use daytrader::module::Type;
///
/// let result = Type::function(args)?;
/// assert_eq!(result, expected);
/// ```
pub fn function(param1: Type1, param2: Type2) -> Result<ReturnType, ErrorType> {
```

### Compliant vs Non-Compliant

```rust
// ✅ COMPLIANT — full doc comment with all sections
/// Evaluates whether a trade is permissible under current risk rules.
///
/// Runs all registered risk rules against the provided trade context and
/// returns an evaluation result with the aggregate decision and any violations.
///
/// # Arguments
///
/// * `context` — The trade context containing symbol, quantity, price, and account state
///
/// # Returns
///
/// A [`RiskEvaluation`] containing the approval decision and list of rule results.
///
/// # Errors
///
/// Returns [`RiskEngineError::RuleEvaluation`] if a rule fails to execute.
/// Returns [`RiskEngineError::Database`] if PDT history lookup fails.
///
/// # Examples
///
/// ```
/// # use daytrader::risk::engine::RiskEngine;
/// # use daytrader::risk::types::TradeContext;
/// # tokio_test::block_on(async {
/// let context = TradeContext::new(symbol, quantity, price, equity, false);
/// let evaluation = engine.evaluate_trade(context).await?;
/// if evaluation.is_approved() {
///     // proceed with order placement
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # });
/// ```
pub async fn evaluate_trade(
    &self,
    context: TradeContext,
) -> Result<RiskEvaluation, RiskEngineError> { ... }

// ❌ NON-COMPLIANT — no documentation; intent, errors, and usage are unknown
pub async fn evaluate_trade(
    &self,
    context: TradeContext,
) -> Result<RiskEvaluation, RiskEngineError> { ... }
```

```rust
// ✅ COMPLIANT — module-level doc comment
//! Risk management engine — evaluates trade proposals against configurable rules.
//!
//! This module provides:
//!
//! - [`RiskEngine`] — the main orchestrator; evaluates all rules for a trade
//! - [`RiskRuleEngine`] — composable rule evaluation with `Vec<Box<dyn RiskRule>>`
//! - [`TradeContext`] — typed input to risk evaluation
//! - [`RiskEvaluation`] — typed output with approval decision and violations

// ❌ NON-COMPLIANT — no module doc
// (empty, or just `use` statements at the top of mod.rs)
```

---

## Appendix: Quick Reference Checklist

Use this checklist before every commit. Also enforced by `./scripts/verify_agent_work.sh`.

### Pre-Commit Checklist

- [ ] No `f64`/`u32`/`i64`/`String`/`bool` in public struct fields or `pub fn` parameters for domain values — use newtypes
- [ ] No `.unwrap()` or `.expect()` outside `#[cfg(test)]`
- [ ] No `Box<dyn std::error::Error>` in public API signatures — use typed error enums
- [ ] No `Result<_, String>` anywhere in production code
- [ ] All `for` loops that push to `Vec` replaced with `.map().collect()`
- [ ] All `for` loops that filter replaced with `.filter().collect()`
- [ ] All `for` loops that accumulate a scalar replaced with `.fold()` or `.sum()`
- [ ] All `match` arms are exhaustive (no `_` unless justified and documented)
- [ ] All new `pub fn` use trait bounds or `impl Trait`, not concrete types, where a trait exists
- [ ] All new `dyn Trait` uses have a justification comment explaining why static dispatch is not viable
- [ ] New `pub` items have `///` doc comments with `# Errors` section if fallible
- [ ] New modules have `//!` module-level doc comments
- [ ] `tokio::sync::Mutex` used for async contexts; `std::sync::Mutex` for sync-only contexts
- [ ] No lock held across an `.await` point
- [ ] `./scripts/verify_agent_work.sh` passes
