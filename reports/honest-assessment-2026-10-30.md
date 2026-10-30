# DayTrader: Honest Whole-App Assessment (2026-10-30)

Scope: architecture, logical flow, duplication, database/caches, real vs mock trading, connection setup, backtesting readiness, and a comparison against established trading frameworks. All claims below were checked against the code at commit `5b09248` (`main`). File references are `path:line`.

Verification performed in this session:

- `cargo check` at HEAD: **fails** (46 errors).
- Scratch copy with a placeholder `src/data/mod.rs` and a prepared `DATABASE_URL`: library compiles with 71 warnings; benches and several integration tests still fail to compile; unit-test result recorded in section 1.
- Three targeted code audits (trading loop, broker/config, DB/persistence) plus spot-checks of every high-severity claim.

---

## 1. The repo does not build from a clean clone

| # | Problem | Evidence |
|---|---------|----------|
| 1.1 | `.gitignore` line 27 is `data/`, which also matches `src/data/`. `src/lib.rs:16` declares `pub mod data;` but the directory was never committed. Every clean checkout fails with E0583. | `git check-ignore -v src/data/mod.rs` → `.gitignore:27: data/` |
| 1.2 | 46 `sqlx::query!` / `query_as!` macro sites need either `DATABASE_URL` pointing at a database that already has the full schema **plus** the `schema_migrations` table, or a `.sqlx/` offline cache. `.sqlx/` is gitignored (`.gitignore:13`), no `.env.example` exists, and neither CI (`.github/workflows/ci.yml`), the Dockerfile, nor `scripts/verify_agent_work.sh` sets `DATABASE_URL` or `SQLX_OFFLINE`. | `cargo check` output; scratch build needed a hand-built DB |
| 1.3 | Benches do not compile (`benches/risk_benchmarks.rs:390,506` reference `Price`/`Fill` without imports; `pattern_signal_benchmarks` has 6 errors). `tests/risk_engine_integration.rs:33` references a field that no longer exists. `tests/pattern_streaming_pipeline_integration.rs` uses `sqlx::PgPool` with no `postgres` feature. `tests/risk_database_integration.rs` needs `--features mock`. | Scratch `cargo check --all-targets` |
| 1.4 | The Tauri crate cannot compile: `Arc::clone(&event_bus)` on a non-Arc (`src-tauri/src/main.rs:126`), imports of undeclared modules `risk::position_tracker` and `utils::holidays` (`state.rs:12-13`), `tokio::spawn` from a non-async `main` (`main.rs:149`), `db::repositories::PdtRepository` does not exist (`state.rs:5,203`). No Tauri artifacts in `target/`, so it has apparently never been built. | Read + grep |
| 1.5 | Three directories are orphaned (not in any `mod` tree, never compiled): `src/domain/`, `src/infrastructure/`, `src/risk/position_tracker.rs`, `src/utils/holidays.rs`. The infrastructure code references types and functions that do not exist. The "B-042 PDT holidays" work landed in a file that is not compiled. | `grep "^pub mod" src/risk/mod.rs src/utils/mod.rs src/lib.rs` |
| 1.6 | `Cargo.lock` is gitignored for an application crate. Builds are not reproducible. | `.gitignore:3` |
| 1.7 | Governance docs describe `main ← staging ← feat/*`; no `staging` branch exists on the remote, and CI triggers on `main`/`develop` (no `develop` either). The verify script passes unit tests when failures are ≤ the count of lines in `known_test_issues.txt` (21 entries) and uses clippy `-W` while CI uses `-D warnings`. The claims of "235/235 passing" in `BACKLOG.md` cannot currently be reproduced from git. | `git branch -a`, `scripts/verify_agent_work.sh:94-115` |

Unit test result from the scratch build: see addendum at the bottom.

---

## 2. Nothing runs end to end

`src/main.rs` initialises logging and metrics, logs "Phase 0", then exits (`// TODO: Initialize application in Phase 1`). No production code constructs `ExecutionEngine`, `OrderEngine`, `positions::PositionTracker`, `Database`, `MigrationManager`, or `AlpacaProvider`. The Tauri app builds a `RiskEngine` and a `MockBroker` and nothing else.

Stage-by-stage status of the trading loop:

| Stage | Status |
|-------|--------|
| WebSocket bars → `MarketEvent::NewCandle` | **Not wired.** `alpaca/websocket.rs:280` stores bars in a `HashMap`; nothing publishes to the `EventBus`. The only `NewCandle` publishers are tests. |
| Candle → pattern → signal | Logic exists (`patterns/streaming.rs`); built only in an integration test. |
| Signal → `ExecutionEngine` | Never constructed anywhere, including tests. |
| Pre-trade risk check | **Advisory only.** `orders/engine.rs:186` checks a halt flag. The real rule evaluation runs asynchronously on the `OrderPlaced` event, which is published before submission, and errors are only logged (`risk/engine.rs:181-183`). This directly contradicts `.claude/rules/security.md` rule 4. |
| Broker fill → order tracker → position | **No fill path exists.** `OrderExecutionProvider` has no stream/callback; there is no Alpaca `trade_updates` subscription; `sync_order_status` is never called; `PositionTracker::update_from_fill` is never called. |
| Position close → daily P&L → halt | Unreachable; `close_position` returns a fake id without sending anything (`positions/tracker.rs:510-526`). |

### Runtime bugs that would bite the moment it is wired

- **Deadlock.** `risk/engine.rs:408-409` takes write locks on both `pattern_risk_evaluator` and `pattern_stats_tracker`, then calls `evaluate_pattern_trade`, which takes `stats_tracker.read()` on the same `Arc` (`pattern_limits.rs:459`; linked at `engine.rs:112`). Any pattern that passes the confidence threshold hangs the risk task forever.
- **Guaranteed panic.** `Price::new(Decimal::ZERO).unwrap()` at `risk/engine.rs:389`; `Price::new` rejects non-positive values (`brokers/types.rs:99`). Runs on every detected pattern when a database is attached. Release profile is `panic = "abort"`.
- **Lagged subscribers die silently.** All production consumers use `while let Ok(ev) = rx.recv().await` (`execution/engine.rs:272`, `risk/engine.rs:180`, Tauri `main.rs:156`, `events.rs:39`). The first `RecvError::Lagged` exits the loop; the risk engine stops evaluating while trading remains "allowed". The project's own concurrency rule forbids this.
- **Market orders bypass every notional check.** `estimated_value()` is `None` for market orders, so buying power is checked against $0 (`orders/engine.rs:333-341`) and `TradeContext` gets price 0 (`risk/engine.rs:252`). The default `ConservativeStrategy` places market orders.
- **PDT is never enforced pre-trade.** `is_day_trade` is hardcoded `false` (`risk/engine.rs:254`); every fill is recorded as a day trade (`engine.rs:311-313`); the window covers 6 business days (`pdt.rs:126,140`); holidays are ignored because `holidays.rs` is orphaned. The count resets to zero on restart.
- **Daily loss limit never halts.** `DailyLossTracker` returns a `RiskEvaluation` that is discarded at `risk/engine.rs:317,344,361`. Commission is added to P&L as profit (`:316-319`). Two unlinked halt flags exist (`RiskState.is_trading_halted` and `DailyPnlState.is_halted`).
- **Market hours use the UTC hour.** `MarketHoursRule` compares `current_time.hour()` to 9–16 with `Utc` (`execution/rules.rs:190-196`). That permits trading from 4–5 AM ET and blocks the afternoon session. ET is hardcoded as UTC−5 with no DST elsewhere (`risk/limits.rs:390-393`). No `chrono-tz` dependency.
- **Emergency liquidation misfires.** `check_emergency_liquidation(daily_pl)` compares daily P&L to a $500 *balance* threshold (`risk/engine.rs:367,578`), so any close while daily P&L ≤ +$500 trips it. No liquidation is implemented.
- **Inverted risk multiplier.** Worse-performing patterns get a *larger* multiplier (`pattern_stats.rs:337-339`), which feeds position size (`pattern_limits.rs:465`).
- **Risk rule engine evaluates empty state.** `RiskRuleEngine::new` builds its own `DailyLossTracker`, `PositionSizer` (equity 0) and `PdtEnforcer` (equity 0), separate from the ones the `RiskEngine` updates (`risk/rules.rs:476-499`, `risk/engine.rs:95-101,158`).
- Un-awaited async call: `evaluator.update_pattern_status(...)` at `risk/engine.rs:451` never runs, so auto-disable never happens.
- `DashMap` `RefMut` held across `.await` in `positions/tracker.rs:266→290`, `343→409`, `431→440`.
- `Decimal::from_f64_retain(0.7) < 0.70`, so confidence threshold comparisons fail at the boundary (`execution/engine.rs:316`, `execution/rules.rs:102`).
- `ExecutionEngine` re-publishes `PatternDetected` (`engine.rs:650,673`), so the `RiskEngine` double-counts signals. `SignalStreamingPipeline` never clears its buffers after `process_batch` (`streaming.rs:563-583`).

---

## 3. Duplication

The codebase is about 31.7k lines; a large fraction is the same concept defined several times with hand-written, lossy converters between them.

- **Position:** `brokers::traits::Position`, `events::types::Position`, `positions::tracker::Position`, `risk::position_sizing::PositionInfo`, `db::PositionRow`, `AlpacaPosition`, plus the orphaned `risk::position_tracker::TrackedPosition`. Four independent position stores exist, none synchronised.
- **Order / status / side:** `brokers::traits` (5 types, 7 statuses) vs `events::types` (2 types, 6 statuses) vs `orders::types::ManagedOrder` vs `OrderRow`. Mapping `ManagedOrder → events::Order` turns Stop/StopLimit/Trailing into Market and Expired into Cancelled (`orders/tracker.rs:86-124`).
- **Candle / Quote / Fill:** each defined twice (`brokers/traits.rs` and `events/types.rs`).
- **Pattern stats:** seven types across `risk::pattern_stats` (two), `execution`, `execution::rules`, `patterns::types`, `db::models`, `domain::models`, using three different win-rate scales (0–1, 0–100, Decimal). None is ever fed a real trade.
- **Risk configuration thresholds** live in `execution::PerformanceThresholds`, `risk::types::PatternPerformanceThresholds`, and `RiskConfig`.
- **P&L calculation** appears in `positions/pnl.rs`, `position_sizing.rs:98-118`, `events/types.rs:443-451`, `execution/rules.rs:335,370`. FIFO tax lots are the default but never used; realised P&L is always average cost.
- **Repositories:** two parallel abstractions plus two mock sets: concrete sqlx structs in `db/repositories.rs`, `db/traits.rs::PdtRepositoryTrait`, `domain/ports/repositories.rs` (f64 models, not compiled), `infrastructure/repositories.rs` (not compiled, calls non-existent methods). `OrderEngine` writes raw SQL (`orders/engine.rs:401-473`) while `db::OrderRepository` sits unused.
- **Schema:** four sources (`src/db/schema.sql`, `migrations/001+002`, DDL inside `migrations.rs` macros, `scripts/setup_db.sh`). Already drifted: `day_trades` and `pdt_audit_events` exist only in `migrations/001`, not in `schema.sql`, which is the one that actually runs.
- **Tauri** re-implements PDT (hardcoded 3 trades/$25k, `commands/risk.rs:217,236,369`), weekday helpers, market-data and pattern caches, and a hand-copied fallback `RiskConfig`.
- Enum→string mappers copy-pasted in `orders/engine.rs:496-516` and `db/repositories.rs:1030,1605`.

---

## 4. Database and caches

- **Nothing opens a database in production.** `Database::new` is only called by tests. It also cannot create a new file: sqlx's default `create_if_missing` is false (`db/connection.rs:80-86`), so a fresh path fails with SQLite error 14 unless `setup_db.sh` ran first.
- **Pragmas only reach one pooled connection.** `PRAGMA synchronous/cache_size/temp_store/mmap_size` are executed via `&self.pool` (`connection.rs:105-130`); the other connections keep defaults. `cache_size = 1000000` is pages, not KiB (≈4 GB per connection); almost certainly meant `-1000000`. `busy_timeout` and `acquire_timeout` are not configured.
- **Money is stored as `REAL`.** Every price/P&L column is f64; writes go `Decimal → f64` with `.unwrap_or(0.0)` (silently writing $0 on failure, `db/models.rs:259,276,278,299,415`); reads use `from_f64_retain`, which carries binary noise (150.1 → 150.0999999…). This violates the project's own "Decimal for money" invariant at the storage layer.
- **Quantities are `u32`**, so fractional shares (which Alpaca supports) cannot be represented or parsed (`alpaca/types.rs:117-125`).
- **No state is reloaded on startup.** `get_active_positions`, `get_open_orders`, `get_all_pattern_stats`, `sync_from_broker` have no production callers. After a restart: PDT count = 0, daily P&L = 0, halt flag cleared, disabled patterns re-enabled, open orders and positions unknown.
- **Fills are never persisted.** Nothing inserts into `trades`. Order rows stay `'pending'` forever. Cancels never reach the DB because `update_order_status_in_db` filters on the internal id after the column was overwritten with the broker id (`orders/engine.rs:446-473`).
- **No idempotency key.** `client_order_id` appears nowhere. A crash between submit and the broker-id swap orphans a live broker order; a retry duplicates it.
- **Stop-limit and trailing-stop orders cannot be saved:** `format!("{:?}").to_lowercase()` produces `"stoplimit"`, which fails the `CHECK(order_type IN ('stop_limit', …))` constraint (`orders/engine.rs:385`, `schema.sql:101`).
- **FK failures on every position write:** `account_id = 1` is hardcoded (`positions/tracker.rs:602,617`) and nothing inserts into `accounts` outside tests; with `foreign_keys=ON` the write is rejected after the in-memory cache was already updated.
- **Caches drift from DB.** `positions::tracker` mutates the DashMap first and writes second with no rollback; on close it removes from cache before archiving. `OrderTracker.archived_orders` grows unbounded. Candle cache is bounded per symbol but unbounded in symbols; candles are never persisted.
- `rusqlite` and `sqlx` are both dependencies; `rusqlite` is used only for one dead error variant and one test file that creates its own throwaway tables (`tests/database_integration.rs`).
- Migrations: `MigrationManager` hardcodes versions 1 and 2, no checksums, no transaction around `migrate_to_v2`, and is never called. Tests use `sqlx::migrate!` against a different schema. Production runs neither.
- `[database] path`, `max_connections`, `enable_wal` in `config/default.toml` are never converted into `db::DatabaseConfig`. Four different default DB paths exist.

---

## 5. Real vs mock trading and connections

- **No broker factory, no selection logic.** `AppConfig.broker.provider` and `is_paper` are never read by code that builds a broker. Nothing in `src/` constructs `AlpacaProvider`; only the two examples do. Tauri hardcodes `MockBroker`.
- **Nothing guards against live trading.** `AlpacaConfig::paper()` and `live()` differ only in `base_url` and an `is_paper` flag that is never read outside tests (`alpaca/config.rs:22-41`). All fields are `pub`. There is no URL/flag consistency check, no explicit opt-in, no startup banner. Today you are protected only because nothing wires Alpaca up.
- **`config/default.toml` cannot deserialize into `AppConfig`.** `BrokerConfig` requires `api_key`/`api_secret` with no default (`config/mod.rs:35-36`); the toml correctly omits them. `test_load_default_config` swallows the error (`tests/config_validation.rs:474-476`). `[pattern_profiles.*]`, `[[confidence_multipliers]]` are top-level in the toml but expected under `risk.*` in the struct, so they would be silently dropped. `base_url`/`ws_url` in the toml have no struct fields.
- **Credentials:** no hardcoded keys found (good). But nothing in the library loads them; `keyring` is a dependency with zero uses despite the security rule; `AlpacaConfig` and `RestClient` derive `Debug` with a plain `api_secret: String`, so any `{:?}` log leaks the secret.
- **WebSocket does not reconnect after a disconnect.** `connect_and_run` returns `Ok(())` on server Close, read error, or failed ping (`websocket.rs:177-188,211-213,225`) and `ws_handler` treats `Ok` as "closed normally" and exits (`:122-125`). Reconnect happens only on an `Err` from the initial connect/auth/subscribe sends. Fixed 5 s retry, no backoff, no resubscribe (`subscribed_symbols` is tracked but never replayed), no pong timeout. `connect()` returns `Ok` before any TCP/auth exchange, and auth failure is just `log::error!`. `get_quote` returns the last quote forever with no staleness check. The `alpaca/README.md` advertises backoff, resubscribe, and rate-limit retry that do not exist.
- **Message parsing drops whole batches** on any unknown `T` value (`websocket.rs:20-46,231-239`). Prices go `f64 → Decimal::from_f64_retain`. Feed is hardcoded to IEX for both paper and live; no SIP option. `Quote.last` is the last bar close (up to a minute stale) or falls back to ask.
- **No rate limiter.** Alpaca allows 200 req/min; `tower` is a dependency with the `limit` feature and zero uses. `utils/retry.rs` (`RetryExecutor`, `CircuitBreaker`) is dead code, and its backoff `from_millis(100).factor(2)` yields 200 ms → 20 s → cap, not doubling.
- **Alpaca type mapping will break on real responses:** `parse_order_status` rejects valid statuses (`pending_cancel`, `replaced`, `done_for_day`, `held`, …) and `get_open_orders` fails the whole call on one such order (`alpaca/types.rs:323-336`, `rest.rs:166-169`). 403 is mapped to `AuthenticationFailed`, but Alpaca returns 403 for insufficient buying power and PDT denials (`rest.rs:118-120`). `trail_price` is sent as `trail_percent` (`types.rs:155-161` vs `:400`). `Retry-After` is ignored.
- **MockBroker cannot simulate execution.** `place_order` sets `Accepted` and stores the order; it never fills, never moves cash/positions, ignores limit/stop logic, market hours, and time-in-force (`mock.rs:315-320`). Error injection uses unseeded `rand::random`, so runs are not reproducible. `start_stream` flips a bool and emits nothing (`:279`). It is a stub for unit tests, not a paper-trading or backtesting simulator.
- **No `Clock` abstraction.** About 120 non-test `Utc::now()` call sites (`risk/pdt.rs` 19, `risk/engine.rs` 11, `orders/tracker.rs` 8, …). PDT windows, daily resets, and timestamps all read wall-clock time.
- Dependencies declared but unused in `src/`: `crossbeam`, `tower`, `keyring`, `once_cell`, `ta`, `sha2`, `url`, `tracing-appender`, `metrics-exporter-prometheus`; `tokio-retry` is used only by dead code; `rusqlite` is effectively dead; `log` and `tracing` are mixed; `parking_lot` and `std::sync::RwLock` appear despite the "tokio::sync only" rule.

---

## 6. Backtesting with canned data

**There is no backtesting capability today**, and the current design makes adding one harder than it should be.

- No historical data loader, no CSV/Parquet reader, no replay engine, no `bin/backtest`, no fixture data files anywhere in `tests/`, `benches/`, or `examples/`. `plans/backtesting-requirements.md` is a requirements doc with every checkbox unticked and references Python scripts and a DuckDB store that do not exist.
- `AlpacaProvider::get_candles` returns `Err("Historical candles not yet implemented")` (`alpaca/mod.rs:177-190`), so you cannot even backfill from Alpaca.
- The `ta` crate (indicators) is a dependency with zero uses; indicators are hand-rolled in `patterns/indicators.rs` with `f64` fields.
- Candles are never written to the `candles` table, so there is no accumulated local history.

Why the current code cannot be driven by canned data without surgery:

1. **No injectable clock.** Every risk/PDT/daily-reset path calls `Utc::now()`. A replay of 2022 data would be evaluated against today's date.
2. **No execution simulator.** `MockBroker` never fills. A backtest needs a fill model (next-bar-open for market orders, touch logic for limits, slippage, partial fills).
3. **No fill path into positions.** Even with a simulator, there is no channel from "order filled" to `PositionTracker`, so no P&L, no equity curve.
4. **Market data enters via a side-channel `HashMap`,** not the event bus, so there is no single `MarketEvent::NewCandle` ingress to replay into.
5. **The event bus is a lossy `broadcast`.** Backtests need deterministic, lossless, ordered delivery (a bounded `mpsc` per stage or a synchronous pipeline), otherwise results vary run to run.
6. **Pattern stats, risk state and results are never persisted or reloaded,** so there is nothing to produce a report from.

Minimum design that would work with this codebase: a `Clock` trait (`SystemClock` / `SimClock`); a `MarketDataSource` trait with `AlpacaStream` and `CsvReplay` implementations that both publish `NewCandle`; a `SimBroker` implementing `OrderExecutionProvider` with a configurable fill model and seeded RNG that publishes `OrderFilled`; a synchronous `BacktestRunner` that steps the clock per bar and drives the same `ExecutionEngine`/`RiskEngine` used live; and a `BacktestResults` aggregator (equity curve, drawdown, per-pattern stats). A handful of committed CSV fixtures (e.g. SPY/AAPL 1-min for a few known days) under `tests/data/` would then give deterministic regression tests for the whole loop.

---

## 7. Comparison with established trading systems

What NautilusTrader, QuantConnect Lean, Freqtrade, Hummingbot, Backtrader and barter-rs all have, and this app lacks:

| Capability | Mature frameworks | DayTrader |
|-----------|-------------------|-----------|
| Single event loop with one canonical event stream and deterministic ordering | Yes (Nautilus message bus, Lean algorithm loop) | Lossy `tokio::broadcast`, side-channel HashMaps, consumers that die on lag |
| Same strategy/risk code runs live and in backtest, differing only by injected `Clock` + data source + execution venue | Core design principle in all of them | No clock abstraction; no replay source; mock cannot fill |
| Backtest engine with fill models, slippage, commissions, latency | Yes | None |
| Canonical domain model (one `Order`, one `Position`, one `Fill`, one `Bar`) | Yes | 6+ Position types, 4 Order types, 2 of each Candle/Quote/Fill |
| Order state machine with validated transitions and idempotent `client_order_id` | Yes | Any transition accepted; no client order id |
| Fill/trade-update stream from the broker driving positions | Yes | Missing entirely |
| Pre-trade risk gate that is synchronous and blocking | Yes (Nautilus `RiskEngine` sits between strategy and exec client) | Asynchronous, advisory, log-only |
| Durable state (positions, open orders, risk counters) persisted and rehydrated on start; reconciliation against the broker on connect | Yes | Nothing reloaded; `sync_from_broker` never called |
| Explicit, hard-to-miss live/paper/sim mode with guard rails | Yes | A `bool` nobody reads |
| Timezone-aware trading calendar (exchange calendar, DST, holidays, half-days) | Yes | UTC hours, UTC−5 constant, holiday code not compiled |
| Reconnecting data clients with backoff, resubscribe, heartbeat timeouts | Yes | None of the three |
| Money as fixed-point end to end including storage | Yes | `Decimal` in memory, `f64` in the DB and config |

Structural observations:

- **Layer count is far too high for the amount of behaviour.** There are 13 top-level modules, a domain/ports/infrastructure hexagonal layer (uncompiled), per-module `CLAUDE.md` boundary docs, an exceptions file, and verification scripts, wrapped around a system that has never executed a single trade. The governance apparatus is more complete than the product. Mature Rust trading code (barter-rs, Nautilus core) keeps a small number of crates with one canonical type per concept.
- **The "strict downward dependency graph" is already violated** in several places (`risk → patterns`, `events → patterns`, `db → risk/positions`, `positions → db`, raw SQL in `orders`), and the exceptions file legitimises more. The layering documentation describes an aspiration, not the code.
- **The plan/report documents describe phases 1–4 as "complete"** while the corresponding runtime paths are stubs or unreachable. This gap between documentation and reality is the single biggest process risk: future work (human or agent) trusts the docs.
- **Test suite shape**: 66 `sleep(Duration)` calls in `tests/` show that integration tests coordinate via timing rather than deterministic signals; they will be flaky and they mask the lossy-bus problem. 21 unit tests are permanently excused via `known_test_issues.txt`.

---

## 8. What to do, in order

1. **Make it build from a clean clone.** Change `.gitignore` to `/data/`; commit or delete `pub mod data;`; un-ignore `.sqlx/`, run `cargo sqlx prepare`, commit the cache, set `SQLX_OFFLINE=true` in CI/Dockerfile/verify script; commit `Cargo.lock`; delete or compile the orphaned directories; fix the benches and the integration tests. Make CI actually red when it should be.
2. **Collapse the domain model.** One `Bar`, `Quote`, `Order`, `OrderStatus`, `Fill`, `Position`, `PatternStats` in a single `domain` module that everything imports. Delete the converters. This alone removes several thousand lines and most of the lossy mappings.
3. **Introduce `Clock`, `MarketDataSource`, and a real `SimBroker`.** Thread the clock through risk, PDT, orders. Make the WebSocket and a CSV replayer both publish `NewCandle`. Make `SimBroker` fill with a configurable model and publish `OrderFilled`.
4. **Build the real loop once, with a synchronous risk gate.** `signal → RiskEngine::check(order) -> Result → broker.submit(client_order_id) → fill stream → positions → P&L → risk state`, persisted in one transaction per fill, rehydrated on start, reconciled against the broker on connect. Replace `broadcast` with bounded `mpsc` for the order/fill path; keep `broadcast` only for UI fan-out.
5. **Fix the risk engine's actual logic:** deadlock, zero-price panic, market-order notional, PDT day-trade detection and 5-day window, daily-loss halt wiring, commission sign, emergency threshold, inverted multiplier, UTC market hours (add `chrono-tz` and an exchange calendar).
6. **Harden the broker layer:** broker factory with `Mock | Paper | Live` and a consistency check between flag and URL; reconnect-on-any-exit with backoff and resubscribe; auth await; pong timeout; rate limiter; full status enum; correct 403/422 mapping; `client_order_id`; redacted `Debug` for secrets; `trade_updates` stream.
7. **Store money as TEXT `Decimal` or integer minor units**, quantities as `Decimal`, one schema source (sqlx migrations) run at startup.
8. **Then** write the backtest runner and commit fixture CSVs, and make the "Phase complete" claims in `plans/` and `reports/` match what runs.

---

## Addendum: scratch unit-test run

On a scratch copy with a placeholder `src/data/mod.rs`, `schema.sql` applied to a SQLite file, the `schema_migrations` table added by hand, and `DATABASE_URL` set:

- `cargo test --lib`: **213 passed, 0 failed** (BACKLOG claims 235/235; the count differs but the suite is green).
- `cargo check --all-targets`: benches and three integration test files still fail to compile (section 1.3).

So the unit tests themselves are healthy once the build is unblocked. The gap is not test quality in isolation; it is that the tested units are never assembled into a running system, and the build/CI setup cannot prove even this much from git.
