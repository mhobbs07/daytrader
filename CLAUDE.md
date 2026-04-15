# DayTrader - AI Agent Governance

## Project
Rust algorithmic daytrading platform. Workspace: root crate + `daytrader/src-tauri`.
Edition 2024, Rust 1.92+, Tokio async runtime, SQLite/SQLx, Tauri 2 frontend.

## Governance System

This project uses a layered Claude Code governance system:

| Layer | Location | Purpose |
|-------|----------|---------|
| **Hooks** | `.claude/settings.json` | Pre/post tool enforcement (boundary guard, quality checks, command safety) |
| **Hook Scripts** | `.claude/hooks/` | Shell scripts called by hooks (architecture guard, dangerous command blocker, post-edit quality check, session guardrails) |
| **Path-Scoped Rules** | `.claude/rules/` | Rules activated when working in matching paths (type-safety, concurrency, testing, security, coding-style, architecture) |
| **Module CLAUDE.md** | `src/<module>/CLAUDE.md` | Per-module dependency rules — loaded on-demand when reading files in that module |
| **Skills** | `.claude/skills/` | Scoped agent workflows that persist until complete (`/verify-commit`, `/fix-module`, `/architecture-audit`, `/backlog-task`) |
| **Verification Scripts** | `scripts/` | `verify_agent_work.sh` (master pipeline), `verify_architecture.sh`, `verify_test_quality.sh` |
| **Exceptions** | `.governance/` | `architecture_exceptions.txt`, `known_test_issues.txt` |

## Build Commands
```
cargo build                          # Full build
cargo check                          # Type check only
cargo test --lib                     # Unit tests
cargo test                           # All tests
cargo test --test '*'                # Integration tests only
cargo fmt -- --check                 # Format check
cargo clippy -- -D warnings          # Lint (warnings = errors)
cargo bench                          # Benchmarks
cargo audit                          # Security audit
cargo tarpaulin --out Xml            # Coverage report
```

## Required Verification
Before ANY commit, you MUST run:
```
./scripts/verify_agent_work.sh
```
This script runs: format check, compilation, linting, unit tests, integration tests, docs, architecture boundaries, test quality, and backlog consistency.
A commit that has not passed this script is non-compliant. No exceptions.

Use `/verify-commit` to run the pipeline and auto-fix issues until it passes.

---

## Architecture Overview

The dependency graph flows strictly downward. Higher layers depend on lower layers, never the reverse.

```
TOP OF TREE (nothing imports this)
  execution/          Orchestrates the full trading loop

ORCHESTRATION
  orders/             Order lifecycle, risk-validated submission

BUSINESS LOGIC
  risk/               Risk engine, PDT compliance, position limits
  patterns/           Pattern recognition, signal generation
  positions/          Position tracking, P&L calculation

INFRASTRUCTURE
  config/             Application configuration
  events/             Event bus (tokio broadcast channels)
  db/                 SQLite persistence via SQLx
  domain/             Pure domain models (fully isolated)
  infrastructure/     Repository implementations (db <-> domain bridge)

FOUNDATION (no upward dependencies)
  utils/              Shared utilities, error types
  brokers/            Broker abstraction (Alpaca, Mock)
```

Each module has a `CLAUDE.md` in its directory with exact allowed/forbidden imports.
Approved exceptions are documented in `.governance/architecture_exceptions.txt`.
The `guard-module-boundaries.sh` hook blocks forbidden imports at edit time.

---

## Project Invariants

These are enforced by hooks, rules, and verification scripts. See `.claude/rules/` for full details.

- **Type safety**: `#![forbid(unsafe_code)]`, `Decimal` for money, newtypes for domain concepts, typed errors on all public functions
- **Error handling**: `thiserror` for library errors, no `.unwrap()`/`.expect()`/`panic!()` in production
- **Concurrency**: `Arc<T>`, `DashMap`, `tokio::sync` only (never `std::sync`), handle `RecvError::Lagged`
- **Security**: No hardcoded credentials, TLS for broker communication, PDT compliance never bypassed, risk checks before order submission
- **Testing**: Every public function tested, meaningful assertions only, property-based tests for numeric functions

---

## Agent Session Rules

### Before Starting Work
1. Read `BACKLOG.md` and identify your task (or use `/backlog-task next`)
2. Update `BACKLOG.md` to mark the task "In Progress" with the current date
3. Read the relevant plan document in `plans/` if one exists
4. Read the `CLAUDE.md` in every module you plan to touch

### During Work
1. One logical task per session — do not mix unrelated changes
2. Run `cargo check` after every significant code change
3. If you modify a module, verify its tests pass: `cargo test --lib -- <module_name>`
4. Do NOT refactor code outside your task scope
5. Do NOT add dependencies without justification
6. Do NOT modify module boundaries or public interfaces without explicit approval

### Before Committing
1. Run `/verify-commit` or `./scripts/verify_agent_work.sh` — MUST pass completely
2. Update `BACKLOG.md` to mark your task "Done" with a summary of changes
3. Update `plans/progress-tracking.md` if completion percentages changed

### Commit Message Format
```
type(scope): description

Types: feat, fix, docs, style, refactor, test, chore, perf, ci, revert
Scope: module name (e.g., risk, orders, patterns)
Max 72 chars in subject line, lowercase, no trailing period
```

### Available Skills
- `/verify-commit` — Run verification pipeline, auto-fix until green
- `/fix-module <name>` — Fix all issues in a specific module until clean
- `/architecture-audit` — Audit boundaries, report violations, optionally fix
- `/backlog-task <id|next>` — Pick up and complete a backlog task end-to-end
- `/swarm <feature> <description>` — Orchestrate parallel worktree agents across layers
- `/promote-to-staging <branch> [--auto-merge]` — Verify, push, and PR a feature branch to staging
- `/code-review [branch|PR|--staged]` — Review changes for architecture, safety, and compliance
- `/api-contract <feature> [--generate|--verify]` — Define/verify shared type contracts between layers

### Accountability
- Hooks enforce boundaries and quality checks in real-time as you edit
- `verify_agent_work.sh` is the final gate — all checks must pass
- All claims are machine-verified. Follow through is not optional.

---

## Worktree Swarm System

For features that cut across multiple architectural layers (db + backend + frontend), use `/swarm` to spawn parallel agents in isolated worktrees on a shared feature branch.

### How It Works
1. Orchestrator creates `feat/<feature-name>` branch
2. Spawns 2-4 agents, each in its own worktree (via `isolation: "worktree"`)
3. Each agent works in isolation — no merge conflicts during development
4. Agents commit their work to their worktree branches
5. Orchestrator merges all branches back to the feature branch
6. Integration verification runs on the merged result

### Model Budget Policy

Costs are controlled by assigning models based on task complexity:

| Agent Role | Model | Cost Tier |
|------------|-------|-----------|
| Orchestrator | opus | High (only for coordination) |
| Backend / DB agents | sonnet | Medium |
| Frontend / Test agents | haiku | Low |

See `.claude/rules/agent-models.md` for full policy. Maximum 4 concurrent agents.

### Worktree Cleanup
```
.claude/hooks/swarm-worktree-cleanup.sh --dry-run   # List stale worktrees
.claude/hooks/swarm-worktree-cleanup.sh --clean      # Remove stale worktrees
```

---

## Git Branching & Staging Workflow

```
main              ← production (protected, PR + review required)
  └─ staging      ← integration testing (PRs from feature branches)
       └─ feat/*  ← feature work (from /swarm or direct development)
            └─ worktree branches  ← parallel agent work (merged to feat/)
```

### Flow
1. Work happens on `feat/<name>` branches (created by `/backlog-task` or `/swarm`)
2. When a feature is ready, use `/promote-to-staging <branch>` to:
   - Run full verification
   - Rebase on staging
   - Push and create a PR targeting `staging`
   - Optionally auto-merge with `--auto-merge`
3. `staging -> main` promotion is a human decision after integration testing

### Rules
- Never commit directly to `main` or `staging`
- Feature branches target `staging`, not `main`
- Never force-push to `main` or `staging`
- See `.claude/rules/branching.md` for full policy
