# DayTrader: Honest Whole-App Assessment and Recovery Strategy (2026-10-30)

**Branch audited:** `staging` at `e61e85a` (the integration branch, 124 files and ~9k lines ahead of `main`), plus the open PR [mhobbs07/daytrader#29](https://github.com/mhobbs07/daytrader/pull/29) `feat/close-trading-loop` at `6cd5450`, referred to below as **CTL**. `main` at `5b09248` is six months stale (last merge April 22) and does not compile from a clean clone; it is not a useful reference point and this report does not audit it further.

**What was verified in this session (not just read):**

| Check | Result |
|---|---|
| `cargo check` on `staging`, no `DATABASE_URL` | Fails: 45 sqlx macro errors |
| `cargo check --all-targets` on `staging`, hand-provisioned `DATABASE_URL` | Library and all test targets compile; 691 library warnings; 3 bench targets fail to compile |
| `cargo test --lib` on `staging` | 323 passed, 0 failed |
| `cargo test --tests` on `staging` | All 17 test binaries pass; 27 tests `#[ignore]`d |
| `cargo clippy -- -D warnings` on `staging` (what CI runs) | 1,792 errors |
| `scripts/verify_agent_work.sh` on `staging` | Fails compilation in any environment without the `sqlite3` CLI (it silently skips DB provisioning) |
| Three code audits (trading loop, brokers/config/Tauri, DB/persistence) against `staging` and CTL | Every high-severity claim below was spot-checked by hand |

File references are `path:line` on `staging` unless marked CTL.

---

## 1. Summary verdict

The repository contains a large number of individually unit-tested components and a growing amount of real engineering (DST-correct market hours, a `Clock` trait, a token-bucket rate limiter, real day-trade detection, a first backtest replay engine on CTL). It does not yet contain a trading system. On `staging`:

- No binary runs the trading loop. `src/main.rs` is a stub; the Tauri app runs risk + mock data only. CTL is the first branch to construct `OrderEngine`, `ExecutionEngine` and `PositionTracker` together, and only against the mock broker.
- The risk engine deadlocks on the first detected pattern, and its pre-trade gate is advisory.
- Every generated signal carries the symbol `"DEFAULT"`; detectors read the oldest cached candle as "current" (fixed in CTL only).
- Nothing persists fills, nothing rehydrates on restart, PDT and daily-loss state live in memory.
- There is no backtest capability on `staging`; CTL adds a v1 that is a good skeleton but is not yet deterministic, has no data loader, and runs with the risk engine not started.
- CI cannot go green: it provisions no database for the sqlx macros, and clippy under `-D warnings` has 1,792 errors.
- The same concepts are defined 4 to 8 times each (Position, Order, Candle, Quote, Fill, PatternStats), with hand-written lossy converters between them. This is the biggest single drag on every other fix.

The build and test infrastructure is in better shape than `main` suggested, and the unit tests are healthy. The gap is assembly, correctness of the risk core, and duplication, not test discipline.

---

## 2. Build, CI and repo hygiene

| # | Finding | Status on staging | Evidence |
|---|---|---|---|
| 2.1 | `.gitignore` `data/` swallowed `src/data/` | **Fixed** (anchored to `/data/`) | `.gitignore:25` |
| 2.2 | sqlx macros need `DATABASE_URL`; no `.sqlx/` offline cache; `.sqlx/` gitignored; no `SQLX_OFFLINE` | **Still present** | `.gitignore:12`, `.env.example:6`; BACKLOG B-075 admits it |
| 2.3 | CI never provisions a DB, so `clippy`, `test` and `doc` jobs cannot compile the macros | **Still present** | `.github/workflows/ci.yml` has no `DATABASE_URL`; `docs.yml` same |
| 2.4 | CI triggers only on `main`/`develop`; staging PRs get no CI at all; `ci.yml:72` runs `cargo test --test integration` which does not exist; Tauri crate is never built in CI | **Still present** | `ci.yml:3-7,72` |
| 2.5 | `cargo clippy -- -D warnings` (the CI step) fails | **Still present**: 1,792 errors | measured this session; B-074 says "~130" |
| 2.6 | `Cargo.lock` gitignored for an application | **Still present** | `.gitignore:3` |
| 2.7 | Dockerfile copies untracked `Cargo.lock`, omits the `src-tauri` workspace member, provisions no DB | **Still present** | `Dockerfile:14` |
| 2.8 | verify script depends on the `sqlite3` CLI and silently skips provisioning without it; reuses a stale DB if one exists; external `DATABASE_URL` bypasses it | **New** | `scripts/verify_agent_work.sh:33-43` |
| 2.9 | Benches do not compile (`position_benchmarks`, `phase_3d_benchmarks`, `pattern_signal_benchmarks`) | **Still present** | newtype API drift |
| 2.10 | Orphaned `domain/`, `infrastructure/` | **Fixed** (deleted, B-064) | |
| 2.11 | Orphaned `risk/position_tracker.rs`, `utils/holidays.rs` | **Fixed** (now compiled and used) | `risk/mod.rs:7`, `utils/mod.rs:6` |
| 2.12 | 28 tests permanently excused via `known_test_issues.txt` or `#[ignore]`, including the only candle-to-signal end-to-end test | **Still present** | `.governance/known_test_issues.txt` |
| 2.13 | ~50 stale remote branches (`convoy/*`, `gt/*`, worktree branches); 24 closed-not-merged PRs whose work was merged by direct pushes; `main` six months behind `staging` | **Process debt** | `git branch -r` |
| 2.14 | `.gitlab-ci.yml` pins Rust 1.75; README says Rust 1.75 / edition 2021; `Cargo.toml` is edition 2024, `rust-version = 1.92` | Doc drift | |

---

## 3. The trading loop

### 3.1 What is wired

| Stage | staging | CTL |
|---|---|---|
| Market data → `NewCandle` | Only the Tauri `MockDataDriver` and the unused `data::ingestion` pipeline publish it. The Alpaca WebSocket stores bars in a HashMap with **no public accessor** and never publishes. | Unchanged |
| Candle → pattern → signal | `PatternStreamingPipeline` runs in Tauri, **feeding candles oldest-first to detectors that treat index 0 as newest** (`candlestick.rs:60`, `engulfing.rs:61`, `streaming.rs:377`) | Fixed (window reversed) |
| Signal symbol | `signals.rs:919` hardcodes `"DEFAULT"`; the signal cache therefore dedupes across symbols | Backtester patches the symbol after the fact; live path unchanged |
| Signal → `ExecutionEngine` | Never constructed | Constructed in Tauri, automation off by default |
| Pre-trade risk gate | `orders/engine.rs:212` checks only the halt flag. Full rule evaluation runs asynchronously on `OrderPlaced`, which is published **before** `submit_order`, and a Critical result is only logged (`risk/engine.rs:199-203`). Violates `.claude/rules/security.md` rule 4. | Unchanged |
| Fill → order tracker → position | `update_from_fill` and `sync_order_status` have no callers | 1 s `sync_order_statuses` poller + fill listener, mock only; no Alpaca `trade_updates` stream |
| Exits | `RuleEngine::evaluate_exit` has no callers; `PositionTracker::close_position` returns a fake id and places nothing; `NoDuplicatePositionRule` rejects the Sell that would close a held symbol | Unchanged. **The loop can open positions but never close them.** |
| `src/main.rs` | Stub | Stub |

### 3.2 Correctness bugs in the risk core (all still present on staging and CTL)

- **Self-deadlock on every `PatternDetected`.** `risk/engine.rs:556-557` takes write locks on `pattern_risk_evaluator` and `pattern_stats_tracker`; `evaluate_pattern_trade` then takes `stats_tracker.read()` on the same `Arc` (`pattern_limits.rs:458-459`), and even the early-return path calls `update_performance_based_multipliers`, which reads it again (`pattern_limits.rs:620-622`). No test publishes `PatternDetected` into a started `RiskEngine`, so it has never been observed.
- **Consumers die on `RecvError::Lagged`.** `risk/engine.rs:199` and `execution/engine.rs:272` use `while let Ok(..)`. The risk engine then stops evaluating while `is_trading_allowed` keeps returning true. (`events/filters.rs:97-109` and CTL's fill listener handle it correctly.)
- **Market orders skip notional checks.** Buying power is checked against `Decimal::ZERO` when `estimated_value()` is `None` (`orders/engine.rs:359-366`); `TradeContext` price is `unwrap_or(ZERO)` (`risk/engine.rs:343`). CTL avoids it only by forcing limit orders.
- **PDT.** Day-trade detection is now real (`predicts_day_trade`, FIFO tracker) and holidays are honoured. Still wrong: `count > max` allows the 4th trade (`pdt.rs:411`); `>= cutoff` gives a 6-business-day window (`pdt.rs:135`); the trading date is the UTC calendar day (`pdt.rs:350,397`, `risk/types.rs:650`); `PdtEnforcer` is always built with `repository: None` (`risk/engine.rs:110`) and there is no SQLite `PdtRepositoryTrait` implementation (B-066), so the count resets on restart. The desktop app never builds a `PdtEnforcer` at all and reports `eligible: true` with $50k equity (`commands/risk.rs:276-292,436-446`).
- **Daily loss limit never halts.** `DailyLossTracker` results are discarded (`risk/engine.rs:463,507`); commission is added to P&L as profit (`:462`); unrealized P&L is accumulated as an absolute value per `PositionUpdated`; two unlinked halt flags; `check_emergency_liquidation(daily_pl)` compares daily P&L to the $500 *balance* threshold (`:513`), so a small profit halts trading; no liquidation is implemented (B-061).
- **Daily state never resets.** `reset_daily_limits`, `should_reset_daily`, `reset_daily_counts` have no production callers; `RiskState.trading_date` never rolls.
- **Inverted multiplier.** Worse-performing patterns get a *larger* risk multiplier (`pattern_stats.rs:327-342`), applied to signal confidence (`signals.rs:1089-1095`).
- **Rule engine evaluates empty state.** `RiskRuleEngine::new` builds its own `DailyLossTracker`, `PositionSizer`, `PdtEnforcer` with equity 0 (`risk/rules.rs:479,493`), separate from the ones `RiskEngine` feeds.
- **Un-awaited future.** `evaluator.update_pattern_status(...)` at `risk/engine.rs:599` is async and never awaited, so auto-disable never happens.
- **Guards held across `.await`.** `DashMap` `RefMut` in `positions/tracker.rs:266-294, 345-409, 431-440` and `execution/engine.rs:~538-548`.
- **Eager index panic.** `.unwrap_or(&self.config.confidence_multipliers[0])` (`pattern_limits.rs:448`) panics on an empty vector; the Tauri fallback config and both CTL test configs use `vec![]`.
- **Feedback loop.** `ExecutionEngine` re-publishes `PatternDetected` (`execution/engine.rs:650,673`), which the risk engine counts again. `SignalStreamingPipeline` (unused by the apps) never clears its buffers and its stop branch exits on the first poll.
- **Ghost orders.** `place_order` persists and publishes before `submit_order`; a submit error leaves the order Pending in tracker and DB with no `OrderRejected` and leaks `pending_orders` in the risk engine. A partial fill followed by cancel loses the filled shares (no event on `PartiallyFilled`, no `fill_info` on `Cancelled`).
- **Time zones.** Market hours are now DST-correct via `chrono-tz` (`utils/market_hours.rs`) and used by `OrderEngine`, `MarketHoursRule::standard_hours`, `DailyLossTracker`. Still UTC: `MarketHoursRule::new(h1,h2)`, all PDT dates.
- **Clock.** `utils/clock.rs` exists (`SystemClock`, `FixedClock`, CTL adds `SteppedClock`) but is consumed by exactly one component (`OrderEngine`). 82 non-test `Utc::now()` calls remain in `risk` (61), `orders` (9), `positions` (8), `execution` (4); the mock broker stamps fills with wall-clock time.

### 3.3 What is better than main

Real day-trade detection, holiday calendar, DST market hours, rate limiter, `Retry-After` parsing, WebSocket exponential backoff on initial connect, the zero-price `unwrap` removed, `position_tracker`/`holidays` compiled, dead hexagonal layer deleted, `default.toml` loadable (CTL), historical bars fetch (CTL), Tauri runtime panic fixed (CTL).

---

## 4. Duplication (not consolidated on staging; CTL adds more)

| Concept | Definitions |
|---|---|
| Position | `brokers::traits::Position`, `events::types::Position`, `positions::tracker::Position`, `risk::types::TrackedPosition`, `risk::position_sizing::PositionInfo`, `db::PositionRow`, `PositionHistoryRow`, `AlpacaPosition` |
| Position **stores** | `positions::PositionTracker`, `risk::position_tracker` (FIFO), `PositionSizer.portfolio`, `OrderTracker` fill fields; CTL's backtester adds a fifth accounting path |
| Order | `brokers::traits::Order`, `events::types::Order`, `OrderRequest`, `ManagedOrder`, `OrderRow`; two `OrderType`/`OrderStatus` enums with lossy mapping (Stop/StopLimit/Trailing → Market, Expired → Cancelled, `orders/tracker.rs`) |
| Candle, Quote, Fill | each twice (`brokers/traits.rs` vs `events/types.rs`; `FillInfo` vs `Fill`). B-067 tracks Quote only. |
| PatternStats | `patterns/types.rs:371`, `risk/pattern_stats.rs:48,63`, `db/models.rs:30`, `execution/rules.rs:83`, Tauri `commands/patterns.rs:33`, `PatternRiskState`; three win-rate scales |
| Candle caches | `patterns::streaming::CandleCache`, `data::cache::MarketDataCache`, Tauri `state.rs::MarketDataCache` |
| Schema | `schema.sql` (what runs), `migrations/001+002` (unused), DDL inside `migrations.rs` macros (never called), `schema_migrations` DDL hand-copied into two shell scripts with differing column types |
| Repositories | concrete `db/repositories.rs` structs; `PdtRepositoryTrait` with only a mock impl; `OrderRepository` has zero callers while `OrderEngine` runs its own raw SQL (`orders/engine.rs:408-500`) |
| P&L | `positions/pnl.rs`, `positions/tracker.rs` sell path, risk FIFO lots, broker/event `unrealized_pl`, backtester lots |
| Config thresholds | `execution::PerformanceThresholds`, `risk::types::PatternPerformanceThresholds`, `RiskConfig`, Tauri hand-copied fallback |
| `src/data/` | restored but **unused**: nothing references `crate::data`; overlaps streaming cache and db candles |

The architecture checker was tightened in July and found 52 hidden boundary violations across 7 forbidden edges, including a circular `patterns ↔ risk` dependency (B-070). Shell-script layering has not held.

---

## 5. Database, caches, persistence

- **Nothing opens a database in production on staging.** `Database::new` has no non-test callers; Tauri uses `new_with_caches`. CTL opens it in the Tauri app and falls back silently to display-only if it fails.
- **`Database::new` cannot create a file**: `create_if_missing` is never set (`connection.rs:80-86`). On a clean checkout the CTL app therefore silently drops to display-only.
- **Pragmas reach one pooled connection** (`connection.rs:105-132`); `cache_size = 1000000` is pages (~4 GB); `acquire_timeout` unset; `config::DatabaseConfig` never converted to `db::DatabaseConfig`; five different default DB paths across code and docs.
- **Money as `REAL`** (70 columns); `Decimal → f64` with `unwrap_or(0.0)` silently writing $0 (`repositories.rs:127,129,408,1152-1156,…`); `from_f64_retain` on read. `Quantity(u32)`: fractional shares truncated (`positions/tracker.rs:545`).
- **Hardcoded `account_id = 1`** (`positions/tracker.rs:601-617`, Tauri `state.rs:152,183`, CTL `main.rs:237`) and **nothing inserts an `accounts` row** outside tests; CTL's comment saying `setup_db.sh` creates it is wrong. With `foreign_keys=ON` (sqlx default) the first position save fails, after the in-memory cache was already mutated.
- **Fills never persisted** (no `INTO trades` anywhere); `filled_quantity/avg_price/filled_at` never written on fill; cancels never reach the DB because `cancel_order` filters on the overwritten internal id (`orders/engine.rs:316,485-500`); `StopLimit`/`TrailingStop` cannot be saved (`format!("{:?}").to_lowercase()` → `stoplimit`, fails `CHECK`, `engine.rs:~414`).
- **No `client_order_id`.** A crash between submit and the broker-id swap orphans a live broker order; a retry duplicates it.
- **No rehydration.** `get_active_positions`, `get_open_orders`, `get_all_pattern_stats`, `sync_from_broker` have no production callers (CTL too). Restart loses PDT count, daily P&L, halt state, pattern kill-switch state, open orders and positions.
- **Cache/DB drift.** Positions mutated before write with no rollback; removed from cache before archive; `archived_orders` unbounded; candles written only by the unused ingestion pipeline.
- **Migrations.** `MigrationManager` hardcodes v1/v2, no checksums, no transaction, never called; `sqlx::migrate!` used nowhere; `day_trades`/`pdt_audit_events` exist only in `migrations/001`, not in `schema.sql`.
- `rusqlite` still a dependency for one dead error variant and one test that builds its own throwaway tables.

---

## 6. Brokers, config, connections, paper vs live

- **No broker factory.** `AppConfig.broker.provider`/`is_paper` are never read. Nothing in `src/` constructs `AlpacaProvider`. Tauri hardcodes `MockBroker` (staging `main.rs:71`, CTL `main.rs:177`).
- **No live-trading guard.** `paper()` and `live()` differ only in `base_url` and an `is_paper` flag nobody reads (`alpaca/config.rs:7-41`). No flag/URL consistency check, no explicit opt-in, no banner.
- **Credentials.** No key loader in the library; `keyring` unused; `AlpacaConfig` and `RestClient` derive `Debug` with a plain `api_secret: String`. No hardcoded secrets found.
- **Config.** `default.toml` fails to deserialize on staging (`missing field api_key`); the test swallows the error (`tests/config_validation.rs:478-496`). CTL adds serde defaults so it loads, but the `pattern_profiles` and `confidence_multipliers` blocks are at the wrong nesting level and are **silently dropped** (measured: 0 profiles, 0 multipliers). `[database] enable_wal`, `broker.base_url`, `ws_url`, `[performance]` reconnect settings have no struct fields.
- **WebSocket.** Backoff exists now (`alpaca/backoff.rs`) but only covers the initial connect/auth-send path. `connect_and_run` returns `Ok` on server Close, read error, stream end, or failed ping, and `ws_handler` treats `Ok` as "closed normally" and exits for good (`websocket.rs:130-133,196-244`). No resubscribe, no pong timeout, `connect()` returns before any TCP/auth exchange, auth errors only logged, subscribe races auth. A dead stream is indistinguishable from a quiet one because `get_quote` serves the last value forever.
- **Parsing.** Any unknown message type fails the whole batch; prices via `from_f64_retain`; feed hardcoded to IEX; `Quote.last` is a stale bar close.
- **REST.** Rate limiter: **fixed** (`alpaca/ratelimit.rs`, 200/60 s token bucket). `Retry-After`: parsed. Still: `parse_order_status` rejects valid statuses (`pending_cancel`, `replaced`, `done_for_day`, `held`, …) and `get_open_orders` fails the whole call on one such order; fractional qty fails `u32` parse; `trail_price` sent as `trail_percent`; 403 mapped to `AuthenticationFailed` (Alpaca uses 403 for buying power and PDT denials); no `client_order_id`.
- **Historical bars.** Staging: "not yet implemented". CTL: `get_bars` with pagination, unit-tested; not connected to the backtester.
- **MockBroker.** Staging: never fills, never moves cash or positions, ignores order type/TIF/hours, unseeded RNG. CTL: opt-in `auto_fill_orders` fills in full at limit or last quote on the next `get_order` poll, regardless of whether the bar touched the limit; still no cash/position effect; `filled_at` is wall-clock.
- **Tauri.** Staging: `tokio::spawn` from a non-async `main` (guaranteed panic; CTL fixes with `#[tokio::main]`), `EventBridge` exits on `Lagged`, `std::sync::Mutex` caches, PDT re-implemented with hardcoded 3/$25k and permissive fallbacks. Tauri crate is not in CI.
- **Unused deps.** `crossbeam`, `tower`, `keyring`, `once_cell`, `sha2`, `url`, `tracing-appender`, `metrics-exporter-prometheus`, `ta`; `tokio-retry` only in dead `utils/retry.rs` (whose backoff computes 100^n ms); `rusqlite` effectively dead; `parking_lot` in one file; `log` and `tracing` mixed.

---

## 7. Backtesting with canned data

**Staging:** none. No loader, no replay, no fixtures, no binary; `get_candles` unimplemented; `ta` crate unused; candles never persisted.

**CTL (`src/execution/backtest.rs`, 841 lines):** a genuine v1 and the right shape. It steps a `SteppedClock` per candle, runs the production `PatternRegistry → SignalEngine → ExecutionEngine → OrderEngine → PositionTracker` synchronously, fills via `MockBroker` auto-fill, applies slippage (bps per side) and a flat commission in its own FIFO lot accounting, and produces a `BacktestReport` with counts, win rate, gross/net/open P&L, per-pattern attribution and an equity curve. One end-to-end test exists.

What stops it being usable as a validation gate today:

1. **No data source.** `run(Vec<Candle>)` is called by nothing; no CLI, no Tauri command, no CSV/Parquet loader, no fixtures under `tests/data/`. The new `get_bars` is not connected to it.
2. **Risk engine not started.** `RiskEngine` is constructed but `start()` is never called (documented as a v1 limitation), so no PDT, daily-loss, position-size or pattern-performance checks run. A backtest that bypasses the risk engine cannot validate the risk engine.
3. **Not deterministic.** `MockBroker` stamps fills with `Utc::now()` (so `holding_secs` is ~0), 81 wall-clock calls remain in the components it drives, order ids contain v4 UUIDs, per-pattern output order depends on `HashMap` iteration. The test asserts only `>=` bounds and never compares two runs.
4. **Fill model is optimistic.** Every limit order fills in the same step as the signal at the limit price, without checking the bar's high/low; buying power is a static $100k.
5. **Slippage/commission live only in the report's accounting,** not in the broker fill or the position tracker, so the strategy's view of its positions differs from reported P&L.
6. **Long-only; no exits.** Open positions are marked to last close. Since the live loop also cannot exit positions (section 3.1), the backtester inherits that.
7. **Masks a live bug.** It stamps `signal.symbol = candle.symbol` to work around the `"DEFAULT"` symbol, so the live path stays broken.

---

## 8. Comparison with established systems

What NautilusTrader, QuantConnect Lean, Freqtrade, Hummingbot, Backtrader and barter-rs share, and where this repo stands on `staging` (+CTL):

| Capability | Mature frameworks | DayTrader |
|---|---|---|
| One canonical domain model | Yes | 4–8 definitions per concept |
| One event loop, deterministic ordering, lossless order/fill path | Yes | Lossy `broadcast` everywhere; consumers die on lag |
| Injectable clock used by every time-dependent component | Yes | Trait exists; 1 consumer (+1 in CTL) |
| Same strategy/risk code in live, paper and backtest | Core principle | CTL backtester reuses engines but skips the risk engine |
| Fill model (touch logic, partials, slippage, latency) | Yes | Optimistic auto-fill on poll |
| Blocking pre-trade risk gate | Yes (Nautilus `RiskEngine` sits in the order path) | Async, advisory, log-only |
| Order state machine, idempotent `client_order_id` | Yes | Any transition accepted; no client id |
| Broker fill/trade-update stream driving positions | Yes | 1 s poll against mock only |
| Persist + rehydrate + reconcile against broker on start | Yes | None |
| Explicit sim/paper/live mode with guard rails | Yes | A `bool` nobody reads |
| Exchange calendar with DST, holidays, early closes | Yes | DST + holidays now; no early closes; PDT still UTC-dated |
| Fixed-point money end to end including storage | Yes | `Decimal` in memory, `f64` in DB and config |
| Compile-enforced module boundaries | Crates | Shell scripts; 52 hidden violations found when tightened |

---

## 9. Recovery strategy

### 9.1 On splitting into two repositories

**Recommendation: do not split the repository. Split the crate.**

The repo is not large (≈35k lines of Rust, ≈1.8k of TypeScript). The problems are 50 stale branches, a six-month gap between `main` and `staging`, four schema sources, duplicated types, and documentation that reports phases as complete when the runtime paths are stubs. None of those is fixed by a second repository, and all of them get harder to manage across two. The existing ADR (`plans/architecture-decision-frontend-separation.md`) already makes the right call against separating the Tauri frontend, because Tauri's IPC and build are designed around co-location. A "core vs app" split would add cross-repo versioning and release coordination for a one-person project whose core API is about to change substantially during consolidation.

What *would* give you the boundary discipline you are after is converting the single `daytrader` crate into a **Cargo workspace of small crates**. Cargo then enforces the dependency graph at compile time, which the `verify_architecture.sh` approach has demonstrably not done, and each crate gets its own `cargo test`, its own clippy budget, and a README that describes something small enough to be true. Suggested shape:

```
crates/
  dt-core        domain types (Bar, Quote, Order, Fill, Position, Money=Decimal, Symbol),
                 Clock trait, exchange calendar, typed errors. No I/O, no tokio.
  dt-strategy    indicators, candlestick patterns, signal generation   -> core
  dt-risk        risk engine, PDT, limits, sizing                     -> core
  dt-engine      order lifecycle, positions, execution loop, pipeline -> core, risk, strategy
  dt-broker      broker traits + SimBroker (deterministic fill model)  -> core
  dt-alpaca      Alpaca REST + WS implementing dt-broker traits        -> core, broker
  dt-store       sqlx persistence, migrations, repositories            -> core
  dt-backtest    replay runner, data loaders, report                   -> engine, broker, store
bins/
  dtctl          headless CLI: `run --paper`, `backtest --data ...`, `reconcile`
apps/
  desktop        Tauri + React, depends on dt-engine through a thin facade
```

The frontend stays where it is. If a second repo ever makes sense, it is for a published **historical data set** (Parquet fixtures, hundreds of MB), not for code.

### 9.2 Process changes that make the rest possible

- **Stop feature work until the build is honest.** Nothing new lands until CI is green from a clean clone on the trunk.
- **One trunk.** Make `staging` the default branch (or fast-forward `main` to it and delete `staging`), delete the ~50 dead branches, and open PRs only against the trunk. The `main ← staging ← feat/*` ladder is more process than a solo project needs and is currently not followed anyway (`main` is six months behind).
- **CI is the verifier.** Make `scripts/verify_agent_work.sh` and `ci.yml` run the identical steps, with `SQLX_OFFLINE=true` and a committed `.sqlx/` cache, `Cargo.lock` committed, and the Tauri crate built (headless, no bundle). Delete the "known failures ≤ N" escape hatch; a test is either green, fixed, or deleted with a note.
- **Retire the governance that is not working.** Keep `CLAUDE.md`, `BACKLOG.md` and the hooks that block dangerous commands. Replace the per-module `CLAUDE.md` boundary docs and `verify_architecture.sh` with crate boundaries. Replace the `plans/` and `reports/` "phase complete" narratives with a single `STATUS.md` that lists what runs end to end, verified by a named test.
- **"Done" means a test proves it.** Every BACKLOG item names the test that demonstrates it. Agents (and humans) are not allowed to mark Done on a reading of the code.

### 9.3 Sequenced plan with exit criteria

**Phase 0: Truth and build (≈1 week)**
- Decide the fate of the three open PRs: merge [#29](https://github.com/mhobbs07/daytrader/pull/29) and [#30](https://github.com/mhobbs07/daytrader/pull/30) after fixing the `"DEFAULT"` symbol in the live path (not just the backtester), and close [#6](https://github.com/mhobbs07/daytrader/pull/6) (superseded).
- Commit `Cargo.lock` and `.sqlx/`; `SQLX_OFFLINE=true` in CI, Dockerfile, verify script; CI triggers on the trunk and all PRs; Tauri crate in CI.
- Burn down clippy to zero under `-D warnings`, or land an explicit `#![allow]` list in `lib.rs` that is ratcheted down per PR. 1,792 is not "~130" and the policy should say which it is.
- Delete dead code and deps: `utils/retry.rs`, `SignalStreamingPipeline`, `db/migrations.rs` + `migrations/`, `OrderRepository`, `src/data/` (or wire it, see Phase 3), the unused dependencies in section 6.
- Fix the benches or delete them.
- **Exit:** clean clone → `cargo clippy -D warnings`, `cargo test`, `cargo doc` all green in CI on the trunk.

**Phase 1: Canonical model and workspace (≈2–3 weeks)**
- Create `dt-core` with exactly one `Bar`, `Quote`, `Order`, `OrderStatus` (one state machine with validated transitions), `Fill`, `Position`, `Money` (Decimal), `Quantity` (Decimal), `Symbol`, `ClientOrderId`, `Clock`, `TradingCalendar`.
- Move every module to use them and delete the converters. Collapse the four position stores into one `Portfolio` in `dt-engine`; the risk engine reads from it instead of keeping its own.
- Thread `Clock` through risk, PDT, orders, positions, the sim broker. Target: zero `Utc::now()` outside `SystemClock`.
- Store money as `TEXT` Decimal (or integer minor units), quantities as Decimal; one schema source via `sqlx::migrate!` run at startup; `create_if_missing`; pragmas via `SqliteConnectOptions`.
- **Exit:** `grep -r "Utc::now()" crates/ --include=*.rs | grep -v clock.rs | grep -v cfg(test)` is empty; one type per concept; workspace builds with crate-enforced boundaries.

**Phase 2: Make the risk engine correct (≈1–2 weeks)**
Fix, each with a deterministic test that publishes the triggering event into a *started* engine with a `FixedClock`:
deadlock lock order; `Lagged` handling; market-order notional; PDT `>=`/`>` and 5-day window and ET dating; daily-loss evaluation wiring, commission sign, unrealized accumulation, single halt flag, emergency threshold semantics; daily reset on ET date roll; inverted multiplier; `RiskRuleEngine` sharing the real trackers; un-awaited `update_pattern_status`; eager `[0]` index; guards across `.await`; config nesting so `pattern_profiles`/`confidence_multipliers` actually load.
- **Exit:** a property/scenario test suite in `dt-risk` that replays a scripted day (fills, P&L swings, 4th day trade, midnight ET roll) and asserts halts and rejections at the right moments.

**Phase 3: One real loop (≈3–4 weeks)**
- `dt-engine` owns a single ordered pipeline: `Bar → Strategy → Signal → RiskGate::check(&OrderIntent) -> Result → Broker::submit(client_order_id) → FillStream → Portfolio → Persist(txn) → RiskState`. The risk gate is a synchronous call in the order path; the event bus is for fan-out to UI and metrics only. Use bounded `mpsc` for the order/fill path.
- Fill path: Alpaca `trade_updates` WebSocket (plus a reconciliation poll), partial fills, cancels with filled shares, `OrderRejected` on submit failure.
- Exits: evaluate exit rules per bar; stop-loss / take-profit / end-of-day flatten; `close_position` places a real order.
- Persistence: fills into `trades`, order status by broker id, one transaction per fill (order + trade + position + risk counters); rehydrate open orders/positions/PDT/daily P&L on start and reconcile against the broker.
- Broker layer: `BrokerKind::{Sim, Paper, Live}` factory; `Live` requires an explicit flag **and** URL check **and** a typed confirmation; redacted `Debug` for secrets; env/keyring loader; WebSocket reconnect on any exit with resubscribe and pong timeout; complete status enum; 403/422 mapping; fractional quantities.
- Market data: one `MarketDataSource` trait; Alpaca stream publishes `Bar`s into the pipeline; the `data::ingestion` validation and candle persistence sit behind it or are deleted.
- **Exit:** `dtctl run --paper` runs for a full session against Alpaca paper, places and closes trades, survives a WebSocket drop and a process restart without losing state, and `dtctl reconcile` shows zero drift.

**Phase 4: Backtesting as the gate (≈2 weeks, can overlap Phase 3)**
- `SimBroker` in `dt-broker`: bar-touch fill logic for limits/stops, next-bar-open for market, configurable slippage/commission/latency applied at the fill (so the portfolio and the report agree), seeded RNG, fills stamped with the stepped clock.
- Data: `dtctl fetch` pulls Alpaca bars into Parquet or CSV under `data/`; commit a small fixture set (e.g. SPY, AAPL, TSLA 1-min for ~10 trading days spanning a DST change and a holiday) under `tests/data/`.
- Runner drives the same `dt-engine` pipeline with the risk engine **started**, deterministic ids, stable output ordering; a test runs the same fixture twice and asserts byte-identical reports.
- Metrics: drawdown, Sharpe/Sortino, profit factor, per-pattern, per-regime; JSON + Markdown report.
- **Exit:** the fixture backtest is a CI test; a change to risk or strategy code that moves the result fails CI until the expected report is updated deliberately.

**Phase 5: Paper, then live (≥30 trading days)**
- 30 days on Alpaca paper with the same binary; compare to the backtest over the same window; the live/paper gap and the risk-event log are the go/no-go inputs the existing `plans/backtesting-requirements.md` already describes.
- Only then implement the live opt-in path.

### 9.4 Executing the plan with a smaller model

The plan above assumes most of the hands-on work will be done by a cheaper model. That is workable for most of it, with these adjustments. The history of this repo is itself the evidence: the phases that went wrong are the ones where an agent was given a broad goal ("implement Phase 4") and allowed to declare it done.

**Split the work by who should do it.**

| Work | Who | Why |
|---|---|---|
| Phase 0 (build, CI, deletions, clippy burn-down) | small model | mechanical, CI is the judge |
| Designing `dt-core` types, the `OrderStatus` state machine, the `RiskGate` and `Broker` traits, the pipeline shape | **strong model or you**, once, up front | this is the one architectural decision everything else depends on; a weak model will reintroduce the duplication it is supposed to remove |
| Migrating each module onto `dt-core`, crate by crate | small model | mechanical once the target types exist and compile |
| Each risk-engine bug in Phase 2 | small model, one bug per task | each has a known location and a scripted test |
| Phase 3 pipeline skeleton and transaction boundaries | strong model for the skeleton; small model for filling in broker/persistence details | ordering and atomicity decisions are easy to get subtly wrong |
| `SimBroker` fill model, data loaders, report metrics | small model | well-specified, testable in isolation |
| Reviewing PRs | strong model, or CI alone | a second reading by the same small model adds little |

**Write tasks so the model cannot drift.** Each BACKLOG item should carry, in this order: the exact files it may touch, the test it must make pass (written or at least named before the task starts), the command that proves it (`cargo test -p dt-risk pdt_window`), and the sentence "do not change anything outside these files". Tasks of more than ~300 changed lines should be split before they are handed out. The `/backlog-task` skill already has roughly this shape; tighten it so that "Done" is set by CI, not by the agent.

**Give it guard rails that do not depend on its judgment.**
- Crate boundaries in Cargo (Phase 1) rather than shell scripts, so a forbidden import is a compile error.
- `-D warnings` and `clippy::pedantic` from the start of each new crate, so the budget never grows again.
- Branch protection on the trunk requiring green CI; no direct pushes, by agents or anyone.
- Keep the hook that blocks `git push --force`, `rm -rf` and friends.
- A `preflight` job that greps for `Utc::now()`, `unwrap()` outside tests, `f64` in money fields and `std::sync::Mutex`, as ratchets that can only decrease.

**Order Phase 1 for a small model.** Do not ask it to "consolidate the domain types". Instead: (1) a strong model writes `dt-core` with doc comments and unit tests and merges it unused; (2) the small model migrates one leaf module at a time (`brokers`, then `events`, then `positions`, then `orders`, then `risk`, then `execution`), each as its own PR that deletes the old type and its converter and leaves the suite green; (3) the final PR deletes anything in the old locations that is now unreferenced. Each step is small, reversible and verifiable.

**Phase 2 as a checklist.** Hand each bug over as "file, line, what is wrong, the test that must pass". Section 3.2 of this document is already close to that form. Insist that the test publishes the real event into a started engine with a `FixedClock`; a test that calls the inner function directly would have passed on today's code too.

### 9.5 What to do with the existing documents

`plans/` and `reports/` currently describe Phases 1–4 as complete. Archive them under `docs/history/` and replace with `STATUS.md` (what runs, proven by which test), `ARCHITECTURE.md` (the crate graph, one page), and the backlog. Future agents and future you will trust the docs; make them true.

---

## Appendix: verification notes

- `main` (`5b09248`): fails `cargo check` with E0583 (`src/data` swallowed by `.gitignore`); with a placeholder module and provisioned DB, 213 unit tests pass; benches and three integration test files do not compile. All seven CI Pipeline runs on `main` fail at `cargo fmt` for the E0583 reason.
- `staging` (`e61e85a`): numbers in the table at the top. The library compiles with 691 warnings; three benches fail; 27 tests ignored.
- CTL (`6cd5450`): library compiles with `DATABASE_URL`; Tauri crate could not be built in this environment (no GTK system libraries), so Tauri conclusions are from reading.
- The `validate-branch-name` workflow on `main` rejects every branch name the repo's own branching rules prescribe; `staging` has already deleted that workflow.
