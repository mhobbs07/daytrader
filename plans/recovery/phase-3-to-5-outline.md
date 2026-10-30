# Phases 3–5: Outline

These phases are deliberately not task-specified yet. Phase 1 renames and relocates most of what they touch; writing exact file lists now would produce specs that are wrong by the time they are used. When Phase 2 exits, a **[design]** task produces the Phase 3 specs in the same format as Phase 0, against the real crate layout.

## Phase 3: One real loop

**Shape (decided):** `dt-engine` owns a single ordered pipeline per symbol:

```
Bar → Strategy::on_bar → Option<Signal>
    → RiskGate::check(&OrderIntent) -> Result<Approved, Rejection>   // synchronous, in the order path
    → Broker::submit(Approved, ClientOrderId)
    → FillStream (broker trade updates + reconciliation poll) → Portfolio::apply_fill
    → Store::persist(txn: order + fill + position + risk counters)
    → RiskState::on_fill
    → ExitRules::on_bar (stop, target, time, end-of-day flatten) → OrderIntent …
```

The event bus remains for fan-out (UI, metrics, logs) only. Order and fill traffic uses bounded `mpsc` channels; nothing in the money path is lossy.

**Work items (to be specified):**
- `RiskGate` trait in `dt-risk` with a synchronous `check`; `OrderEngine` calls it and refuses to submit on `Err`. Delete the async-advisory path.
- `Broker` trait in `dt-broker`: `submit`, `cancel`, `replace`, `open_orders`, `positions`, `account`, `fills() -> impl Stream<Fill>`; `SimBroker`, `AlpacaBroker`.
- Alpaca: `trade_updates` WebSocket for fills; reconnect-on-any-exit with backoff, resubscribe, pong timeout; wait for `authenticated` before subscribing; complete status enum; 403/422 mapping; `client_order_id`; fractional quantities; redacted `Debug` for secrets; `keyring`/env credential loader.
- `BrokerKind::{Sim, Paper, Live}` factory from config. `Live` requires `broker.live = true`, a URL that matches, and `DAYTRADER_CONFIRM_LIVE=<account id>` in the environment; the binary prints a banner and refuses otherwise.
- `MarketDataSource` trait: `AlpacaStream` and `ReplaySource` both yield `Bar`s into the pipeline. Decide the fate of `src/data/ingestion.rs` here.
- Exits: evaluate exit rules per bar for each open position; `close_position` places a real order; end-of-day flatten at a configurable time before close.
- Persistence (`dt-store`): `sqlx::migrate!` as the only schema source; `trades` written on every fill; order status by `broker_order_id`; one transaction per fill; `accounts` row created on first run; rehydrate open orders, positions, PDT window, daily P&L, halt state on start; `reconcile` compares store vs broker and reports/repairs drift.
- `dtctl`: `run --sim|--paper|--live`, `reconcile`, `fetch`, `backtest`. `src/main.rs` deleted.
- Tauri: becomes a view over `dt-engine` state plus commands to toggle automation and flatten; PDT panel reads the real enforcer; no re-implemented rules.

**Exit:** `dtctl run --paper` against Alpaca paper completes a full session, opens and closes trades, survives a forced WebSocket drop and a process restart without losing or duplicating state; `dtctl reconcile` reports zero drift; `STATUS.md` row "paper trading" is proven by a recorded session log checked into `docs/sessions/`.

## Phase 4: Backtesting as the gate

- `SimBroker` fill model: market → next bar open ± slippage; limit → fills only if the bar's range touches the limit (at limit, or at open if gapped through); stop → triggers on touch then fills as market; partial fills by configurable participation rate; latency in bars or ms; commission schedule; seeded RNG; fills stamped with the stepped clock. Slippage and commission are applied **at the fill**, so the portfolio and the report agree.
- `dt-backtest` runner drives the same `dt-engine` pipeline with the risk engine **running**, deterministic ids (`ClientOrderId` derived from a seed + sequence), stable output ordering.
- Data: `dtctl fetch --symbols SPY,AAPL,TSLA --from --to --timeframe 1Min` → Parquet (or CSV) under `data/history/`; committed fixtures under `tests/data/` for ~10 trading days including a DST change and a holiday; a validation step rejects gaps and bad OHLC.
- Report: total/annualised return, max drawdown and duration, Sharpe, Sortino, profit factor, expectancy, per-pattern and per-regime tables, equity curve; JSON + Markdown.
- CI test: run the fixture backtest twice, assert byte-identical JSON; compare against `tests/data/expected/*.json` and fail on any change (update deliberately with a `--bless` flag).

**Exit:** as above; the fixture backtest is a required CI check.

## Phase 5: Paper, then live

- 30 trading days on Alpaca paper with the `dtctl` binary; nightly `dtctl backtest` over the same window; a report comparing fills, P&L and risk events; the go/no-go criteria from `plans/backtesting-requirements.md` (archived under `docs/history/`) still apply.
- Live opt-in only after the owner signs off the report in `docs/sessions/`.
