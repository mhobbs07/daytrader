---
name: add-trait-boundary
version: 1.0
description: Replace a concrete type parameter with a generic trait bound
applies_to: ["*.rs"]
trigger: "public fn takes concrete struct that could be a trait"
---

# Skill: Add Trait Boundary

Use this skill when a public function or `impl` block takes a concrete type where an abstract trait would be more appropriate. This enables: testing with mock implementations, multiple concrete implementations, and decoupling of modules.

## Step 1 — Identify the function and concrete type

Look for:
- Public functions taking a concrete struct (e.g. `fn new(broker: AlpacaBroker)`)
- Struct fields holding a concrete type (e.g. `broker: AlpacaBroker`)
- Functions in a higher-level module calling methods on a lower-level concrete type

```bash
grep -n "AlpacaBroker\|MockBroker\|SqliteRepository" src/<module>/*.rs
```

## Step 2 — Check if an appropriate trait already exists

```bash
grep -rn "pub trait" src/
```

Common existing traits:
- `MarketDataProvider` — `src/brokers/traits.rs` — provides market data (quotes, bars)
- `OrderExecutionProvider` — `src/brokers/traits.rs` — executes orders
- `PdtRepositoryTrait` — `src/db/traits.rs` — PDT tracking data access
- `PatternDetector` — `src/patterns/` — detects a candlestick/indicator pattern
- `ExecutionStrategy` — `src/execution/` — runtime-swappable execution algorithm
- `EventPublisher` — `src/events/` — publishes events to the bus

If an appropriate trait exists, proceed to Step 4. If not, define one.

## Step 3 — Define the trait

Create or add to `traits.rs` in the same module as the concrete implementation:

```rust
/// Provides read access to market price data.
///
/// Implement this trait for any data source: live broker, replay feed,
/// or mock for testing.
pub trait MarketDataProvider: Send + Sync {
    /// Returns the current bid/ask quote for `symbol`.
    ///
    /// # Errors
    /// Returns `BrokerError::SymbolNotFound` if the symbol is unknown.
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;

    /// Returns OHLCV bars for `symbol` over `timeframe`.
    ///
    /// # Errors
    /// Returns `BrokerError::ApiError` on network or parse failure.
    async fn get_bars(
        &self,
        symbol: &Symbol,
        timeframe: TimeFrame,
        limit: Count,
    ) -> Result<Vec<Bar>, BrokerError>;
}
```

Guidelines:
- Add `Send + Sync` bounds for traits used across async boundaries
- Document each method with `///` and a `# Errors` section
- Use the types from the module's type system, not raw primitives
- Keep the trait focused: prefer narrow traits over fat traits (Interface Segregation)

## Step 4 — Change the function/struct signature

### For function parameters:

```rust
// BEFORE
pub fn new(broker: AlpacaBroker, db: SqliteRepository) -> Self {

// AFTER — option A: generic (zero runtime cost, monomorphized at compile time)
pub fn new<B: MarketDataProvider, R: PdtRepositoryTrait>(broker: B, db: R) -> Self {

// AFTER — option B: impl Trait syntax (equivalent to A, often cleaner)
pub fn new(broker: impl MarketDataProvider, db: impl PdtRepositoryTrait) -> Self {
```

Choose **generic** or **`impl Trait`** for free functions and constructor `new()` calls — these use static dispatch and incur zero runtime overhead.

### For struct fields that hold the dependency:

```rust
// BEFORE
pub struct RiskEngine {
    broker: AlpacaBroker,
}

// AFTER — option A: generic struct (static dispatch, zero cost)
pub struct RiskEngine<B: MarketDataProvider> {
    broker: B,
}

// AFTER — option B: trait object (dynamic dispatch, use only when type erasure is required)
pub struct RiskEngine {
    broker: Arc<dyn MarketDataProvider>,
}
```

Use **generic struct** (option A) when the concrete type is always known at the call site.

Use **`Arc<dyn Trait>`** (option B) only when:
- The type must be stored alongside other trait objects in a heterogeneous collection, OR
- The type must be set at runtime (e.g. `ExecutionStrategy` swapped based on config), OR
- The struct is returned across an `async` boundary and `PhantomData<T>` would complicate lifetimes

Document the justification in a comment when using `Arc<dyn Trait>`:

```rust
/// Strategy stored as trait object to allow runtime swapping via configuration.
/// Static dispatch is not applicable here: the concrete type is not known at compile time.
strategy: Arc<dyn ExecutionStrategy>,
```

## Step 5 — Update callers

Static dispatch (generic or `impl Trait`) is transparent to callers — they pass the same concrete type they did before. The compiler monomorphizes automatically.

```rust
// Caller code — unchanged
let engine = RiskEngine::new(alpaca_broker, sqlite_repo);
```

If you changed to a trait object (`Arc<dyn Trait>`), callers must wrap:

```rust
// BEFORE
let engine = RiskEngine::new(alpaca_broker);

// AFTER
let engine = RiskEngine::new(Arc::new(alpaca_broker) as Arc<dyn MarketDataProvider>);
// or just:
let engine = RiskEngine::new(Arc::new(alpaca_broker));  // compiler infers the cast
```

## Step 6 — Handle PhantomData if needed

If a generic struct does not store the type `T` directly (e.g. it's only used at construction time), add `PhantomData`:

```rust
pub struct Processor<T: DataSource> {
    config: ProcessorConfig,
    _source: PhantomData<T>,
}
```

This is rarely needed — if the struct stores the dependency (which it usually does), `PhantomData` is not required.

## Step 7 — Run verification

```bash
cargo check
cargo test --lib -- <module_name>
./scripts/verify_agent_work.sh
```

**Acceptance check**: The architecture verification script checks that module boundaries are respected. Verify that the trait is defined in the correct layer — traits for broker concepts belong in `src/brokers/traits.rs`, not in `src/risk/`.
