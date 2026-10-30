# DayTrader Recovery Program

**Status:** Adopted 2026-10-30. Supersedes the phase plans under `plans/` (those describe work as complete that does not run; they are history, not guidance).
**Basis:** `reports/honest-assessment-2026-10-30.md` (the audit this program answers).
**Executing model:** Opus 5.5 for implementation tasks. Design tasks marked **[design]** are done once by a stronger model or the owner. Haiku is prohibited for this repo (see `.claude/rules/agent-models.md`).

## Decisions already made (do not relitigate in a task)

1. **One trunk.** `main` becomes the only long-lived branch. `staging` is fast-forwarded into `main` and deleted (B-086). Until B-086 lands, PRs target `staging`; after it, `main`. No other long-lived branches.
2. **One repository, many crates.** The single `daytrader` crate is split into a Cargo workspace (Phase 1). The repository is not split. The Tauri app stays in `daytrader/`.
3. **CI is the verifier.** `scripts/verify_agent_work.sh` and `.github/workflows/ci.yml` run the same steps. "Done" is a green CI run on the PR, not an agent's statement.
4. **Merge order for in-flight work:** [#31](https://github.com/mhobbs07/daytrader/pull/31) (this plan) → B-085 fix PR into `feat/close-trading-loop` → [#30](https://github.com/mhobbs07/daytrader/pull/30) → [#29](https://github.com/mhobbs07/daytrader/pull/29) `feat/close-trading-loop` → then Phase 0. [#6](https://github.com/mhobbs07/daytrader/pull/6) is superseded and should be closed.
5. **Feature freeze** until Phase 0 exit criteria are met. No new strategy, UI or broker features.

## Phases

| Phase | File | Goal | Exit criterion |
|---|---|---|---|
| 0 | `phase-0-truth-and-build.md` | Clean clone → green CI; dead code gone; docs truthful | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc` all green in CI on the trunk, with no excused failures |
| 1 | `phase-1-core-and-workspace.md` | One definition per domain concept; `Clock` everywhere; crate-enforced boundaries | Workspace of crates builds; zero `Utc::now()` outside `SystemClock`; converters deleted |
| 2 | `phase-2-risk-correctness.md` | Every known risk-engine bug fixed with a scenario test | `dt-risk` scenario suite replays a scripted day and asserts every halt/rejection |
| 3 | `phase-3-to-5-outline.md` | One real loop: sync risk gate, fill stream, exits, persistence, rehydration, broker factory | `dtctl run --paper` completes a session, survives WS drop and restart, `dtctl reconcile` shows zero drift |
| 4 | `phase-3-to-5-outline.md` | Deterministic backtest with committed fixtures as a CI gate | Same fixture twice → byte-identical report; result change fails CI |
| 5 | `phase-3-to-5-outline.md` | 30 days paper, then live opt-in | Paper/backtest gap within tolerance; risk-event log reviewed |

Phases 3–5 are outlined, not task-specified. They get task specs when Phase 2 exits, because Phase 1 will change the names everything is written against.

## How a task is written

Every task has: **ID**, **Goal** (one sentence), **Who** (`agent` or `human` or `[design]`), **Files it may touch** (an allow-list), **Steps**, **Proof** (the exact command(s) that must pass), **Exit** (observable end state), **Size** (S ≤100 changed lines, M ≤300, L >300 and must be split before it is handed out).

## Rules for the executing model

1. Claim exactly one task. Put it "In Progress" in `BACKLOG.md` with the date and branch.
2. Branch from the trunk: `fix/<id>-<slug>`, `refactor/<id>-<slug>`, `chore/<id>-<slug>`.
3. Touch only the files the task lists. If the task cannot be done within them, stop and report; do not widen the scope.
4. Write or name the proving test before changing production code. Run the task's **Proof** commands and paste their output in the PR body.
5. No `.unwrap()`/`.expect()`/`panic!` outside `#[cfg(test)]`; no `std::sync` locks; `Decimal` for money; no new `Utc::now()` (use the injected `Clock` once Phase 1 provides it).
6. Never skip, ignore, or baseline a test to get green. If a test is wrong, fix the test and say why in the PR.
7. One PR per task. Conventional commit subject ≤72 chars. PR body: Goal, Proof output, files changed, anything left undone.
8. "Done" is set after CI is green on the PR, not before. The agent may set it only by linking the green run.
9. Do not run `git push --force` on any branch you did not create in this session. Never on the trunk.
10. If something outside the task looks wrong, add a BACKLOG row for it. Do not fix it in passing.

## Target crate layout (Phase 1)

```
Cargo.toml                 workspace root (no code)
crates/
  dt-core/                 domain types, Clock, calendar, errors. No I/O, no tokio.
  dt-strategy/             indicators, candlestick patterns, signal generation     → core
  dt-risk/                 risk engine, PDT, limits, sizing                         → core
  dt-engine/               order lifecycle, portfolio, execution pipeline            → core, risk, strategy
  dt-broker/               Broker + MarketDataSource traits, SimBroker               → core
  dt-alpaca/               Alpaca REST + WS implementing dt-broker traits            → core, broker
  dt-store/                sqlx persistence, migrations, repositories                → core
  dt-backtest/             replay runner, loaders, report                            → engine, broker, store
bins/dtctl/                headless CLI: run --paper | backtest | reconcile | fetch
daytrader/                 Tauri + React app (unchanged location)                    → engine (facade)
```

Dependency arrows point downward only. Cargo enforces them; `scripts/verify_architecture.sh` is deleted once the split is complete.

## What replaces the current governance

Keep: root `CLAUDE.md` (trimmed), `BACKLOG.md`, the dangerous-command hook, `/backlog-task`, `/verify-commit`, `/code-review`.
Delete after Phase 1: per-module `CLAUDE.md` boundary files, `verify_architecture.sh`, `architecture_exceptions.txt`, `architecture_violations_baseline.txt` (crates replace them).
Delete in Phase 0: `known_test_issues.txt` escape hatch, `production_safety_baseline.txt` ratchet (replaced by `-D warnings` + clippy lints that forbid `unwrap_used`/`expect_used` in non-test code).
Replace: `plans/*` and `reports/phase-*` → `docs/history/`; add `STATUS.md` that lists what runs end to end and the test that proves each line.
