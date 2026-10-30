# DayTrader Agent Backlog

Last Updated: 2026-10-30 (Recovery Program adopted — see `plans/recovery/README.md` and `reports/honest-assessment-2026-10-30.md`. Feature freeze until Phase 0 exits. Task specs live in `plans/recovery/phase-*.md`; the rows below are the index.)

## Rules
1. Every agent session MUST claim a task before starting work (or create a new one)
2. Only ONE task may be "In Progress" per agent session
3. Status transitions: Pending → In Progress → Done (or Blocked)
4. "Done" requires `./scripts/verify_agent_work.sh` to have passed
5. Blocked tasks must include a reason
6. Update this file in every commit that changes source code

---

## Active Tasks

| ID | Task | Status | Started | Branch | Notes |
|----|------|--------|---------|--------|-------|
| B-085 | Fix hardcoded "DEFAULT" signal symbol in the live signal path; derive from candle window, reject mixed-symbol windows, drop backtester stamp workaround | Done | 2026-10-30 | fix/signal-symbol-live-path | SignalEngine now tags signals with the candle window's symbol and returns SignalError::MixedSymbols for heterogeneous windows (new SignalError enum wraps PatternError); cache no longer dedups across symbols; backtest post-hoc symbol stamp removed |

---

## Program: Recovery and Research (supersedes the priority tiers below)

Specs: `plans/recovery/` (`README.md` decides order and IDs; the research roadmap is merged into it and remains the design reference). Goal: a trustworthy verdict on whether the approach makes money (Phase 4 Go/No-Go), live capital only on a later decision (Phase L). State as of 2026-10-30: B-086 is done. `main` is the only long-lived branch and is protected (PR required, administrators included, force-push and deletion blocked, no required status checks until B-089 makes CI green). `staging` and the stale branches are deleted; the unmerged ones are archived in a local bundle held by the owner. #29, #30, #31, #32, #34 and #35 are merged; #6 is closed as superseded. Open: #36 (UI quote midpoint fix). Next: B-088 (hermetic sqlx build), then B-089 and B-090 to B-094.

### Phase 0: Truth and Build (`plans/recovery/phase-0-truth-and-build.md`)

| ID | Task | Who | Size | Status |
|----|------|-----|------|--------|
| B-085 | Fix hardcoded `"DEFAULT"` signal symbol in the live signal path (derive from candle window, reject mixed-symbol windows, drop backtester stamp) — PR into `feat/close-trading-loop` | agent | S | Done 2026-10-30 — PR #32 merged into `staging` |
| B-086 | Consolidate to one trunk: fast-forward `staging` into `main`, protect `main`, delete `staging` and stale branches, close #6 | human | S | Done 2026-10-30 (`main` at `936798b`; 3 branches kept on purpose: `fix/review-mcp-security-hardening` pending the MCP decision, and `fix/ui-quote-last-midpoint` plus `feat/b036-b039-frontend-consolidation` until #36 merges) |
| B-087 | Update governance (CLAUDE.md, rules, skills, workflows, docs) for one trunk; `/promote-to-staging` → `/promote` | agent | M | Done 2026-10-30 — PR #34 merged into `staging` |
| B-088 | Hermetic sqlx build: commit `.sqlx/` + `Cargo.lock`, `SQLX_OFFLINE=true` everywhere, `setup_db.sh` without `sqlite3` CLI, `create_if_missing`, pragmas via `SqliteConnectOptions` | agent | M | Done 2026-10-30 (`chore/b088-hermetic-sqlx`) |
| B-089 | CI mirrors verify script, runs on every PR and trunk push, includes Tauri + frontend, no excused failures; delete `known_test_issues.txt` escape hatch | agent | M | Pending |
| B-090 | Clippy `-D warnings` clean: `utils`, `brokers`, `config`, `data` | agent | M | Pending |
| B-091 | Clippy `-D warnings` clean: `events`, `db` | agent | M | Pending |
| B-092 | Clippy `-D warnings` clean: `patterns` | agent | M | Pending |
| B-093 | Clippy `-D warnings` clean: `risk` (split by file) | agent | M | Pending |
| B-094 | Clippy `-D warnings` clean: `orders`, `positions`, `execution`, Tauri, tests, benches; deny `unwrap_used`/`expect_used` in non-test code; flip CI + local to `-D warnings` | agent | M | Pending |
| B-095 | Delete dead code (`retry.rs`, `SignalStreamingPipeline`, `MigrationManager` + `migrations/`, `OrderRepository`, …) and 15 unused dependencies | agent | M | Pending |
| B-096 | Fix or delete benches so `cargo bench --no-run` compiles | agent | S | Done 2026-10-28 via B-221 (benches repaired on the sweep branch, landed by the r0 integration PR) |
| B-097 | `config/default.toml` loads completely (`risk.*` nesting, `deny_unknown_fields`, real assertions in `config_validation`) | agent | S | Partly done via B-205 (validated `RawRiskConfig` → `RiskConfig`, all-zero limits rejected); still open: `risk.*` nesting in `default.toml`, `deny_unknown_fields`, profile/multiplier count assertions |
| B-098 | Documents tell the truth: `STATUS.md`, archive `plans/` + phase reports to `docs/history/`, fix README/onboarding, delete `IMPORTANT-CARGO-FIX.md`/`cargo.sh` | agent | M | Pending |
| B-081 | (existing) 27 `#[ignore]`d tests pass or are deleted with reasons | agent | M | Pending |

### Phase 1: Canonical model and workspace (`plans/recovery/phase-1-core-and-workspace.md`)

| ID | Task | Who | Size | Status |
|----|------|-----|------|--------|
| B-101 | Design and land `crates/dt-core` (one definition per domain type, `Clock`, `TradingCalendar`, typed errors), merged unused | **[design]** | L | Pending (blocked on Phase 0 exit) |
| B-102–B-108 | Migrate `brokers`, `events`, `patterns`, `positions`, `orders`, `risk`, `execution`+`db`+Tauri onto `dt-core`, one PR each, deleting local types and converters | agent | M each | Pending |
| B-109 | Thread `Clock` everywhere; CI grep forbids `Utc::now()` outside `SystemClock` | agent | M–L | Pending |
| B-110–B-117 | Split into workspace crates bottom-up (`dt-broker`, `dt-alpaca`, `dt-strategy`, `dt-risk`, `dt-store`, `dt-engine`, `dt-backtest`, `dtctl`); delete `verify_architecture.sh` and per-module `CLAUDE.md` | agent | M each | Pending |

### Phase 2: Risk correctness (`plans/recovery/phase-2-risk-correctness.md`)

| ID | Task | Who | Size | Status |
|----|------|-----|------|--------|
| B-121 | Risk test harness (started engine + injected clock + timeout-based expectations) | agent | S | Pending |
| B-122–B-136 | One defect each: deadlock, `Lagged`, market-order notional, PDT off-by-one/window/UTC date/persistence, daily-loss wiring + commission sign + single halt, emergency threshold semantics, daily reset, inverted multiplier, rule engine sharing real trackers, un-awaited call + eager index, guards across `.await`, `PatternDetected` re-publish, ghost orders + partial-then-cancel | agent | S–M each | Pending |
| B-137 | Scripted-day scenario suite demonstrably sensitive to each fix | agent | M | Pending |

### Phases 3, 4, 5 and L (`plans/recovery/phase-3-to-L-outline.md`)

Outlined only. A **[design]** task writes the Phase 3 and 4 specs when Phase 2 exits.

- **Phase 3, one correct pipeline in simulation:** synchronous risk gate, fills into one portfolio, exits, persistence and rehydration against `SimBroker`. IDs assigned when specified.
- **Phase 4, research platform:** B-209 (data quality, first), B-223 (adjustment correctness), B-210, B-211, B-216 (regimes, needed by B-212's attribution), B-212, B-213, then **B-222 Go/No-Go** (human). B-224 (bench engine shutdown) lands with B-213.
- **Phase 5, after the verdict, order set by B-222:** B-215, B-217, B-218, B-219, B-220.
- **Phase L, only on a decision to trade real money:** live connectivity plus roadmap Phase L hardening, then 30 trading days of paper.

Absorbed: B-066 (SQLite PDT repository) into B-127; B-067 (Quote unification) into B-102/B-103; B-070 (architecture reconciliation) into B-110 to B-117; B-074 into B-090 to B-094; B-075 into B-088/B-089; B-061 (emergency liquidation) into B-129 and Phase L.

---

## Pending Tasks (pre-recovery tiers, kept for reference)

### Priority: Medium (Quality & Completeness)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-066 | Implement concrete SQLite `PdtRepositoryTrait` and wire into AppState/PdtEnforcer (PDT enforcement currently in-memory only; day-trade history and audit commands return empty fallbacks) | Medium | 2026-07-06 tauri compile fix | `src/db/`, `daytrader/src-tauri/src/state.rs` |
| B-067 | Unify Quote types (`brokers::traits::Quote` has symbol/last/u32 sizes vs `events::types::Quote` with Quantity sizes) or document the boundary mapping as intentional | Medium | 2026-07-06 Candle-unification analysis | `src/brokers/traits.rs`, `src/events/types.rs` |
| B-068 | Run doctests in the verification pipeline (`cargo test --doc`); known pre-existing doctest failures in risk/limits.rs flagged in PR #20 | Medium | 2026-07-06 tauri compile fix | `scripts/verify_agent_work.sh` |
| B-070 | Architecture reconciliation: hidden boundary violations found when checker was tightened to catch inline qualified paths — patterns<->risk is circular (risk->patterns already eliminated by B-058), db -> risk/orders/positions inverts layering, events -> patterns. Baselined (ratchet) in `.governance/architecture_violations_baseline.txt`; burn down edge by edge. (domain -> db edges resolved 2026-07-07 by deleting the never-compiled `src/domain/`/`src/infrastructure/` modules in B-064) | Medium | 2026-07-06 checker fix | `src/patterns/signals.rs`, `src/db/`, `src/events/types.rs` |
| B-074 | **Superseded by recovery B-090 to B-094.** Warning count re-measured 2026-07-28: `cargo check --lib` alone emits **693** warnings, not ~130. Clean up accumulated clippy/rustc warnings so `cargo clippy -- -D warnings` passes (documented invariant; GitHub CI enforces it and failed on PR #24; local pipeline is warn-only with a TODO to tighten — tighten it once clean) | Medium | 2026-07-07 PR #24 CI failure | `src/` (risk, positions, orders, brokers, events, db, utils) |
| B-075 | Fix GitHub CI for sqlx macros: docs + test jobs fail with "set DATABASE_URL" because CI provisions no database (commit `.sqlx/` offline cache via `cargo sqlx prepare` + set `SQLX_OFFLINE=true`, or provision sqlite in workflows); also CI triggers only on PRs to main/develop, so staging PRs get zero CI — NEEDS OWNER APPROVAL (CI/CD change) | Medium | 2026-07-07 PR #24 CI failure | `.github/workflows/ci.yml`, `.github/workflows/docs.yml` |
| B-081 | Ignored-test burn-down REMAINDER: (a) 4 tests re-ignored with accurate reasons — PdtLimit rule counting semantics (`test_pdt_edge_cases`, `test_pdt_e2e_workflow`: violations fire for every predicted day trade under equity threshold, not only past the 3-trade allowance), `PatternPerformanceUpdate` not published on detection without prior stats (`test_pattern_specific_events`), daily-reset test needs public-API rewrite (`test_daily_reset_timing`); (b) 5 PostgreSQL-era pipeline tests in pattern_streaming_pipeline_integration.rs to port to in-memory SQLite (incl. `test_end_to_end_candle_to_signal_pipeline`); (c) unit-level `.governance/known_test_issues.txt` baseline audit; (d) fold in B-068 doctests | Medium | 2026-07-08 session review | `tests/`, `src/risk/`, `.governance/known_test_issues.txt` |

### Priority: Research Platform (R0–R5)

Designs: `plans/2026-07-28-research-platform-roadmap.md`. **Order is set by the merged program in `plans/recovery/README.md`**; the Phase column below shows the roadmap section and, after the arrow, the program phase it now belongs to. **Automation gate stays OFF until Phase L.**

> **IDs renumbered 2026-10-30.** This track was planned on `feat/r0-parallel-sweep` with IDs B-085 to B-103, which collide with the Recovery Program. Old to new: B-085 → B-201, B-086 → B-202, B-086b → B-203, B-088 → B-205, B-089a → B-207, B-103 → B-221, B-087 → B-204, B-089 → B-206, B-090 → B-208, B-091 → B-209, B-092 → B-210, B-093 → B-211, B-094 → B-212, B-095 → B-213, B-096 → B-214, B-097 → B-215, B-098 → B-216, B-099 → B-217, B-100 → B-218, B-101 → B-219, B-102 → B-220. Commit messages on that branch keep the old IDs. The two plans are to be merged into one (see `plans/recovery/README.md`).

| ID | Task | Priority | Phase | Scope |
|----|------|----------|-------|-------|
| B-204 | Money → `Money<6>` fixed-point, SQLite `INTEGER` micros across ~24 tables (**BREAKING**, dev DB disposable) **Decided 2026-10-30 (owner): integer micros.** Supersedes the recovery plan's decimal-text storage; `Money` in `dt-core` (B-101) uses this representation and B-108 migrates the columns. | Critical | R0.2 → P1 (B-101, B-108) | `src/domain/money.rs`, `src/db/models.rs`, `migrations/003_*.sql` |
| B-206 | Honest CI gate: clippy `-D warnings`, `known_test_issues.txt` → empty, `std::sync` → 0, add `cargo sqlx prepare --check`, `cargo-deny`, `cargo-machete` (supersedes/folds B-074, B-081) Folded into recovery B-089 to B-094. | High | R0.5 → P0 (B-089 to B-094) | `scripts/`, `.governance/` |
| B-208 | Dependency cleanup: delete `ta`/`once_cell`/`tower`/`crossbeam`/`rusqlite` (0–2 refs each); upgrade reqwest 0.11→0.13, tungstenite 0.21→0.30, thiserror 1→2, sqlx 0.8→0.9 Dependency deletions folded into recovery B-095; the version upgrades remain here. | Medium | R0.6 → P0 deletions (B-095); upgrades after P0 | `Cargo.toml` |
| B-209 | Data-quality gates + `data_quality_events` quarantine table; free-tier divergence audit → `docs/data-validity.md` | High | R0.7 → P4, first | `src/data/validation.rs` |
| B-210 | Backtester realism: `FillModel` trait (spread/impact/participation), short side, PDT accrual during replay, EOD liquidation | Critical | R1 → P4 | `src/execution/backtest.rs` |
| B-211 | Walk-forward + purged K-fold with embargo; deflated Sharpe fed by sweep trial count | Critical | R1 → P4 | `src/execution/` |
| B-212 | Metric suite + Monte Carlo bootstrap + per-pattern × regime × hour attribution | High | R1 → P4 | `src/execution/`, `src/utils/stats.rs` |
| B-213 | Backtest CLI (`src/bin/backtest.rs`) + determinism and lookahead-detector gates | High | R1 → P4 | `src/bin/` |
| B-214 | `src/domain/` leaf module; drain architecture ratchet to empty (supersedes B-067, B-070) Overlaps recovery B-101 (`dt-core`) and B-110 to B-117; reconcile in the plan merge. | High | R2 → P1 (`dt-core`) | `src/domain/`, `src/events/`, `src/db/` |
| B-215 | Sealed `SignalSource` trait; adapt all 9 detectors; feature store with PIT `CHECK` constraints + restatement log | High | R2 → P5 | `src/signals/` |
| B-216 | Regime classification (threshold-based) + regime-bucketed pattern stats | High | R2 → P4, before B-212 | `src/patterns/`, `src/risk/pattern_stats.rs` |
| B-217 | Universe scanner: PIT snapshots, free-tier factors (RVOL time-of-day normalized, gap, ATR%, momentum, pattern prior), ranking with rationale | Medium | R3 → P5 | `src/universe/` |
| B-218 | `tauri-specta` bindings + `dto.rs` boundary; money crosses as branded string (supersedes hand-written TS types) | Medium | R4 → P5 | `daytrader/src-tauri/src/dto.rs`, `daytrader/src/types/` |
| B-219 | Research UI: equity/underwater curves, trade blotter, attribution heatmap, sweep surface, validity banner | Medium | R4 → P5 | `daytrader/src/` |
| B-220 | ML signal layer: ONNX via `ort` behind `--features ml`, model registry, shadow-only evaluation, drift/decay/crowding metrics | Low | R5 → P5 | `src/signals/ml/` |
| B-222 | ⭑ Go/No-Go checkpoint: run the detectors through the Phase 4 harness and decide (stop, scanner-first, or signals-first); record in `docs/sessions/` | Critical | → end of P4 (human) | `docs/sessions/` |
| B-223 | Corporate-action adjustment correctness, before anything consumes it (from the #39 review): sort dividend reference prices by full timestamp, not date; keep distinct dividends on the same ex-date and apply amount corrections (action identity, not `(symbol, ex_date)`); compute each dividend's reference close and factor once per action instead of per candle (quadratic today); split-adjust volume by the inverse factor; bound actions by an explicit as-of date so future actions never alter a historical backtest (the roadmap's `AdjustedBars::as_of`). `adjust_candles` has no runtime caller yet, so none of this is live. | Critical | R0.1 → P4, before B-210 | `src/data/corporate_actions.rs` |
| B-224 | Benches: risk benchmarks call `RiskEngine::start()` per iteration and never stop it, so detached receiver tasks accumulate and distort timings (from the #39 review). Give the engine a shutdown handle and use it in the benches before `verify_perf.sh` gates on them. | Medium | R0.5 → P4 (with B-213's throughput gate) | `benches/risk_benchmarks.rs`, `src/risk/engine.rs` |

#### Research track: completed on the sweep branch (landed on `main` by the r0 integration PR)

| ID | Was | Task | Completed | Branch | Notes |
|----|-----|------|-----------|--------|-------|
| B-201 | B-085 | R0.3 — remove `to_string().parse()` decimal round-trip from the pattern-detection hot path | 2026-07-28 | `refactor/r0-measurement-integrity` | 16 sites across `patterns/{traits,utils,multi_candle}.rs`, `brokers/types.rs`, `risk/types.rs`; all collapsed onto the canonical `patterns::utils::decimal_to_ratio` |
| B-202 | B-086 | R0.1 step 1 — typed `BarAdjustment` + candle provenance | 2026-07-28 | `refactor/r0-measurement-integrity` | `adjustment=raw` was hardcoded; now a `#[non_exhaustive]` enum defaulting to `All`, threaded through `get_bars`/`MarketDataProvider`/storage. Schema v3 adds an `adjustment` discriminator to the candles uniqueness key so price bases cannot collide. Step 2 (raw + factor table, query-time adjustment) still open |
| B-203 | B-086b | R0.1 step 2 — corporate-action factors + query-time adjustment | 2026-10-28 | `feat/r0-parallel-sweep` | Migration v4, `src/data/corporate_actions.rs`, query-time factor adjustment, 4:1-split-produces-no-signal regression test |
| B-205 | B-088 | R0.4 — config type spine (`RawRiskConfig` → `TryFrom` → `RiskConfig`) | 2026-10-28 | `feat/r0-parallel-sweep` | `derive(Default)` removed; `RiskConfig` deserializes via `#[serde(try_from)]` so every load path validates (scalars, pattern profiles, confidence bands, performance thresholds) with typed `RiskConfigError`. Tauri fallback uses embedded `config/default.toml`. Follow-ups: private fields/newtypes; gate `test_fixture()` behind cfg; fail hard on invalid user config instead of falling back |
| B-207 | B-089a | R0.5 — gate matched failure *counts*, not test *names* | 2026-07-28 | `refactor/r0-measurement-integrity` | Any N new failures passed while N stayed under the known-issues ceiling; caught a real `data::ingestion` regression only by manual inspection. Now fails on any failing test absent from the baseline |
| B-221 | B-103 | R0.5 — repair the `benches/` suite + `scripts/verify_perf.sh` | 2026-10-28 | `feat/r0-parallel-sweep` | All 5 criterion suites compile against the current API (no benchmarks deleted). Found Cargo.toml lacked `[[bench]] harness = false`, so benches had run zero criterion benchmarks; added. New `scripts/verify_perf.sh` (compile gate; `--quick` runs each bench once). Not yet wired into verify/CI (B-089) |

### Priority: Low (Enhancements)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-048 | Mock data generator improvements for tests | Low | progress-tracking.md | `tests/` |
| B-049 | Load testing implementation | Low | progress-tracking.md | `tests/` |
| B-050 | Release automation pipeline | Low | progress-tracking.md | `.github/workflows/` |
| B-051 | Production monitoring setup | Low | progress-tracking.md | infrastructure |
| B-052 | User acceptance testing (PDT compliance scenarios) | Low | progress-tracking.md | `tests/`, `src/risk/` |
| B-053 | Interactive Brokers integration | Low | progress-tracking.md | `src/brokers/` |
| B-054 | Phase 6: Backtesting & validation framework | Low | plans/progress-tracking.md | `tests/`, `src/patterns/` |
| B-065 | Frontend test coverage (currently zero test files in daytrader/src) | Low | 2026-07-06 audit | `daytrader/src/` |

### Blocked / Needs Human Decision

| ID | Task | Status | Reason |
|----|------|--------|--------|
| B-061 | Implement emergency liquidation logic (risk/engine.rs:589 TODO; `emergency_liquidate_threshold` config exists with no execution path) | Blocked | Auto-liquidation semantics in a real account need human sign-off before implementation |
| B-057 | Decimal API + private-field drift in integration tests | Abandoned | Branch fix/b057-decimal-privacy is self-declared UNVERIFIED WIP ("do not merge as-is"); its BACKLOG "Done" claim was incorrect. Integration tests pass on staging without it (B-056 superseded). Re-derive only if a concrete need reappears. |
| — | MCP server suite (fix/review-mcp-security-hardening, 6 servers, +14.7k lines) | Deferred | Not in backlog scope; failed 5 verification checks 2026-05-26; large new attack surface needs dedicated security review track |

---

## Completed Tasks

| ID | Task | Completed | Branch | Verification | Summary |
|----|------|-----------|--------|--------------|---------|
| B-001 | Phase 1: Core infrastructure | 2025 | main | Pre-governance | Broker abstraction, event bus, data ingestion |
| B-002 | Phase 2: Risk management | 2025 | main | Pre-governance | PDT compliance, risk engine, position sizing |
| B-003 | Phase 3A: Pattern detection (10 patterns) | 2025 | main | Pre-governance | Candlestick patterns, technical indicators |
| B-004 | Phase 3D: Integration & optimization | 2025 | main | Pre-governance | Database, streaming, performance tuning |
| B-005 | Phase 4A: Order management | 2026-01-25 | main | Pre-governance | 2,649 lines, 5 modules |
| B-006 | Phase 4B-C: Position tracking & execution | 2026-01-25 | main | Pre-governance | Position tracker, execution engine |
| B-007 | Agent governance framework | 2026-03-07 | claude/strange-dubinsky | verify_agent_work.sh | CLAUDE.md, verification scripts, backlog, hooks |
| B-008 → B-024 | (Renumbered into B-026+ with refined scopes) | — | — | — | Original B-008..B-024 absorbed into refined tasks above |
| B-026 | Fix CSS: Dashboard layout + component styles | 2026-03-28 | gt__toast__2b1b6675 | manual | App.css rewritten with full dashboard, dark mode, all component classes |
| B-027 | Fix MarketData: add `last` price + error handling | 2026-03-28 | gt__toast__2b1b6675 | manual | QuoteResponse.last added (Rust+TS), component shows bid/last/ask with error state |
| B-028 | Fix systemStore: initialize on startup | 2026-03-28 | gt__toast__2b1b6675 | manual | Immediate refresh + error field added to SystemStore interface |
| B-031 | Add ConnectionStatus indicator component | 2026-03-28 | gt__toast__2b1b6675 | manual | New ConnectionStatus.tsx listens for push events + polls systemStore |
| B-035 | Expose actual risk config from RiskEngine | 2026-03-28 | gt__toast__2b1b6675 | manual | RiskEngine::get_config() added; get_risk_limits reads live config |
| B-036 | Fix test compilation errors and async deadlocks | 2026-04-14 | fix/test-compilation | cargo test --lib (235/235) | Fixed 91 compile errors + 3 RwLock deadlocks; added .unwrap() to newtype constructors in tests; fixed include_str path |
| B-037 | Fix 21 test failures from newtype refactor | 2026-04-19 | fix/test-compilation | cargo test --lib (235/235) | PDT off-by-one, zero-limits = unlimited, events/bus metrics order, MockBrokerAdapter unique ids, plus stale test assertions |
| B-055 | Scaffold migrations/ for sqlx integration tests | 2026-04-21 | fix/b055-migrations-scaffold | cargo check --tests (no canonicalization error) | Created migrations/001_initial_schema.sql + 002_trading_signals.sql matching schema.sql; unblocks sqlx::migrate! compile-time macro in 3 integration tests |
| B-056 | Fix Fill struct construction in integration tests for post-newtype refactor | 2026-04-21 | fix/b056-fill-struct-renames | cargo test --test '*' passes (0 failures) | Removed `symbol`/`side` from Fill literals; fixed 9 integration test files: database_integration (tx rollback scope), loss_limit_edge_cases (6 wrong limits/values), market_scenarios/mock_broker_risk/risk_e2e_workflows/risk_engine_integration/risk_integration (13 tests #[ignore] for unimplemented RiskEngine halt propagation + event-subscription timing), pattern_signal_database_integration (API renames, unique IDs), pdt_edge_cases (dynamic dates), performance_validation (threshold + parallel), position_workflows_integration (schema+FK, Decimal approx, PnL arithmetic), risk_database_integration (equity threshold) |
| B-029 | Wire up MockBroker to emit synthetic market data events | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 PASSED | New MockDataDriver in daytrader/src-tauri/src/mock_driver.rs spawns per-symbol tokio tasks that emit QuoteUpdate + NewCandle events at 1s interval; subscribe/unsubscribe commands wired; idempotent subscribe, clean unsubscribe, OHLC-correct random-walk prices |
| B-038 | Add event listener cleanup in marketStore / patternStore on unmount | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 + npm run build | patternStore robustified with array pattern; StoreCleanup wrapper runs dispose on all stores at app unmount |
| B-041 | Add error boundary component for graceful IPC failure handling | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 + npm run build | ErrorBoundary.tsx created; wraps App in main.tsx; shows clean error UI on IPC crashes |
| B-047 | Fix trivial test in src/risk/mod.rs (assert!(true)) | 2026-04-22 | feat/techdebt-unblock-sweep | 9/9 pass | Strengthened test to construct RiskEngine; removed stale baseline entry from known_test_issues.txt |
| B-040 | Add PatternList filter UI (by symbol, signal type, confidence threshold) | 2026-04-22 | feat/techdebt-unblock-sweep | npm run build + verify_agent_work.sh 9/9 | Symbol substring filter, signal type dropdown, confidence threshold slider; filters applied client-side via useMemo |
| B-033 | subscribe_symbol starts mock data streaming | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 (same commit as B-029) | subscribe_symbol delegates to MockDataDriver (idempotent); unsubscribe_symbol command added and registered; confirmed by 2026-07-06 audit |
| B-045 | Database query indexing | pre-2026-07 | (already on staging) | 2026-07-06 audit | ~28 CREATE INDEX statements in schema.sql covering all hot paths + programmatic indexes in migrations.rs; closed as already-done |
| B-046 | Event filtering in events/filters | pre-2026-07 | (already on staging) | 2026-07-06 audit | filters.rs fully implemented (636 lines, composable lazy filters, tests, no TODOs); closed as already-done |
| B-060 | Harden verification pipeline | 2026-07-06 | fix/b060-verify-pipeline-hardening (PR #10) | full pipeline 8/8 | Exit-code-based unit-test gate, portable counting, working-tree-aware skips, DATABASE_URL auto-provisioning, production-safety baseline check, frontend build gate |
| B-030 | Candle chart via lightweight-charts | 2026-07-06 | feat/b030-candle-chart (PR #11) | verify 7/7 + npm build | CandleChart.tsx (v4 API), backfill via get_recent_candles, live updates from store, ResizeObserver |
| B-032 | PDT status panel | 2026-07-06 | feat/b032-b034-risk-ui-events (PR #12) | verify 8/8 + npm build | riskStore.pdtStatus + fetchPdtStatus; RiskStatus PDT panel with green/yellow/red severity |
| B-034 | Risk store auto-refresh via push events | 2026-07-06 | feat/b032-b034-risk-ui-events (PR #12) | verify 8/8 + npm build | EventBridge forwards RiskViolation as risk:violation; riskStore listens + refreshes; 30s polling fallback kept |
| B-042 | PDT holiday enforcement | 2026-07-06 | fix/b042-pdt-holiday-enforcer (PR #14) | verify 8/8 | MarketHolidayCalendar threaded through PdtState window logic; holiday edge-case tests (Christmas/New Year, Thanksgiving, Jul 4 observance); also exported orphaned utils::holidays module |
| B-044 | Alpaca rate limiting | 2026-07-06 | feat/b044-alpaca-rate-limiting (PR #13) | verify 8/8 | Token bucket (200 req/min default), real Retry-After parsing, exponential WS reconnect backoff, README corrected |
| B-069 | Fix Tauri app binary compilation | 2026-07-06 | fix/tauri-app-compile (PR #15) | verify 10/10 | 23 errors invisible to pipeline (bare cargo check skips workspace members): unified duplicate Candle type, wired orphaned holidays/position_tracker modules + implemented their missing types, PdtRepositoryTrait object in state, pipeline now checks --workspace |
| B-036 | TypeScript types module | 2026-07-07 | feat/b036-b039-frontend-consolidation (PR #18) | verify 9/9 + npm build | daytrader/src/types/ single source for all IPC shapes; also fixed Decimal-as-string wire format in raw event payloads |
| B-037 | React custom hooks | 2026-07-07 | feat/b036-b039-frontend-consolidation (PR #18) | verify 9/9 + npm build | useTauriEvents (auto-unlisten), useMarketData (subscribe/unsubscribe lifecycle), usePatterns (filters) |
| B-039 | Multi-symbol watchlist | 2026-07-07 | feat/b036-b039-frontend-consolidation (PR #18) | verify 9/9 + npm build | Watchlist.tsx: validated add input, removable chips, localStorage persistence, per-symbol MarketData cards |
| B-062 | Real risk-violations DB query | 2026-07-07 | fix/b062-b063-violations-query-std-sync (PR #19) | verify full | PerformanceRepository::get_recent_risk_violations; DB errors now surface to frontend instead of reading as "no violations"; mock mode distinguished |
| B-063 | std::sync concurrency-rule fixes | 2026-07-07 | fix/b062-b063-violations-query-std-sync (PR #19) | verify full | db/mock.rs to tokio::sync (10 poisoned-lock unwraps gone); holidays.rs kept std::sync with documented sync-context exception |
| B-043 | DST-correct market session boundaries | 2026-07-07 | fix/b043-et-session-boundaries (PR #20) + fix/b043-review-followups (PR #26) | verify full | utils/market_hours.rs via chrono-tz America/New_York; fixed limits.rs, pdt.rs, execution/rules.rs call sites; follow-up: ET trading_date consistency, sticky invalid-config halt, direction-aware ambiguity |
| B-059 | Production unwrap/expect removal | 2026-07-07 | PRs #13/#20/#22 (split by file owner) | verify full + ratchet | alpaca/rest.rs expect (PR #13), limits.rs try_from_config with fail-safe halt (PR #20), engine.rs placeholder fallbacks eliminated (PR #22); baseline entries removed |
| B-058 | Real day-trade detection in RiskEngine | 2026-07-07 | fix/b058-risk-engine-day-trades (PR #22) | verify full (312 unit tests) | predicts_day_trade pre-trade lookahead + PositionTracker::process_fill ground truth feeding PdtEnforcer (single counter); specific RiskViolationType variants; risk->patterns architecture edge eliminated (baseline 3 to 0); pending-order lifecycle incl. cancel/reject cleanup |
| B-072 | Market-hours check in order engine | 2026-07-07 | claude/serene-pasteur-af8a90 (PR #25) | verify_agent_work.sh full pass | utils/market_hours::is_market_open (B-043 session check + holidays::is_business_day) + utils/clock (Clock trait, SystemClock/FixedClock); place_order step 2 rejects with MarketClosed on closed market/weekend/holiday or check error (fail closed); engine tests pinned to fixed in-session clock |
| B-071 | Fix mock feature compile errors in MockPdtRepository::get_day_trade_summary | 2026-07-07 | claude/epic-grothendieck-7746f9 (PR #24) | cargo test --lib --features mock (full suite green) | E0716: temporary `&vec![]` dropped while borrowed (replaced with longer-lived empty Vec binding); E0277: unique_symbols collected whole DayTradeRecord structs into HashSet without Eq/Hash (now maps to &t.symbol); 2 regression tests merged into B-063's test module; undetected because verify pipeline never built --features mock — pipeline now runs it as step 5 |
| B-064 | Dead code cleanup + launch docs | 2026-07-07 | chore/phase-a-launch-docs-dead-code | pending verify | Removed template lib.rs/greet + [lib] target, patternStore.loadStatistics/statistics, get_detected_patterns_for_symbol command + unused PatternSignalResponse DTO; deleted never-compiled src/domain/ + src/infrastructure/ (orphaned hexagonal experiment, zero references); rewrote root+daytrader READMEs with real launch paths, added .env.example, Tauri section + fixed DATABASE_URL in developer-onboarding.md, app title to DayTrader |
| B-076 | Alpaca historical bars fetch (Market Data API v2) | 2026-07-08 | feat/close-trading-loop | verify full + 8 new unit tests | Replaced hardcoded `get_candles` stub: RestClient::get_bars against data.alpaca.markets (new AlpacaConfig::data_base_url with serde default), timeframe mapping, next_page_token pagination, boundary validation (positive prices, high>=low) via AlpacaHistoricalBar::into_candle reusing the WS bar converter; AlpacaProvider::get_candles delegates. Unblocks backtesting (B-054) and indicator warm-start. Live-credential test is #[ignore]d |
| B-077 | Wire trading loop into Tauri app + fix startup panic | 2026-07-08 | feat/close-trading-loop | verify full + 2 new integration tests | App panicked at startup (tokio::spawn with no runtime, main.rs:157) and config parse silently failed (BrokerConfig required api_key/api_secret; now #[serde(default)]) — the demo dashboard never actually launched. Rewired main: #[tokio::main] + tauri::async_runtime::set, Database opened + shared, RiskEngine::with_database, PatternStreamingPipeline replaces hand-rolled detection task (SignalGenerated + persistence), OrderEngine + ExecutionEngine (automation OFF, per-pattern flags on) instantiated for the first time in a runnable binary, 1s order-status poll task, fill listener feeding positions::PositionTracker (P&L) + quote marks. Library additions: RiskEngine::position_tracker() accessor, MockBroker auto_fill_orders opt-in, OrderEngine::sync_order_statuses. 5 new Tauri commands (execution status/toggle, active/recent orders, open positions). tests/trading_loop_integration.rs: order->fill->both-trackers with pattern attribution, signal consumption + kill switch |
| B-078 | Frontend trading views (automation toggle, orders, positions) | 2026-07-08 | feat/close-trading-loop (worktree-agent-ace19e1a5f794696e) | npx tsc --noEmit + npm run build pass; pending verify_agent_work.sh | daytrader/src/types/execution.ts mirrors the 5 B-077 execution.rs DTOs; new executionStore.ts (Zustand, 5s poll, refreshAll, setAutomation); TradingControls.tsx (automation kill-switch, disabled + explanatory note when engines_available is false, execution metrics); OrdersPanel.tsx (Active/Recent tables with empty-state copy); PositionsPanel.tsx (open positions table, green/red P&L); wired into App.tsx below the risk/system grid; StoreCleanup disposes the new store; CSS additions reuse existing App.css variables/card conventions |
| B-079 | Fix candle-ordering bug in pattern detection callers | 2026-07-08 | feat/close-trading-loop | cargo test --lib (344) + backtest regression | Detectors index newest-first (candles[0] = current bar; morning star reads candles[2] as oldest) but PatternStreamingPipeline, the Tauri fallback task, and volume-ratio context all fed CHRONOLOGICAL windows — every live detection evaluated the OLDEST cached candle as "current", so patterns lagged by up to the full window and volume ratios compared against future bars. Callers now pass reversed views; contract documented on PatternRegistry::detect_all. Found because the backtester's first replay produced zero detections on textbook fixtures |
| B-082 | De-flake test_pattern_detection_scaling | 2026-07-08 | feat/close-trading-loop | verify full | Best-of-5 timing batches (minimum as low-noise estimator) instead of a single averaged batch; µs-scale ratios failed nondeterministically under parallel test load and blocked the pipeline twice in one day |
| B-083 | Ignored-test burn-down phase 1 + 4 risk engine bugs | 2026-07-08 | fix/b081-ignored-tests | verify full (unit + integration) | Un-ignored 19 of 28 baselined tests (14 now pass enabled; 4 re-ignored with accurate reasons, 1 legit env-gated). Root causes fixed in ENGINE code, all previously masked by the ignores: (1) RiskEngine::start subscribed inside the spawned task — events published right after start() were silently missed; (2) every event loop died permanently on RecvError::Lagged (risk engine, execution engine, Tauri EventBridge) — under load the app silently stopped processing risk events; (3) DailyLossTracker halts never propagated to engine.is_trading_allowed(), realized OR unrealized — breaching the daily loss limit did not stop trading, and the fill path now publishes TradingHalted; (4) check_emergency_liquidation compared DAILY P&L against the balance threshold — any modest loss triggered emergency halt (also self-deadlocked on the state lock, pre-existing); (5) pattern_limits.rs indexed confidence_multipliers[0] on an empty list — production panic. Test-side: 7 early-exit event-drain loops, orders with no price ($0 value can't violate size limits), PDT test rewritten with real same-day round trips, loss tests had codified the masked behavior |
| B-084 | PR #29 review fixes: fill delta accounting + token encoding | 2026-07-08 | feat/close-trading-loop | verify full + partial-fill regression test | (1) CRITICAL: sync_order_statuses fed the broker's CUMULATIVE filled_quantity into ManagedOrder::update_with_fill's additive update — a partial(5)-then-full(10) sequence booked 15 shares, corrupting position sizing/P&L/risk exposure; now books per-tranche deltas with exact weighted tranche pricing, and OrderTracker publishes the terminal OrderFilled event from the order's cumulative state so consumers see the complete fill exactly once. (2) Alpaca bars page_token was string-concatenated into the URL unencoded (base64-ish tokens contain +/=); bars queries now go through reqwest's percent-encoding query builder |
| B-080 | Backtest replay engine v1 (Phase 6 / B-054 start) | 2026-07-08 | feat/close-trading-loop | verify full + 7 new tests | execution::backtest::BacktestEngine replays candles through the PRODUCTION pipeline (detect -> signal -> strategy -> risk-gated orders -> simulated fills) synchronously per candle for determinism; SteppedClock (utils) drives market-hours/time checks at replay time; ExecutionEngine::with_clock + pub process_signal added; FIFO round-trip accounting with slippage bps + per-fill commission, per-pattern win/loss/P&L attribution feeding record_trade_outcome (live auto-enable loop), equity curve sampling. v1 limits documented: long-only, risk engine async task not started (static pre-trade checks still run), open positions marked to last close. Regression fixture: hammer entry closed by shooting-star exit with positive attributed P&L |

---

## Task Detail: Frontend Critical Path (B-026 through B-035)

These tasks form the critical path for a working frontend. They should be done in order by priority.

---

### B-026: Fix CSS — Dashboard Layout and Component Styles

- **Source**: Codebase audit — `App.css` has logo/hover styles only; dashboard components have no layout
- **Scope**: `daytrader/src/App.css`
- **Acceptance Criteria**:
  1. `.dashboard` grid/flex layout renders 2x2 or responsive columns
  2. `.market-data-card`, `.pattern-list`, `.system-status`, `.risk-status` have card styling
  3. `.status-grid`, `.risk-metrics`, `.limits-grid` have proper spacing/grid
  4. Dark mode handled via `prefers-color-scheme`
  5. Responsive: works at 1200×800 minimum window
- **Pre-work Checklist**:
  - [x] Read `App.tsx` and all component files to understand class names used
  - [x] Confirmed CSS classes expected: `.dashboard`, `.dashboard-section`, `.market-data-card`, `.quote-details`, `.pattern-list`, `.pattern-buy`, `.pattern-sell`, `.system-status`, `.status-grid`, `.status-item`, `.connected`, `.disconnected`, `.risk-status`, `.risk-metrics`, `.metric-group`, `.metric-value`, `.positive`, `.negative`, `.status-indicator`, `.halted`, `.active`, `.risk-limits`, `.limits-grid`, `.limit-item`, `.risk-violations`, `.violations-list`, `.violation-item`, `.risk-actions`, `.error-message`, `.loading`
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` in `daytrader/` passes
  - [ ] Visual inspection confirms layout is usable

---

### B-027: Fix MarketData Component — Add `last` Price + Error Handling

- **Source**: Codebase audit — component references `quote.last` but `QuoteResponse` struct (Rust) and `Quote` interface (TS) don't include `last`; only `bid`/`ask` present
- **Scope**: `daytrader/src/components/MarketData.tsx`, `daytrader/src/store/marketStore.ts`, `daytrader/src-tauri/src/commands/market_data.rs`
- **Acceptance Criteria**:
  1. `QuoteResponse` in `market_data.rs` includes a `last` field (can equal `bid` or cached last trade price)
  2. `Quote` interface in `marketStore.ts` includes `last: number`
  3. `MarketData.tsx` displays bid, ask, and last without TypeScript error
  4. Component handles `getLatestQuote` failure gracefully (shows error message, not crash)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED (Rust side)
  - [ ] `npm run build` in `daytrader/` passes (TS side)

---

### B-028: Fix systemStore — Initialize on Startup

- **Source**: Codebase audit — `systemStore.ts` sets interval but never calls `refreshStatus()` immediately; UI shows "Loading system status..." forever until first interval fires (5s delay)
- **Scope**: `daytrader/src/store/systemStore.ts`, `daytrader/src/components/SystemStatus.tsx`
- **Acceptance Criteria**:
  1. `systemStore` calls `refreshStatus()` once on initialization (before first interval)
  2. `SystemStatus` component shows actual data within 500ms of mount
  3. Handles IPC error gracefully (shows "Unavailable" not crash)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-029: Wire MockBroker to Emit Synthetic Market Data Events

- **Source**: Codebase audit — `main.rs` uses `MockBroker::default()` but `subscribe_symbol` command calls `broker.subscribe_quotes()` which in mock does nothing; no market data events flow to frontend
- **Scope**: `daytrader/src-tauri/src/main.rs`, `src/brokers/mock.rs`
- **Acceptance Criteria**:
  1. After app startup, MockBroker (or a spawned task) emits `MarketEvent::QuoteUpdate` and `MarketEvent::NewCandle` events at a configurable interval (e.g., every 1s for demo)
  2. The EventBridge receives and forwards these to the frontend
  3. MarketData component displays live (mock) quote updates
  4. Pattern detection service receives candles and publishes `MarketEvent::PatternDetected` events
- **Pre-work Checklist**:
  - [ ] Read `src/brokers/mock.rs` to understand MockBroker interface
  - [ ] Read `src/events/types.rs` for event struct shapes
  - [ ] Read `src/brokers/traits.rs` for MarketDataProvider trait
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] App shows live mock data ticking in UI

---

### B-030: Add Candle Chart to MarketData Component

- **Source**: `plans/tauri-thin-slice-plan.md` — `lightweight-charts` is already in `package.json`; no chart component exists yet
- **Scope**: `daytrader/src/components/MarketData.tsx` (or new `CandleChart.tsx`)
- **Acceptance Criteria**:
  1. Chart renders using `lightweight-charts` v4 `CandlestickSeries`
  2. Populated from `getRecentCandles` on mount and updated via `market_data:new_candle` events
  3. Chart resizes responsively within its container
  4. No TypeScript errors
- **Pre-work Checklist**:
  - [ ] Verify `lightweight-charts` is in `node_modules` after `npm install`
  - [ ] Read lightweight-charts v4 API docs for React integration pattern
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes
  - [ ] Chart visible in dev build

---

### B-031: Add ConnectionStatus Indicator Component

- **Source**: `plans/tauri-thin-slice-plan.md` — referenced in architecture but not implemented
- **Scope**: `daytrader/src/components/ConnectionStatus.tsx` (new file), `daytrader/src/App.tsx`
- **Acceptance Criteria**:
  1. Component shows colored dot + text: "Connected" (green) or "Disconnected" (red)
  2. Listens for `system:connection` and `system:disconnection` Tauri events
  3. Falls back to polling `systemStore.status.broker_connected`
  4. Positioned in header/navbar area of App
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-032: Add PDT Status Panel to RiskStatus Component

- **Source**: Codebase audit — `risk.rs` has full `get_pdt_status` command implemented; frontend never calls it
- **Scope**: `daytrader/src/components/RiskStatus.tsx`, `daytrader/src/store/riskStore.ts`
- **Acceptance Criteria**:
  1. `riskStore` fetches `get_pdt_status` and stores `pdtStatus` in state
  2. `RiskStatus` component renders PDT panel: equity vs $25K threshold, day trade count, rolling window dates, warnings
  3. Panel color-codes: green (safe), yellow (near limit), red (restricted)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-033: Fix subscribe_symbol — Start Mock Data Streaming

- **Source**: Codebase audit — `subscribe_symbol` command calls `broker.subscribe_quotes()` on MockBroker which is a no-op; subscribing never triggers data flow
- **Scope**: `daytrader/src-tauri/src/commands/market_data.rs`, `daytrader/src-tauri/src/main.rs`
- **Acceptance Criteria**:
  1. Calling `subscribe_symbol("AAPL")` causes the app to start emitting mock quote/candle events for that symbol
  2. Symbols are tracked; unsubscribing stops event flow for that symbol
  3. Backend doesn't crash if same symbol subscribed multiple times
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED

---

### B-034: Wire Risk Store Auto-Refresh via Tauri Events

- **Source**: Codebase audit — risk store polls every 30s; risk violations and PDT events should push immediately
- **Scope**: `daytrader/src/store/riskStore.ts`, `daytrader/src-tauri/src/events.rs`
- **Acceptance Criteria**:
  1. `EventBridge` emits `risk:violation` Tauri event when a `MarketEvent::RiskViolation` occurs
  2. `riskStore` listens for `risk:violation` and immediately refreshes risk state
  3. Polling interval remains as fallback (30s)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-035: Expose Actual Risk Config from RiskEngine

- **Source**: `commands/risk.rs` — `get_risk_limits` returns hardcoded values with a TODO comment; should read from the actual `RiskEngine` config
- **Scope**: `daytrader/src-tauri/src/commands/risk.rs`, `src/risk/engine.rs`
- **Acceptance Criteria**:
  1. `RiskEngine` exposes a `get_config()` or `get_limits()` method returning the risk configuration
  2. `get_risk_limits` Tauri command reads from the live config instead of hardcoded values
  3. Config values initialized from `default.toml` (or defaults) are reflected in UI
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED

---

## Task Template

When creating a new task, include:

```
### B-XXX: [Task Title]
- **Source**: [plan document, user request, or bug report]
- **Scope**: [which modules/files will be touched]
- **Acceptance Criteria**:
  1. [Specific, testable criterion]
  2. [Another criterion]
- **Pre-work Checklist**:
  - [ ] Read relevant plan document
  - [ ] Identified files to modify
  - [ ] Confirmed no conflicting in-progress work
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] Tests added for new functionality
  - [ ] Documentation updated if needed
  - [ ] plans/progress-tracking.md updated if percentages changed
```
