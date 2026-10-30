# DayTrader — Agent Instructions

This file provides instructions for any AI coding agent working in this repository.
It is compatible with Claude Code, Codex, Cursor, and other tools that support AGENTS.md.

## Project

Rust algorithmic daytrading platform.
- Edition 2024, Rust 1.92+, Tokio async runtime, SQLite/SQLx, Tauri 2 frontend
- Workspace: root crate + `daytrader/src-tauri`

## Critical Rules

1. **Run `./scripts/verify_agent_work.sh` before ANY commit.** No exceptions.
2. **Respect module boundaries.** Each `src/<module>/` has a CLAUDE.md declaring allowed/forbidden imports. The dependency graph flows downward only.
3. **Nothing may import from `execution/`.** It is the top of the dependency tree.
4. **All monetary values use `rust_decimal::Decimal`**, never `f64`.
5. **Domain concepts use newtypes**: `Price`, `Quantity`, `Symbol`, `Confidence`, `OrderId`, `PatternName`.
6. **No `.unwrap()` / `.expect()` / `panic!()` in production code.**
7. **`#![forbid(unsafe_code)]`** is set in `lib.rs` — do not add unsafe blocks.
8. **PDT compliance checks must never be bypassed.** This is SEC-mandated.
9. **API keys never in source code.** Use `keyring` crate or environment variables.
10. **Every new public function must have at least one test** with meaningful assertions.

## Architecture Layers

```
execution/          TOP — orchestrates full trading loop (nothing imports this)
orders/             Order lifecycle, risk-validated submission
risk/               Risk engine, PDT compliance
patterns/           Pattern recognition, signal generation
positions/          Position tracking, P&L
config/             Application configuration
events/             Event bus (tokio broadcast)
db/                 SQLite persistence (SQLx)
domain/             Pure domain models (fully isolated)
infrastructure/     Repository implementations
utils/              Shared utilities, error types
brokers/            Broker abstraction (Alpaca, Mock)
```

## Build & Verify

```bash
cargo check                          # Type check
cargo test --lib                     # Unit tests
cargo clippy -- -D warnings          # Lint
cargo fmt -- --check                 # Format check
./scripts/verify_agent_work.sh       # Full pipeline (REQUIRED before commit)
./scripts/verify_architecture.sh     # Module boundary check
```

## Commit Format

```
type(scope): description
```
Types: feat, fix, docs, refactor, test, chore, perf. Max 72 chars subject line.

## Git Workflow

One trunk: `main` is the only long-lived branch. Cut a `<type>/<id>-<slug>` branch from `main` for each BACKLOG task, open a PR that targets `main`, and merge it only when CI is green. Never commit to `main` directly and never force-push it. Full policy: `.claude/rules/branching.md`.

## For Claude Code Users

See `CLAUDE.md` for full governance rules, hooks, skills, and swarm orchestration.
See `.claude/` directory for hooks, rules, and skill definitions.
