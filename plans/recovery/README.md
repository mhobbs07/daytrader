# DayTrader Program: Recovery and Research

**Status:** Adopted 2026-10-30. Merges two plans into one: the Recovery Program (`reports/honest-assessment-2026-10-30.md`) and the Research-Platform Roadmap (`plans/2026-07-28-research-platform-roadmap.md`). **This file decides order, scope and IDs.** The roadmap remains the design reference for the research items (its R0–R5 and L sections hold the detailed designs); where the two disagree, this file and `BACKLOG.md` win. The older phase plans under `plans/` describe work as complete that does not run; they are history, not guidance.
**Executing model:** Opus 5.5 for implementation tasks. Design tasks marked **[design]** are done once by a stronger model or the owner. Haiku is prohibited for this repo (see `.claude/rules/agent-models.md`).

## Goal

**Near term: a trustworthy verdict on whether this approach makes money at all.** The most valuable early output is an honest answer, including an honest "no", delivered before the expensive phases. **Long term, and only on a separate decision: live capital.** Everything before Phase L builds toward live trading without enabling it.

A verdict is only as good as the machine that produces it, so the program first makes the code trustworthy (Phases 0–2), then makes the one pipeline correct in simulation (Phase 3), then builds the research harness and reads the verdict (Phase 4).

## Decisions already made (do not relitigate in a task)

1. **One trunk.** `main` is the only long-lived branch (done, B-086). Every PR targets `main`.
2. **One repository, many crates.** The single `daytrader` crate becomes a Cargo workspace in Phase 1. The repository is not split. The Tauri app stays in `daytrader/`.
3. **CI is the verifier.** `scripts/verify_agent_work.sh` and `.github/workflows/ci.yml` run the same steps. "Done" is a green CI run on the PR, not an agent's statement.
4. **Money is fixed-point integer micros** (decided 2026-10-30, B-204): `Money(i64)` at 6 decimal places in memory, `INTEGER` `*_micros` columns in SQLite. Ratios (win rate, confidence, percentages, multipliers) are not money and stay `REAL`/`f64`. The `Decimal`-for-money rule in `.claude/rules/type-safety.md` and `CLAUDE.md` is replaced when B-101 lands the type.
5. **Automation stays OFF** (paper-only, gate defaulted off) until Phase L. This is what makes deferring the live-capital work safe.
6. **Desktop only, no server tier. Free-tier market data**, with the provider abstraction built so paid feeds drop in later.
7. **The honesty constraint.** Free-tier IEX data is a minority slice of volume and has no delisted tickers. Survivorship bias and feed unrepresentativeness cannot be fixed on this budget, so every research report states them, with the divergence number measured in B-209.
8. **Feature freeze until Phase 0 exits.** Exception granted once: the research sweep (#39), already built and tested, which fixes a backtest correctness bug.
9. **In-flight work:** everything open as of 2026-10-30 has landed (#29 to #39). Nothing else is grandfathered.

## Phases

| Phase | Spec | Goal | Exit criterion |
|---|---|---|---|
| 0 | `phase-0-truth-and-build.md` | Clean clone → green CI; dead code and dead deps gone; docs truthful | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc`, `cargo sqlx prepare --check`, `cargo deny`, `cargo machete` all green in CI on `main`, with no excused failures |
| 1 | `phase-1-core-and-workspace.md` | One definition per domain concept, money as integer micros, `Clock` everywhere, crate-enforced boundaries | Workspace of crates builds; zero `Utc::now()` outside `SystemClock`; converters deleted; no `REAL` money columns; money-exactness fixture passes |
| 2 | `phase-2-risk-correctness.md` | Every known risk-engine bug fixed with a scenario test | `dt-risk` scenario suite replays a scripted day and asserts every halt and rejection |
| 3 | `phase-3-to-L-outline.md` | One correct pipeline **in simulation**: synchronous risk gate, fills into one portfolio, exits, persistence and rehydration, against `SimBroker` | A scripted sim session opens and closes trades, survives a restart without losing state, and every order passed the risk gate |
| 4 | `phase-3-to-L-outline.md` | Research platform: data-quality gates, realistic fills, PDT and EOD in replay, walk-forward and purged CV, metrics, CLI, determinism | Roadmap R1 definition of done; then the **⭑ Go/No-Go checkpoint** (B-222) |
| 5 | `phase-3-to-L-outline.md` | After the verdict: point-in-time features and the `SignalSource` boundary, regimes, universe scanner, research UI, ML in shadow, **in the order the verdict directs** | Per item, from the roadmap's R2–R5 definitions of done |
| L | `phase-3-to-L-outline.md` | Live capital, only on an explicit decision: live broker connectivity, roadmap Phase L hardening, 30 trading days of paper, then opt-in | Paper/backtest gap within tolerance; owner sign-off recorded in `docs/sessions/` |

Phases 3 onward are outlined, not task-specified. They get task specs when Phase 2 exits, because Phase 1 renames and moves most of what they touch.

### Where the roadmap's items went

| Roadmap | Merged into | IDs |
|---|---|---|
| R0.1 split adjustment, R0.3 hot-path parse, R0.4 config spine | Done on the sweep branch, landed by #39 | B-201, B-202, B-203, B-205 |
| R0.2 money fixed-point | Phase 1 (`Money` in `dt-core`, columns migrated) | B-204 → B-101, B-108 |
| R0.5 honest CI gate | Phase 0 | B-206 → B-088 (done), B-089 to B-094 |
| R0.6 dependency cleanup | Phase 0 deletions; upgrades after Phase 0 | B-208 → B-095; upgrades stay B-208 |
| R0.7 data-quality gates and divergence audit | Phase 4, first task | B-209 |
| R1 backtester → research platform | Phase 4 | B-210 to B-213 |
| R2 domain leaf, `SignalSource`, PIT feature store, regimes | Domain leaf → Phase 1 (`dt-core`); the rest → Phase 5 | B-214 → B-101 to B-117; B-215, B-216 |
| R3 scanner, R4 research UI, R5 ML | Phase 5 | B-217 to B-220 |
| Phase L | Phase L, together with the recovery plan's live-connectivity work | specified at Phase L |

## How a task is written

Every task has: **ID**, **Goal** (one sentence), **Who** (`agent` or `human` or `[design]`), **Files it may touch** (an allow-list), **Steps**, **Proof** (the exact command(s) that must pass), **Exit** (observable end state), **Size** (S ≤100 changed lines, M ≤300, L >300 and must be split before it is handed out).

## Rules for the executing model

1. Claim exactly one task. Put it "In Progress" in `BACKLOG.md` with the date and branch.
2. Branch from `main`: `fix/<id>-<slug>`, `refactor/<id>-<slug>`, `chore/<id>-<slug>`.
3. Touch only the files the task lists. If the task cannot be done within them, stop and report; do not widen the scope.
4. Write or name the proving test before changing production code. Run the task's **Proof** commands and paste their output in the PR body.
5. No `.unwrap()`/`.expect()`/`panic!` outside `#[cfg(test)]`; no `std::sync` locks; money as `Decimal` until B-101 lands `Money`, then `Money` only; no new `Utc::now()` (use the injected `Clock` once Phase 1 provides it); `f64` only for ratios, indicators and statistics.
6. Never skip, ignore, or baseline a test to get green. If a test is wrong, fix the test and say why in the PR.
7. One PR per task. Conventional commit subject ≤72 chars. PR body: Goal, Proof output, files changed, anything left undone.
8. "Done" is set after CI is green on the PR, not before. The agent may set it only by linking the green run.
9. Never force-push a branch you did not create in this session, and never force-push `main`.
10. If something outside the task looks wrong, add a BACKLOG row for it. Do not fix it in passing.
11. Every research report (Phase 4 onward) carries the honesty-constraint banner and the B-209 divergence number.

## Target crate layout (Phase 1)

```
Cargo.toml                 workspace root (no code)
crates/
  dt-core/                 domain types (Money as i64 micros), Clock, calendar, errors. No I/O, no tokio.
  dt-strategy/             indicators, candlestick patterns, signal generation     → core
  dt-risk/                 risk engine, PDT, limits, sizing                         → core
  dt-engine/               order lifecycle, portfolio, execution pipeline            → core, risk, strategy
  dt-broker/               Broker + MarketDataSource traits, SimBroker               → core
  dt-alpaca/               Alpaca REST + WS implementing dt-broker traits            → core, broker
  dt-store/                sqlx persistence, migrations, repositories                → core
  dt-backtest/             replay runner, loaders, metrics, report                   → engine, broker, store
bins/dtctl/                headless CLI: backtest | fetch | (Phase L: run --paper | reconcile)
daytrader/                 Tauri + React app (unchanged location)                    → engine (facade)
```

Dependency arrows point downward only. Cargo enforces them; `scripts/verify_architecture.sh` is deleted once the split is complete. `dt-core` is the roadmap's `src/domain/` leaf module, made a crate.

## What replaces the current governance

Keep: root `CLAUDE.md` (trimmed), `BACKLOG.md`, the dangerous-command hook, `/backlog-task`, `/verify-commit`, `/code-review`, `/promote`.
Delete after Phase 1: per-module `CLAUDE.md` boundary files, `verify_architecture.sh`, `architecture_exceptions.txt`, `architecture_violations_baseline.txt` (crates replace them).
Delete in Phase 0: `known_test_issues.txt` escape hatch, `production_safety_baseline.txt` ratchet (replaced by `-D warnings` and clippy lints that forbid `unwrap_used`/`expect_used` in non-test code). Until then, every phase must shrink at least one `.governance/` ratchet file: they are debt registers, not licences.
Replace: `plans/*` (except `plans/recovery/` and the roadmap) and `reports/phase-*` → `docs/history/`; add `STATUS.md` that lists what runs end to end and the test that proves each line.
