# DayTrader → Research-Grade Platform

## Context

`daytrader` is a ~40k-LOC Rust workspace (root crate + Tauri 2 desktop app) that already has more working machinery than most projects of its age: 9 real candlestick detectors, Decimal-based indicators, a signal composer, a risk engine with PDT/pattern limits, an order engine, a position tracker with FIFO lots, a genuine Alpaca REST+WebSocket client, and — unusually — an event-driven backtester (`src/execution/backtest.rs`) that replays candles through the *real* production pipeline rather than a parallel simulator.

The stated goal is a modern masterpiece of a day-trading app. The stated **near-term** goal is different and more specific: find out whether this approach produces yield at all, with a possible conversion to live capital later.

Those two goals imply a different plan than "harden for production." A research verdict is only worth what the measurement harness is worth, and this harness currently has three defects that would each independently invalidate the answer:

1. **`src/brokers/alpaca/rest.rs:250` requests `("adjustment", "raw")`.** Historical bars are fetched split- and dividend-unadjusted, and no adjustment pipeline exists anywhere. A 4:1 split manufactures a fake 75% gap that a detector will happily trade. Every backtest crossing a split is currently fiction.
2. **Money is `f64` end-to-end at the persistence boundary.** `src/db/models.rs` and every `REAL` money column round-trip through `Decimal::from_f64_retain` / `.to_f64().unwrap_or(0.0)`. Over a few thousand backtest trades this is a materially wrong equity curve — and equity curve *is* the research output.
3. **The backtester overstates returns and is unreachable.** It is long-only, does not accrue PDT day-trades during replay (so it systematically flatters sub-$25k accounts), does not force EOD liquidation, models costs as flat bps, and is not exposed to any CLI or UI. It is library-and-test-only today.

So the plan front-loads *measurement integrity*, then builds the research platform, then puts an explicit go/no-go checkpoint before the expensive phases. Live-capital hardening is designed for but deferred behind that decision.

**Decisions taken (from planning Q&A):** desktop-only topology, no server tier; free-tier market data for now with the provider abstraction built so paid feeds drop in; big-bang money migration with the 320KB dev DB treated as disposable.

### The honesty constraint

With free-tier IEX data (a minority slice of consolidated volume) and no delisted-ticker history, two biases are **unfixable within this budget** and must be stated on every report rather than engineered around:

- **Survivorship bias** — the universe contains only names that still exist. Backtested returns are optimistic by an unknown amount.
- **Feed unrepresentativeness** — volume-derived factors (relative volume, participation-rate fills, spread) computed from a partial feed are wrong, not merely noisy.

The plan's response is to *quantify* these early (R0.7) rather than assert them, and to render every research report with an explicit validity banner. A research platform that hides its own error bars is worse than no platform.

---

## Guiding principles

1. **Build the machine that can credibly say "no."** The most valuable early output is a trustworthy negative, delivered cheaply. Optimize for reaching that verdict before the XL phases, not after.
2. **Parse, don't validate, at every boundary.** A value that has crossed a boundary is a distinct type from its wire form. No `Raw*` type reaches a decision function.
3. **Automation stays OFF.** The gate defaults off today; it stays off, paper-only, until Phase L lands. This is what makes deferring the live-capital work safe.
4. **The `.governance/` ratchet files are debt registers, not licences.** Every phase must shrink at least one of them.
5. **Breaking changes are cheaper now than ever again.** There is no production data. Take the schema break in R0.

---

## Phase R0 — Trustworthy measurement

Blocks everything. Nothing measured before this is worth believing.

| # | Item | Files | Design | Effort |
|---|---|---|---|---|
| R0.1 | **Corporate actions & split adjustment** | `src/brokers/alpaca/rest.rs:250`, new `src/data/corporate_actions.rs`, migration | Store bars **unadjusted plus an adjustment-factor table**; never adjust in place (in-place backfill silently rewrites history and destroys reproducibility). Adjustment becomes a query-time view: `AdjustedBars::as_of(date)`. Source factors from Alpaca's corporate-actions endpoint; handle splits, reverse splits, cash dividends. | **M — do first** |
| R0.2 | **Money → fixed-point, end-to-end** | `src/domain/money.rs` (new), `src/db/models.rs`, `migrations/003_*.sql`, all money columns | `#[repr(transparent)] struct Money<const SCALE: u32>(i64)`; `type Usd = Money<6>`. Scale 6 covers sub-penny quoting and per-share commissions, range ±$9.2e12. SQLite `INTEGER` minor units with a `*_micros` column-name convention so raw-SQL mistakes are visible in review. `impl sqlx::{Type,Encode,Decode}` on `Money<S>`. Ratios (`win_rate`, `confidence`, `*_percent`, `risk_multiplier`) stay `REAL` — they are not money and forcing them to fixed-point is cargo-culting. | **L — BREAKING** |
| R0.3 | **Kill the hot-path string parse** | `src/patterns/traits.rs:307,320,333`, `src/brokers/types.rs:114` | These compute ratios as `(a / b).to_string().parse().unwrap_or(0.0)` — a heap allocation and decimal-string parse *per candle per detector*. Replace with `CandleGeometry { body, upper, lower, range }` computed once at ingestion from the integer mantissas. Expect 10–100× on detector throughput, which directly buys research iteration speed. | **S — highest ROI in the roadmap** |
| R0.4 | **Config type spine** | `src/config/mod.rs` | `RiskConfig` is 12 raw `f64` **with `derive(Default)`** — so `RiskConfig::default()` yields `max_daily_loss_dollars = 0.0`, `enforce_pdt_rule = false`. A default risk config is a *disabled* risk config. Split into `RawRiskConfig` (serde-only, `pub(crate)`, `#[serde(deny_unknown_fields)]`, **no `Default`**) and `RiskConfig` (newtypes, private fields) joined by `TryFrom` with a `thiserror` `ConfigError`. A typo'd limit becomes a startup error, not a silent zero. | M |
| R0.5 | **Honest CI gate** | `scripts/verify_agent_work.sh`, `.governance/*` | Step 3 currently carries a literal `# TODO: Tighten to -D warnings`. Fix the ~130 warnings, then `clippy --workspace --all-targets -- -D warnings`. Drive `known_test_issues.txt` (20 entries) to empty and triage the 28 `#[ignore]`d tests. Replace the `std::sync` *baseline* with a hard zero (`tokio::sync::RwLock` where the critical section awaits; `OnceLock` for `utils/holidays.rs`, which is initialise-once data, not a lock). Add `cargo sqlx prepare --check` — `.sqlx/` holds 64 offline entries and R0.2 invalidates all of them. Add `cargo-deny` and `cargo-machete`. | L |
| R0.6 | **Dependency cleanup** | `Cargo.toml` | Verified zero references: **delete `ta`** (all indicator math is hand-rolled), **`once_cell`**, **`tower`**, **`crossbeam`**. Near-zero, replace with std or in-repo: `lazy_static`(3)→`LazyLock`, `num_cpus`(1)→`available_parallelism()`, `parking_lot`(1), `smallvec`(1), `statistical`(2)→`src/utils/stats.rs` (needed anyway for R1 metrics). **`rusqlite 0.32` with `bundled`** has 2 refs — a whole SQLite C build for one error variant in `utils/errors.rs`; delete it. Upgrades: `reqwest 0.11→0.13`, `tokio-tungstenite 0.21→0.30`, `thiserror 1→2`, `config 0.13→0.15` *(or delete — with R0.4's `TryFrom` the dynamic layering buys little; keep `toml`)*, `rand 0.8→0.10`, `metrics 0.23→0.24`, `sqlx 0.8→0.9`. `keyring 2.1` is declared and **entirely unused** — keep it pinned but leave dormant until Phase L. | M |
| R0.7 | **Data-quality gates + free-tier audit** | `src/data/validation.rs`, new `data_quality_events` table | Extend existing validation: session-calendar gap detection, OHLC coherence (`low ≤ open,close ≤ high`), volume sanity, price-jump outliers vs ATR, stale-quote detection, timestamp monotonicity. **Quarantine bad bars into a table rather than dropping them silently.** Then run a one-off divergence audit — sample N symbols/days, compare free-tier bars against any accessible reference — and commit the measured number as `docs/data-validity.md`. This number becomes the caveat banner on every research report. | M |

**Definition of done**
- `grep -rn 'from_f64_retain\|to_f64()' src --include='*.rs'` outside tests → 0
- No `REAL` money columns in `migrations/*.sql` (ratio columns explicitly allow-listed)
- `to_string().parse()` count in `src/` → 0
- `known_test_issues.txt` empty; `std::sync::(Mutex|RwLock)` count 0; clippy `-D warnings` clean
- `cargo sqlx prepare --check` passes
- Property tests: `Money<6>` ↔ `Decimal` round-trip; a 1000-trade P&L fixture equals the hand-computed exact value (the f64 version will not — that divergence *is* the regression proof)
- Split test: a known 4:1 split produces no pattern signal on the split date

---

## Phase R1 — Backtester → research platform

The core deliverable. Depends on R0 for correctness and throughput.

**Architecture decision: stay event-driven.** The existing engine replays through the real pipeline with `SteppedClock` + `MockBroker` + in-memory SQLite. That is the correct and rare choice — vectorized backtests are faster but structurally admit lookahead and cannot model PDT, partial fills, or order state machines. Keep it, extend it. Add a vectorized pre-screen only for parameter sweeps where the objective is signal quality (hit rate, information coefficient), never P&L.

| Item | Design | Effort |
|---|---|---|
| **Realistic fills** | Replace flat `slippage_bps` with a sealed `trait FillModel`: `FixedBps`, `SpreadCrossing` (pay half-spread from the actual quote), `SquareRootImpact` (`impact_bps = k·σ·√(qty/ADV)`, Almgren-style), `Participation` (cap fill at X% of bar volume, carry remainder). Default to impact+participation combined. Model partial fills, rejects, commission and SEC/TAF fees, and — critically — **limit orders fill only if price traded *through*, not merely touched.** Touch-fills are the classic backtest lie. | L |
| **Short side** | `OrderSide::Sell` opening a position needs borrow modelling: `ShortAvailability { Easy, HardToBorrow { fee_bps_annual }, Unavailable }`, static table initially. Mandatory stop (unbounded loss), Reg T 150% margin. | L |
| **PDT accrual during replay** | Wire day-trade counting into the replay's in-memory DB. Today's omission means the backtester **systematically overstates returns for accounts under $25k** — the single most misleading gap in the current engine. | M |
| **EOD liquidation** | `enum HoldingPolicy { DayTradeOnly, AllowOvernight { max_positions: NonZeroU32 } }`. `DayTradeOnly` flattens at 15:55 ET with a closing-liquidity slippage penalty. | M |
| **Walk-forward** | `WalkForward { train, test, step, anchored: bool }` → `Vec<Fold>`; report per-fold *and* aggregate. Anchored vs rolling as an enum, not a bool. | M |
| **Purged K-fold + embargo** | Plain K-fold leaks: overlapping label windows put near-duplicates in train and test. Implement López de Prado purging (drop training samples whose label window overlaps test) plus an embargo of `h` bars after each test set. Non-optional for any ML claim later to be credible. | M |
| **Metric suite** | CAGR, max drawdown + duration + recovery, Sharpe (state rf and √252 annualization), Sortino, Calmar, profit factor, expectancy, payoff ratio, exposure, turnover, avg holding period, and **per-pattern × regime × hour-of-day attribution** (`pattern_trade_history` already exists). Benchmark comparison vs SPY buy-hold with alpha/beta/information ratio. Report gross and after-cost separately. | L |
| **Multiple-testing correction** | **Essential in research mode.** The parameter sweep must feed its trial count into a **deflated Sharpe ratio**. Reject any configuration whose OOS/IS performance ratio < 0.5. Report the objective *surface*, not the argmax — a sharp peak is overfitting; add a plateau-robustness score (mean objective over the neighbourhood). | M |
| **Monte Carlo** | Trade-order bootstrap (resample the realized trade sequence → distribution of max-DD and terminal equity; answers "was my drawdown luck?") and parametric path simulation. Report 5/50/95th percentile equity curves and probability of ruin. | M |
| **CLI exposure** | New `src/bin/backtest.rs` with `clap`: `daytrader-backtest run --config x.toml --symbols AAPL,MSFT --from 2024-01-01`. **The engine being unreachable from any interface is its biggest current defect** — an unusable correct engine is worth less than a usable adequate one. | M |
| **Determinism** | Seed every RNG explicitly (`StdRng::seed_from_u64`). Assert byte-identical results across runs and across thread counts. Non-determinism in a backtester is a silent correctness bug. `rayon` parallelizes *across* folds only — never within a fold, where event ordering is the invariant. | S |

**Definition of done**
- Golden-file test: a fixed 1-year synthetic dataset yields byte-identical metrics across 100 runs and 1/4/16 threads
- **Lookahead detector**: shift the input series forward one bar; assert every signal timestamp shifts by exactly one bar and *no metric improves*. An improbable improvement is a lookahead bug.
- PDT test: a $10k account backtest produces strictly fewer trades than the same config at $50k
- Backtest throughput bench ≥1M candle-detections/sec/core, gated at <10% regression
- CLI smoke test in the pipeline

---

## ⭑ Go / No-Go checkpoint

**Run the 9 existing detectors through the honest harness and read the result.** This is the deliverable the user actually asked for, and it arrives here — before the three XL phases.

Report per pattern, per regime: expectancy after realistic costs, deflated Sharpe, OOS/IS ratio, and the R0.7 validity caveats.

- **Nothing survives costs** → stop. The negative is the finding, and it cost R0+R1 instead of the full roadmap. Optionally proceed to R3 (scanner) on the hypothesis that pattern edge is conditional on universe selection — that is a *specific, testable* reason to continue, not a sunk-cost one.
- **Something survives** → proceed, and let what survived direct whether R3 (more candidates) or R5 (better signals) comes first.

---

## Phase R2 — Point-in-time correctness & the signal boundary

Cheap, and it is what makes R3/R5 possible without a rewrite. Partly foldable into R1.

**`src/domain/` — a new leaf module** with zero outbound dependencies, holding canonical `Money`, `Symbol`, `Quote`, `Candle`, `Order`, `Position`. `events` and `brokers` both re-export from it. This simultaneously resolves B-067 (duplicate `Quote`/`Order`/`Position` between `events::types` and `brokers::traits`) and drains the architecture ratchet — `events -> patterns | 1` dies when `MarketEvent` carries a `SignalId` and flat fields instead of a `patterns::TradingSignal`; `db -> positions | 16`, `db -> risk | 7`, `db -> orders | 2` die when the canonical types move down and the `From` impls move up into their owning modules. Target: **baseline file empty.**

Related type work: `Symbol(Arc<str>)` with a `DashMap` interner (fixes the type story and the allocation story at once — clones become atomic increments, map lookups get a pointer-compare fast path); `Quantity(NonZeroU32)` removing a runtime zero-check *and* its `Result`; `Symbol::new` returning a `thiserror` error instead of `&'static str`; `TradingMode { Paper, Live }` instead of `is_paper: bool`.

**`SignalSource` — the abstraction that makes ML models peers of candlestick detectors:**

```rust
// src/signals/source.rs — new leaf module; patterns:: and (later) ml:: both implement it
#[non_exhaustive]
pub struct SignalContext<'a> {
    pub bars: CandleWindow<'a>,
    pub quote: Option<&'a Quote>,
    pub features: &'a FeatureVector,  // point-in-time guaranteed
    pub regime: Regime,
    pub as_of: Instant,               // the PIT anchor; nothing may read past it
}

pub trait SignalSource: sealed::Sealed + Send + Sync {
    fn id(&self) -> SignalSourceId;
    fn version(&self) -> SourceVersion;           // semver + artifact hash
    fn kind(&self) -> SourceKind;                 // Rule | Statistical | Learned
    fn required_features(&self) -> &'static [FeatureId];
    fn evaluate(&self, ctx: &SignalContext<'_>) -> Result<Option<RawSignal>, SignalError>;
}
```

This is the whole future-proofing story. If a model is a *peer* rather than a special case, then per-source rolling stats, auto enable/disable, attribution, and shadow evaluation all work for it **for free**, because they already work for patterns. Each of the 9 detectors gets an adapter; `SignalComposer` becomes a combinator indifferent to whether its input is a hammer detector or a gradient-boosted tree. Also seal `PatternDetector` and collapse `AsyncPatternDetector`/`MonitoredPatternDetector`/`ConfigurablePatternDetector` — three trait hierarchies with zero implementors is speculative generality; monitoring belongs in the registry as a decorator.

**Feature store with enforced point-in-time correctness** — three mechanisms, belt and braces:
1. `CHECK(source_data_watermark <= bar_ts)` — a feature computed from future data *cannot be inserted*.
2. `FeatureStore::get(symbol, feature_id, as_of)` cannot return rows with `bar_ts > as_of`, enforced in the single query, not by callers.
3. A **restatement log** — when a vendor revises a bar, the old value is versioned, not overwritten. Backtests then specify "as-known-at" vs "as-revised."

Features compute **once, incrementally, on bar close, in the same code path live and in backtest.** One implementation is the only reliable defence against train/serve skew.

**Regime classification** — `Regime` over two axes: volatility state (Low/Normal/High/Crisis from realized-vol percentile) × trend state (Trending/MeanReverting/Choppy from variance ratio or Hurst). **Start with threshold-on-percentiles, not an HMM** — HMM regime labels are notoriously unstable and hard to use live. Then bucket `pattern_rolling_stats` by regime: "hammer wins 58%" is nearly useless; "hammer wins 64% in high-vol mean-reverting, 41% in low-vol trending" is actionable. This is a cheap, high-value change and it directly sharpens the Go/No-Go verdict.

**DoD:** all 9 detectors behind `SignalSource` with byte-identical golden-file signals; architecture baseline empty; PIT violation rejected at the DB level; train/serve skew test (same bar, live path and backtest path, identical `FeatureVector`).

---

## Phase R3 — Universe / candidate scanner

The direct answer to "find where trading desks find patterns." Gated on the Go/No-Go outcome.

A fixed hand-managed watchlist is plausibly *why* signal quality is noise-dominated: most patterns only carry information in the right liquidity and volatility regime. A desk does not scan 8 names, it scans thousands and **ranks**. The scanner is a pipeline stage that must be backtestable point-in-time, or it becomes the largest single source of survivorship bias in the system.

New module `src/universe/`:

```
Reference universe    → PIT snapshot, stored per date
 ↓ Tradability        → not halted, not OTC, price ∈ [$2,$500]
 ↓ Liquidity gates    → ADV(20d), median spread, min float
 ↓ Factor computation → ranking inputs
 ↓ Composite ranking  → winsorized cross-sectional z-scores, regime-conditional weights
 ↓ Diversification    → sector cap, correlation cap
 ↓ Candidate bucket   → top K (20–50), refreshed pre-open + intraday
 ↓ SubscriptionManager → diffs against the live WS subscription set
```

`Candidate { symbol, score, factors, rationale: Vec<FactorContribution> }` — **`rationale` is not optional.** An unexplainable ranking is an untrustable one, and it is what the UI shows.

**Factors available on free-tier data** (these carry most of the value): time-of-day-normalized **RVOL** — the single highest-value factor, and meaningless without the time-of-day normalization; overnight gap % and gap-vs-ATR; ATR% and realized vol; distance from 20/50 SMA in ATR units, % of 52-week range; and the **pattern prior** — historical hit rate of *your* patterns on *this* symbol in *this* regime, from `pattern_rolling_stats`. That last one is free, uniquely yours, and the actual differentiator.

**Factors requiring paid data, deliberately deferred:** float and short interest, days-to-cover, borrow fee, options IV rank, news sentiment, odd-lot and trade-size microstructure. Design the factor trait so they slot in without restructuring.

**Caveat that must ship with it:** volume-derived factors on a partial feed are wrong, not just noisy. R0.7's measured divergence number governs how much weight RVOL should carry, and the scanner must be re-validated if a full feed is ever purchased.

**DoD:** deterministic ranking; every day's ranking snapshot stored so the scanner itself is backtestable; A/B backtest of fixed-watchlist vs scanner-driven selection over ≥2 years across every R1 metric. The survivorship test (reconstructing a past universe includes since-delisted names) is written `#[ignore]`d with the reason — it cannot pass without delisted-ticker data.

---

## Phase R4 — Research UI

Depends on the type-safe boundary. Scoped to *research*, not a live trading desk rebuild.

**First, the Rust↔TS boundary — `specta 2` + `tauri-specta 2`, not `ts-rs`.** There is confirmed live drift of exactly the class `ts-rs` cannot fix: the `market_data:quote_update` event payload omits `symbol` (the store keys by subscribed symbol as a workaround) and `rust_decimal` serializes as a JSON *string* on raw events but as `f64` on commands that convert explicitly. Both are **protocol** bugs on **event channels**, not type-declaration bugs. `ts-rs` emits type declarations; `tauri-specta` emits typed `commands.*()` wrappers and typed `events.*.listen()`, which is where the actual drift lives.

Containment: do **not** derive `Type` across 40k LOC. Introduce `daytrader/src-tauri/src/dto.rs` as the single serialization boundary — every command and every `EventBridge` emission converts to `dto::*` first, with one-directional `From` impls. **Money crosses as a branded string, always** (`type Money = string & { __brand: 'Money' }` with one `parseMoney()` helper), never `f64`. That kills the command/event inconsistency by construction. CI gate: regenerate bindings, `git diff --exit-code`, plus grep gates for zero `invoke(`/`listen(` string literals outside `generated/`.

**The research surface itself:** a `/research` route with equity and underwater-drawdown curves, a virtualized trade blotter, the pattern × regime × hour attribution heatmap, walk-forward fold comparison, the parameter-sweep objective *surface* (not a leaderboard), and Monte Carlo percentile bands. Every view carries the R0.7 validity banner.

Stack: **TanStack Router** (type-safe routes pairing with generated types), **TanStack Query** wrapping generated commands (Tauri commands are RPC and benefit from the same caching/dedup machinery as HTTP), **Zustand 5** for push-based streams, **TanStack Table + Virtual** for blotters, **keep `lightweight-charts`, upgrade 4.1 → 5.x** and supplement with a small hand-rolled SVG layer for equity/underwater curves and pattern annotations. **Do not add Tailwind** — a trading UI has ~20 recurring primitives, not 500 utility combinations; formalize the existing CSS custom properties into a real token layer instead, with configurable up/down colours (red/green fails ~8% of male users and is inverted in some markets) and `font-variant-numeric: tabular-nums` on every number.

Testing, currently **zero** — `package.json` has no test dependencies at all: **vitest + @testing-library/react** for stores, hooks and every money-formatting path; **WebdriverIO + `tauri-driver`** for true end-to-end against the real binary. Note Playwright cannot drive a Tauri window — use it only against the Vite dev server with a mocked IPC layer.

Also here (cheap, overdue): `tauri.conf.json` has `"csp": null` and an 800×600 window with no minimum. Set a real CSP, `minWidth: 1280, minHeight: 800`, and an explicit capability allow-list.

---

## Phase R5 — ML signals, shadow-only

Only meaningful after R1 (you cannot evaluate a model without purged walk-forward) and R2 (features need a PIT store). Because of the `SignalSource` boundary, this phase adds a *peer*, not a subsystem.

**Transport: ONNX artifacts + `ort` in-process, behind a `--features ml` flag.** Train in Python offline (sklearn/LightGBM/PyTorch → ONNX), infer in Rust at µs scale with no IPC and no second runtime in the desktop bundle. Artifacts verified by SHA-256 at load. `candle` is pure-Rust and good for transformers but weaker for the gradient-boosted trees that actually win on tabular financial data; a Python sidecar is right for *training*, wrong for production inference. `ort` is internally `unsafe` — the crate boundary keeps `#![forbid(unsafe_code)]` intact in your code.

**Model registry with promotion that cannot skip stages:** `enum ModelStatus { Registered, Shadow, Canary { capital_fraction }, Active, Retired, Quarantined }`, where `Active` is reachable only from `Canary` and `Canary` only from `Shadow`. In research mode **nothing goes past Shadow** — the model runs live, emits signals, records everything, places zero orders, for a minimum of 60 trading days. Evaluate on hit rate, information coefficient, PnL-if-traded, and **correlation with existing sources** — a model correlating 0.9 with your hammer detector adds nothing.

### On agentic crowding

The concern that increasing agentic trading skews signals is correct and it is the most interesting design constraint in this project. As more agents trade the same public patterns, edge decays and the signal itself becomes a crowding indicator. Concretely:

- **Edge decay** — per-source rolling IC and expectancy with EWMA plus a **CUSUM change-point detector**; fit a decay curve and report `alpha_half_life_days`. Auto-quarantine on change-point fire. The auto enable/disable machinery already exists in `risk/pattern_limits.rs` — this only needs the trigger.
- **Crowding proxies** (you cannot observe other agents, only their footprints): signal-time price impact — how much does price move in the 1–5s *after* a canonical pattern completes, versus three years ago? Rising immediate impact with falling subsequent follow-through is the crowding signature. Also time-to-fill degradation on limit orders at pattern-completion prices, and **cross-sectional signal concentration** — if 40% of the universe fires the same pattern simultaneously it is a market-wide move, not an idiosyncratic signal, and should be down-weighted.
- **Non-stationarity discipline** — never train on a fixed window and deploy indefinitely. Mandate purged walk-forward with periodic retraining, deflated Sharpe fed by the sweep's trial count, and rejection below an OOS/IS ratio of 0.5.
- **Drift monitoring** — PSI and KS tests on each feature's live distribution versus training; auto-demote to Shadow on breach.

**Design stance: treat every signal source as having a finite, measurable lifespan.** The durable advantage is not any pattern — it is infrastructure that detects decay and rotates sources faster than the decay rate. That is the thing worth building, and it is the honest answer to the agentic-skew question.

---

## Phase L — Live-capital hardening (deferred, gated)

Not started until there is a decision to trade real money. Listed so R0–R2 build toward it rather than against it.

- **Order lifecycle typestate** — `Order<S: State>` with sealed states `Draft` → `RiskApproved { decision: RiskDecisionId }` → `Submitted` → `Filled`/`PartiallyFilled`/`Cancelled`/`Rejected`. `OrderEngine::place` accepts only `Order<RiskApproved>`, and the risk engine is the sole minter of `RiskDecisionId` — so "submit without a risk check" becomes *unwritable*, enforced by the compiler rather than by code review. Persistence gets a dynamic `#[non_exhaustive] enum AnyOrder` with `TryFrom<OrderRow>` validating the transition graph once, on load. Verify with a `trybuild` UI test asserting the illegal call fails to compile.
- **PDT persistence (B-066)** — implement `SqlitePdtRepository`; `AppState.pdt_repository` is always `None` today, so PDT state does not survive a restart. Reconcile against the broker at startup (`GET /v2/account` → `daytrade_count`, `pattern_day_trader`); treat the broker as truth and local state as a pre-trade predictor. Block, not warn, at <$25k equity.
- **Pre-trade controls in the spirit of SEC 15c3-5** — Rule 15c3-5 binds broker-dealers, not you, but its control list is the correct checklist: max order notional and quantity, **fat-finger price collar** (currently absent, a classic loss source), duplicate-order detection via idempotency key, max orders/sec, gross and net exposure caps, per-symbol concentration, daily loss, PDT, settled-cash, restricted symbols, and a market-state check (`utils/market_hours.rs` already has DST-correct sessions). Emit a `RiskDecision` record for every check — pass *and* fail.
- **Kill switch (B-061)** — two-tier: *Disarm* (stop new entries, hold positions) and *Flatten* (cancel orders, market-close all, disarm, require passphrase to re-arm). **Persist the state in `app_settings`** — the current in-memory design means a crash-restart silently re-enables. Triggers: manual global hotkey, daily-loss breach, broker connectivity loss beyond N seconds with open positions, and a watchdog that disarms if the event loop stalls >5s.
- **Credential storage & app lock** — `keyring` (declared, entirely unused today) for OS keychain; `secrecy::SecretString` + `zeroize` replacing plain `String` on `AlpacaConfig`; env vars demoted to a `#[cfg(debug_assertions)]` dev escape hatch. Argon2id passphrase verified at launch and re-verified before privileged actions, with idle auto-lock. **Encryption at rest: rely on OS full-disk encryption for v1.** SQLCipher requires abandoning sqlx's sqlite for a custom `libsqlite3-sys` build, and field-level AES breaks `SUM`/`ORDER BY` on exactly the P&L columns you must aggregate — a large cost for a small threat delta on a machine that already has FileVault. Document as an explicit, revisitable decision in `SECURITY.md`.
- **Tamper-evident audit trail** — append-only `order_audit` recording every order transition, every risk decision, every config change, every arm/disarm, chained by `entry_hash = sha256(prev_hash ‖ canonical_json(entry))` with periodic signed checkpoints (`sha2` is already a dependency, currently unused). Populate `audit_log.user_id` — the column exists and is never written — with a stable `OperatorId`, which future-proofs multi-operator without a migration.
- **`AutomationState`** — replace `auto_execution_enabled: bool` with `enum AutomationState { Disabled, ShadowOnly, Armed { armed_at, armed_by } }`, making "armed with no audit trail" unrepresentable. Armed-with-real-money must look visibly different from armed-with-paper in the UI chrome.

**Compliance scope, stated plainly:** as a single operator trading your own money you are not a broker-dealer, RIA, or ATS — no Reg SCI, no CAT reporting, no best-execution obligation, no Form ADV. PDT is enforced by *your broker*; your job is never to breach it. Wash sales (IRC §1091) are a **tax** matter, not a trading restriction — compute `wash_sale_flag` and disallowed loss on realized losses in `positions/pnl.rs` (the FIFO lots are already there) and surface them in a year-end export, but **do not auto-block trades on wash-sale grounds**; that is a strategy decision, and the app reports rather than advises. If you ever trade others' money, all of this changes and the legal entity, not the app, is the constraint.

---

## Boundary summary — where illegal states get eliminated

| Boundary | Current failure | Mechanism | Phase |
|---|---|---|---|
| config → domain | 12 raw `f64` + `derive(Default)` ⇒ default config disables all limits | `RawRiskConfig` → `TryFrom` → `RiskConfig`, no `Default`, `deny_unknown_fields` | R0.4 |
| broker wire → domain | JSON deserialized into semi-domain types; unknown enums coerce | `*Wire` structs only in `alpaca/types.rs`; `TryFrom` with `UnknownVariant` errors — **never** `#[serde(other)]` on anything gating capital; domain enums `#[non_exhaustive]` | R0/R2 |
| domain → db | lossy `f64` round-trip | `Money<6>` with `sqlx::{Type,Encode,Decode}`; row structs separate from domain, joined by `TryFrom` | R0.2 |
| domain → TS/UI | hand-duplicated interfaces; Decimal-as-string vs f64; missing `symbol` field | `tauri-specta` over a single `dto` module; money as a branded string, always | R4 |
| event bus ↔ modules | duplicate `Quote`/`Order`/`Position`; `MarketEvent` embeds a `patterns::` type | `src/domain/` leaf module; events carry `SignalId` + flat fields | R2 |
| detector / model plugin | open trait, 3 unused hierarchies | sealed `SignalSource`; models are peers of detectors | R2 |
| time (the subtle one) | nothing prevents reading future data | `SignalContext.as_of` PIT anchor + DB `CHECK(watermark <= bar_ts)` + `as_of`-scoped query API | R2 |
| order lifecycle | mutable `status` field; `Filled → Pending` is representable | typestate `Order<S>`; `place()` accepts only `Order<RiskApproved>` | L |

---

## Performance: budget first, optimization second

| Stage | p50 | p99 |
|---|---|---|
| WS frame → parsed `Quote` | 20 µs | 100 µs |
| Bar close → all 9 detectors | 150 µs | 500 µs |
| Signal → composer + context | 50 µs | 200 µs |
| Signal → risk decision (no I/O) | 100 µs | 400 µs |
| Risk approve → bytes on wire | 200 µs | 1 ms |
| **In-process total** | **~525 µs** | **<2.5 ms** |
| Broker RTT (uncontrollable) | 50 ms | 250 ms |

**The honest read: in-process latency is under 1% of end-to-end. This is not an HFT system and pretending otherwise wastes effort.** Performance matters in exactly three places: **backtest throughput** (R1 walk-forward over years × hundreds of symbols — target ≥1M candle-detections/sec/core, and this is where the 10–50× Decimal penalty is the whole ballgame), **not blocking the Tokio runtime** (a `std::sync::Mutex` held across an `.await` in the event bridge stalls everything, including the kill switch — R0.5), and **UI responsiveness under a full feed**.

Where each numeric type belongs: `Money<6>` integer fixed-point is **mandatory** for all cash, prices-as-money, P&L, balances, commissions, and anything compared to a limit — and it is *faster* than `Decimal` while being exactly as correct, so R0.2 is a free performance win. `f64` is **explicitly sanctioned** for indicator math, candle geometry ratios, confidence scoring, statistical metrics, and ML features — these are shape measurements, not values of money. An RSI of 69.9999999 versus 70.0000001 is a threshold-hysteresis question, solved with an explicit epsilon band, not with `Decimal`. Codify with `#![deny(clippy::float_arithmetic)]` in `src/risk/`, `src/orders/`, `src/positions/` and an explicit module-level allow with a policy comment in `src/patterns/indicators.rs`.

Data structures: `CandleRing<const N: usize>` replacing per-call reallocated slices (safe `[Candle; N]` + head index — `#![forbid(unsafe_code)]` is set and stays); interned `Symbol(Arc<str>)`; `DashMap` for hot per-symbol caches (already the codebase convention) but **not** a blanket replacement — `DashMap` is worse than `RwLock<HashMap>` for iterate-everything workloads and can deadlock on nested access; `ArcSwap` for read-mostly config. Zero-copy WS deserialization (`simd-json` with `#[serde(borrow)]`) only if the p99 above is actually missed — measure first.

---

## Verification

Target `scripts/verify_agent_work.sh` (13–17 new):

```
 1 fmt                              9 test quality (+ proptest on indicators)
 2 cargo check --workspace         10 production safety (unwrap=0, std::sync=0)   ← R0
 3 clippy -D warnings        ← R0  11 frontend build + tsc (unconditional)
 4 unit tests, 0 known failures ← R0 12 backlog updated
 5 mock-feature tests              13 scripts/verify_perf.sh (criterion <10% regress) ← R0/R1
 6 integration tests (uncond.)     14 cargo deny + cargo machete                      ← R0
 7 cargo doc                       15 cargo sqlx prepare --check                      ← R0.2
 8 architecture (baseline empty) ← R2  16 bindings up-to-date (git diff)              ← R4
                                   17 npm test + e2e                                  ← R4
```

**End-to-end validation of the research claim** — the tests that actually matter:

1. **Split correctness** — replay a known 4:1 split; assert no signal fires on the split date. Fails today.
2. **Money exactness** — a 1000-trade P&L fixture; assert the total equals the hand-computed exact cent value. The current f64 path will not match; that divergence is the proof.
3. **Lookahead** — shift the input series one bar forward; assert every signal timestamp shifts exactly one bar and no metric improves.
4. **Determinism** — identical metrics across 100 runs and 1/4/16 threads.
5. **PDT realism** — a $10k backtest produces strictly fewer trades than the same config at $50k.
6. **Train/serve skew** — the same bar through the live path and the backtest path produces byte-identical `FeatureVector`s.
7. **Manual run** — `cargo run --bin backtest -- run --symbols AAPL,MSFT --from 2023-01-01 --to 2025-01-01`, then the same config through the `/research` UI, and confirm the numbers agree.

---

## Critical files

- `src/brokers/alpaca/rest.rs` — line 250 `("adjustment","raw")`; the split bug
- `src/config/mod.rs` — `RiskConfig`, all-`f64` with a dangerous `Default`
- `src/db/models.rs` + `migrations/` — the money representation break
- `src/patterns/traits.rs` — lines 307/320/333, the `to_string().parse()` hot path
- `src/brokers/types.rs` — line 114 same pathology; canonical newtypes to move into `src/domain/`
- `src/execution/backtest.rs` — the engine R1 extends
- `src/patterns/signals.rs` — `SignalComposer` → `SignalSource` combinator; holds the `patterns → risk` violation
- `src/events/types.rs` — duplicate types; `MarketEvent` embedding a `patterns::` type
- `scripts/verify_agent_work.sh` + `.governance/*` — the gate and its ratchets
- `daytrader/src-tauri/src/{state.rs,events.rs}` — `std::sync` locks; future `dto.rs` boundary

## Sequencing

**R0 → R1 → ⭑Go/No-Go → then R2/R3/R5 as the verdict directs, R4 whenever the UI becomes the bottleneck on interpreting results. Phase L only on a decision to trade live capital.**

Two items to pull forward regardless: **R0.1 (split adjustment)** — it is an active correctness bug that poisons the exact answer being sought — and **R0.3 (the string-parse removal)** — one afternoon, 10–100× on the hottest path, zero risk.
