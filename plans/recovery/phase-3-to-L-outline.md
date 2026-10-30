# Phases 3, 4, 5 and L: Outline

These phases are deliberately not task-specified yet. Phase 1 renames and relocates most of what they touch, so exact file lists written now would be wrong by the time they are used. When Phase 2 exits, a **[design]** task writes the Phase 3 and Phase 4 specs in the same format as Phase 0, against the real crate layout.

The detailed designs for the research items live in `plans/2026-07-28-research-platform-roadmap.md`; section references below (R0.7, R1, ...) point there. Order and IDs are set by `README.md` and `BACKLOG.md`.

## Phase 3: One correct pipeline, in simulation

The backtester replays through the production pipeline, so the pipeline must be correct before any backtest number means anything. This phase makes it correct against the simulated broker only. Live connectivity is Phase L.

**Shape (decided):** `dt-engine` owns a single ordered pipeline per symbol:

```
Bar → Strategy::on_bar → Option<Signal>
    → RiskGate::check(&OrderIntent) -> Result<Approved, Rejection>   // synchronous, in the order path
    → Broker::submit(Approved, ClientOrderId)
    → FillStream → Portfolio::apply_fill
    → Store::persist(txn: order + fill + position + risk counters)
    → RiskState::on_fill
    → ExitRules::on_bar (stop, target, time, end-of-day flatten) → OrderIntent …
```

The event bus remains for fan-out (UI, metrics, logs) only. Order and fill traffic uses bounded `mpsc` channels; nothing in the money path is lossy.

**Work items (to be specified):**
- `RiskGate` trait in `dt-risk` with a synchronous `check`; `OrderEngine` refuses to submit on `Err`. Delete the async-advisory path.
- `Broker` trait in `dt-broker` (`submit`, `cancel`, `replace`, `open_orders`, `positions`, `account`, `fills() -> impl Stream<Fill>`) with `SimBroker` as the only implementation needed in this phase.
- Exits: exit rules evaluated per bar for every open position; `close_position` places a real order; end-of-day flatten at a configurable time before the close.
- Persistence (`dt-store`): `sqlx::migrate!` as the only schema source (this retires `schema.sql`, `MigrationManager` and the hand-written migration DDL, including the corporate-action tables added by #39); `trades` written on every fill; one transaction per fill; an `accounts` row on first run; rehydrate open orders, positions, PDT window, daily P&L and halt state on start.
- `dtctl backtest` and `dtctl fetch` exist as stubs that drive this pipeline; `src/main.rs` is deleted.

**Exit:** a scripted sim session opens and closes trades on fixture data, is killed and restarted mid-session without losing or duplicating state, and every submitted order carries a `RiskGate` approval.

## Phase 4: Research platform, then the verdict

The core deliverable. Implements roadmap R0.7 and R1 on top of the Phase 3 pipeline.

| ID | Item | Roadmap |
|---|---|---|
| B-209 | Data-quality gates (gap, OHLC coherence, volume, outlier, stale quote, monotonic time) with a `data_quality_events` quarantine table, and the one-off free-tier divergence audit committed as `docs/data-validity.md`. **First task of the phase**: its number is the caveat on every later report. | R0.7 |
| B-223 | Corporate-action adjustment correctness before the harness relies on it: timestamp-ordered dividend references, distinct same-day dividends and corrections, per-action (not per-candle) factor computation, split-adjusted volume, and an explicit as-of bound so future actions never alter a historical backtest. Found in the #39 review; `adjust_candles` has no runtime caller yet. | R0.1 |
| B-210 | Backtest realism: `FillModel` trait (spread crossing, square-root impact, participation; limits fill only when price trades **through**), short side with borrow model, PDT accrual during replay, `HoldingPolicy` with end-of-day liquidation. Slippage and commission are applied at the fill so the portfolio and the report agree. | R1 |
| B-211 | Walk-forward folds, purged K-fold with embargo, deflated Sharpe fed by the sweep's trial count, OOS/IS ratio rejection below 0.5, plateau-robustness score. | R1 |
| B-216 | Regime classification (threshold-on-percentiles first, not an HMM); regime-bucketed pattern stats. Moved here from Phase 5: B-212's attribution and the B-222 verdict are per regime, so they need it. | R2 |
| B-212 | Metric suite (gross and after-cost), Monte Carlo trade-order bootstrap, per-pattern × regime × hour attribution (regimes from B-216), benchmark vs SPY. | R1 |
| B-213 | `dtctl backtest` CLI; seeded RNGs; byte-identical results across runs and thread counts; lookahead detector (shift input one bar, assert signals shift exactly one bar and no metric improves); committed fixtures under `tests/data/` (a few liquid symbols, ~10 trading days spanning a DST change, a holiday and a known split) run in CI against blessed expected reports. | R1 + recovery's old Phase 4 |

**Exit:** the roadmap's R1 definition of done (golden-file determinism across 100 runs and 1/4/16 threads; lookahead detector; a $10k account trades strictly less than a $50k one; throughput bench gated at <10% regression; CLI smoke test in CI).

### ⭑ B-222 Go/No-Go checkpoint — **human**

Run the existing detectors through the harness and read the result, per pattern and per regime: expectancy after realistic costs, deflated Sharpe, OOS/IS ratio, and the B-209 caveats.

- **Nothing survives costs:** stop. The negative is the finding. Optionally continue to the scanner (B-217) on the specific, testable hypothesis that pattern edge depends on universe selection.
- **Something survives:** continue to Phase 5, in the order the result suggests.

Record the decision and the report in `docs/sessions/`.

## Phase 5: After the verdict

Ordered by the Go/No-Go result, not fixed in advance. Designs are in the roadmap.

| ID | Item | Roadmap |
|---|---|---|
| B-215 | Sealed `SignalSource` trait with all detectors adapted; feature store with point-in-time `CHECK` constraints and a restatement log; one feature code path for live and backtest. | R2 |
| B-217 | Universe scanner: point-in-time snapshots, liquidity gates, time-of-day-normalized RVOL and other free-tier factors, ranking with rationale; stored daily so the scanner itself is backtestable. | R3 |
| B-218 | `tauri-specta` bindings behind a single `dto.rs` boundary; money crosses as a branded string. | R4 |
| B-219 | Research UI: equity and drawdown curves, blotter, attribution heatmap, sweep surface, validity banner. | R4 |
| B-220 | ML signals behind `--features ml`, shadow-only, with a model registry, drift and decay monitoring, crowding proxies. | R5 |

The domain leaf module (B-214) is not here: it is `dt-core`, built in Phase 1.

## Phase L: Live capital (only on an explicit decision)

Not started until the owner decides to trade real money. Listed so earlier phases build toward it rather than against it.

**Live connectivity (from the recovery plan):**
- `BrokerKind::{Sim, Paper, Live}` factory from config. `Live` requires `broker.live = true`, a URL that matches, and `DAYTRADER_CONFIRM_LIVE=<account id>` in the environment; the binary prints a banner and refuses otherwise.
- Alpaca: `trade_updates` WebSocket for fills; reconnect on any exit with backoff, resubscribe and a pong timeout; wait for `authenticated` before subscribing; complete order-status enum; correct 403/422 mapping; `client_order_id`; fractional quantities; redacted `Debug` for secrets.
- Fill reconciliation is automatic, not only a command. At startup and after every reconnect, the engine pulls orders, fills since the last applied fill, and positions over REST and applies what the stream missed. The risk gate accepts no new order until that completes. A periodic reconciliation poll runs alongside the stream, and drift it cannot repair halts trading.
- `MarketDataSource` with `AlpacaStream` publishing bars into the Phase 3 pipeline.
- `dtctl run --paper|--live` and `dtctl reconcile` (the same reconciliation, run by hand: store vs broker, report and repair drift).
- Tauri becomes a view over `dt-engine` state plus commands to toggle automation and flatten; the PDT panel reads the real enforcer.

**Hardening (roadmap Phase L):** order-lifecycle typestate so submitting without a risk approval does not compile; PDT persistence reconciled against the broker at startup; pre-trade controls in the spirit of SEC 15c3-5 (fat-finger price collar, idempotency, rate and exposure caps); two-tier kill switch persisted across restarts; `keyring` credentials and an app lock; tamper-evident audit trail; `AutomationState { Disabled, ShadowOnly, Armed }`.

**Paper, then live:** 30 trading days on Alpaca paper with the same binary; nightly backtest over the same window; a report comparing fills, P&L and risk events. Live opt-in only after the owner signs off that report in `docs/sessions/`.
