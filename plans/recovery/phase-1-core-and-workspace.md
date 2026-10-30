# Phase 1: Canonical Domain Model and Workspace Split

Goal: one definition per domain concept, an injected clock everywhere, and module boundaries the compiler enforces. This phase is mostly mechanical once B-101 exists; the order below keeps every PR small and the suite green throughout.

Pre-conditions: Phase 0 exited; #29 merged.

---

## B-101 Design and land `dt-core` — **[design]** (strong model or owner; one PR; merged unused)

**Goal:** A new crate `crates/dt-core` containing the single definition of every domain type, with unit tests and doc comments, that nothing depends on yet.
**Files:** `Cargo.toml` (becomes a workspace root listing `.`, `crates/dt-core`, `daytrader/src-tauri`), `crates/dt-core/**` (new).
**Contents (names are decided; field details are the design work):**

```rust
// money & quantities
pub struct Money(i64);              // signed fixed-point micros (6 dp), decided 2026-10-30 (B-204); exact, and sortable/summable in SQLite;
                                    // checked Add/Sub, Mul by Quantity via Decimal at the boundary; Display with 2dp; serde as string
pub struct Price(Decimal);          // > 0
pub struct Quantity(Decimal);       // >= 0, fractional allowed; Display without trailing zeros
pub struct Percentage(Decimal);     // 0..=100
pub struct Bps(u32);

// identity
pub struct Symbol(String);          // uppercase, 1..=10, validated
pub struct OrderId(Uuid);           // internal
pub struct ClientOrderId(String);   // sent to broker; derived deterministically from OrderId
pub struct BrokerOrderId(String);
pub struct AccountId(String);
pub struct PatternName(&'static str or String);

// market data
pub struct Bar { symbol, start: DateTime<Utc>, open, high, low, close: Price, volume: Quantity, vwap: Option<Price>, trade_count: Option<u32>, source: DataSource }
pub struct Quote { symbol, ts, bid: Price, bid_size: Quantity, ask: Price, ask_size: Quantity }
pub enum Timeframe { Min1, Min5, Min15, Hour1, Day1 }

// orders
pub enum Side { Buy, Sell }
pub enum OrderType { Market, Limit { limit: Price }, Stop { stop: Price }, StopLimit { stop: Price, limit: Price }, TrailingStop { trail: TrailAmount } }
pub enum TimeInForce { Day, Gtc, Ioc, Fok, Opg, Cls }
pub enum OrderStatus { New, PendingSubmit, Submitted, Accepted, PartiallyFilled, Filled, PendingCancel, Cancelled, Rejected, Expired, Replaced, DoneForDay, Held, Suspended }
impl OrderStatus { pub fn can_transition_to(self, next: OrderStatus) -> bool; pub fn is_terminal(self) -> bool; pub fn is_open(self) -> bool }
pub struct OrderIntent { symbol, side, qty: Quantity, order_type, tif, pattern: Option<PatternName>, signal_id: Option<SignalId>, created_at }
pub struct Order { id: OrderId, client_order_id, broker_order_id: Option<BrokerOrderId>, intent: OrderIntent, status, filled_qty: Quantity, avg_fill_price: Option<Price>, submitted_at, updated_at }
pub struct Fill { id: FillId, order_id, broker_order_id, symbol, side, qty: Quantity /* this execution only */, price: Price, commission: Money, ts: DateTime<Utc>, cumulative_qty: Quantity }

// positions
pub enum PositionSide { Long, Short }
pub struct Lot { qty: Quantity, price: Price, opened_at, order_id }
pub struct Position { symbol, side, qty: Quantity, lots: Vec<Lot> /* FIFO */, avg_cost: Price, realized_pnl: Money, last_price: Option<Price>, opened_at, pattern: Option<PatternName> }
impl Position { pub fn unrealized_pnl(&self) -> Money; pub fn market_value(&self) -> Money; pub fn apply_fill(&mut self, fill: &Fill) -> Result<Money /*realized*/, PositionError> }

// signals
pub struct SignalId(Uuid);
pub struct Signal { id, symbol, side, confidence: Confidence /*0..=1 Decimal*/, pattern: PatternName, price_at_signal: Price, bar_ts: DateTime<Utc>, context: SignalContext }

// time
pub trait Clock: Send + Sync { fn now(&self) -> DateTime<Utc>; }
pub struct SystemClock; pub struct FixedClock(..); pub struct SteppedClock(..);
pub struct TradingCalendar; // America/New_York; open/close incl. early closes; holidays; is_open(ts); session_date(ts) -> NaiveDate; next_open(ts)

// errors
pub enum CoreError { Validation(String), InvalidTransition { from, to }, MixedSymbols { expected, found }, .. }  // thiserror
```

Rules for the design: no `f64` anywhere; no `async`; no tokio, sqlx, reqwest; `serde` derives with money/quantity serialised as strings; `#![forbid(unsafe_code)]`, `#![deny(clippy::unwrap_used, clippy::expect_used)]` for non-test; property tests (`proptest`) for `Position::apply_fill` (FIFO realized P&L invariants), `OrderStatus` transitions, `TradingCalendar` DST and early-close days; 100% of public items documented.
**Proof:** `cargo test -p dt-core`; `cargo clippy -p dt-core --all-targets -- -D warnings`; `cargo doc -p dt-core` with `RUSTDOCFLAGS=-Dwarnings`.
**Size:** L, but it is new code with no migration, so it is one PR.

## B-102 … B-108 Migrate one module at a time onto `dt-core` — agent, strictly in this order, one PR each

Each PR: replace the module's local type(s) with the `dt-core` type, delete the local definition and every converter to/from it, update tests, keep the suite green. If a `dt-core` type lacks something a module genuinely needs, the PR adds it to `dt-core` with a test (small additions only; a redesign goes back to the owner).

| ID | Module | Types removed | Notes |
|---|---|---|---|
| B-102 | `brokers` | `traits::{Candle,Quote,Order,Position,OrderSide,OrderType,OrderStatus,TimeInForce}`, `types::{Price,Quantity,Symbol,OrderId,PatternName,DollarAmount,Percentage}` | `Quantity` becomes Decimal: Alpaca fractional quantities now parse. `AlpacaOrder → Order` mapping gains all Alpaca statuses. |
| B-103 | `events` | `types::{Candle,Quote,Order,Fill,Position,OrderSide,OrderType,OrderStatus}` | `MarketEvent` variants carry `dt_core` types. Converters in `orders/tracker.rs:86-182` deleted. |
| B-104 | `patterns` | `PatternResult.price_level` etc. use core; `Signal` replaces `TradingSignal` + `PatternSignal` | `Signal.symbol` comes from the bar; the B-085 `MixedSymbols` error moves to `CoreError`. |
| B-105 | `positions` | `tracker::Position`, `pnl.rs` lot logic → `dt_core::Position::apply_fill` | `positions::PositionTracker` becomes the single `Portfolio` (map Symbol→Position + persistence hook). |
| B-106 | `orders` | `ManagedOrder`, `OrderRequest`, `FillInfo`, `OrderConfig` price fields | `Order` + `OrderIntent`; `OrderTracker` validates transitions with `can_transition_to`; `ClientOrderId` sent to broker. |
| B-107 | `risk` | `types::TrackedPosition`, `position_sizing::PositionInfo`, `pattern_stats::{PatternStats,PatternPerformanceStats}` (keep one, in `dt-core` or `dt-risk`), `DailyRiskMetrics` dup | `risk::position_tracker` deleted; risk reads the `Portfolio`. `RiskConfig` money fields become `Money`/`Percentage` (config parsing converts once, at the boundary). |
| B-108 | `execution`, `db`, Tauri | `execution::PatternPerformance`, `rules::PatternStats`, `PerformanceThresholds` dup; `db::models` f64 fields → core types; Tauri DTOs are thin `From<dt_core::_>` | DB columns for money become `INTEGER` micros (B-204, decided 2026-10-30); the dev DB is disposable, so no row-conversion script is needed. |

**Proof for each:** `cargo test --workspace` pass count ≥ before; `grep -rn "<deleted type name>" src/ daytrader/` prints nothing; `cargo clippy --workspace --all-targets -- -D warnings` clean.
**Size:** M each; `risk` (B-107) will need splitting by file.

## B-109 Thread `Clock` everywhere — agent (can start after B-102; finish after B-108)

**Goal:** zero `Utc::now()` outside `dt_core::SystemClock` and `#[cfg(test)]`.
**Files:** every file the grep below lists.
**Steps:** every struct that calls `Utc::now()` gains an `Arc<dyn Clock>` field injected at construction (default `SystemClock` only in binaries' `main`, never in library constructors); the mock broker stamps fills from its clock; `TradingCalendar::session_date(clock.now())` replaces every `Utc::now().date_naive()`.
**Proof:** `grep -rn "Utc::now()" crates/ src/ daytrader/src-tauri/src --include=*.rs | grep -v "clock.rs" | grep -v "#\[cfg(test)\]"` is empty, verified by a CI step added in this PR; `tests/` include one test that runs the order engine with a `FixedClock` at a holiday and asserts rejection.
**Size:** M–L; split by crate if needed.

## B-110 … B-117 Split into workspace crates — agent, one crate per PR, in this order

Order is bottom-up so each crate compiles when extracted: `dt-core` (exists) → `dt-broker` (traits + `SimBroker` from `MockBroker`) → `dt-alpaca` → `dt-strategy` (`patterns/`) → `dt-risk` → `dt-store` (`db/`) → `dt-engine` (`orders/`, `positions/`, `execution/`, `events/`) → `dt-backtest` (`execution/backtest.rs`) → `bins/dtctl` (new; replaces `src/main.rs`) → Tauri depends on `dt-engine`.

Each PR: `git mv` the module into `crates/<name>/src`, add `Cargo.toml` with only the dependencies the crate uses (no `tokio` in `dt-core`, `dt-strategy`), fix paths, keep the old path as a `pub use` re-export in the root crate until the last PR deletes the root crate's `src/`. The per-module `CLAUDE.md`, `verify_architecture.sh`, `architecture_exceptions.txt` and `architecture_violations_baseline.txt` are deleted in the last PR (B-117), because Cargo now enforces the graph. Each crate gets `#![forbid(unsafe_code)]`, `#![warn(missing_docs, clippy::pedantic)]`, `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`.
**Proof:** `cargo test --workspace`; `cargo tree -p dt-core -e normal` shows no tokio/sqlx/reqwest; `cargo tree -i dt-engine` shows only `dt-backtest`, `dtctl`, `daytrader-app`.
**Size:** M each.

## Phase 1 exit checklist

- [ ] One definition of Bar, Quote, Order, OrderStatus, Fill, Position, Signal, PatternStats; `grep -rn "struct Position\b\|struct Order\b\|struct Candle\b\|struct Fill\b" crates/ src/` returns only `dt-core`
- [ ] `Utc::now()` grep (B-109) empty and enforced in CI
- [ ] Money stored as `INTEGER` micros (B-204); `Quantity` is Decimal end to end
- [ ] Money tests from roadmap R0.2: `Money` ↔ `Decimal` round-trip property test; a 1,000-trade P&L fixture equals the hand-computed exact value; money columns follow the `*_micros` naming convention and ratio columns are explicitly allow-listed as `REAL`; `grep -rn 'from_f64_retain\|to_f64()' crates/ src/` outside tests returns nothing
- [ ] Workspace of 8 crates + `dtctl` + Tauri; root `src/` deleted
- [ ] `verify_architecture.sh` and per-module `CLAUDE.md` deleted
