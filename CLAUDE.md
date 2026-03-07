# DayTrader - AI Agent Governance Rules

## Project
Rust algorithmic daytrading platform. Workspace: root crate + `daytrader/src-tauri`.
Edition 2024, Rust 1.92+, Tokio async runtime, SQLite/SQLx, Tauri 2 frontend.

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
This script runs compilation, linting, tests, architecture checks, and test quality checks.
A commit that has not passed this script is non-compliant. No exceptions.

---

## Module Boundary Declarations

The dependency graph flows strictly downward. Higher layers may depend on lower layers, never the reverse. No circular dependencies.

### Foundation Layer
These modules have no upward dependencies. They are imported by everything above.

- **`utils/`** — ALLOWED: internal (`utils::*`), external crates only. FORBIDDEN: all other `crate::` modules except `brokers::types` (for error context).
- **`brokers/types`**, **`brokers/traits`** — ALLOWED: `utils::errors`. FORBIDDEN: all other modules.
- **`brokers/alpaca/`**, **`brokers/mock`** — ALLOWED: `brokers::types`, `brokers::traits`, `utils::errors`. FORBIDDEN: all other modules.

### Infrastructure Layer
- **`config/`** — ALLOWED: `risk::types` (for config struct fields). FORBIDDEN: `orders`, `positions`, `execution`, `patterns`, `events`, `db`.
- **`events/`** — ALLOWED: `brokers::types` (for shared type re-exports). FORBIDDEN: `risk`, `orders`, `positions`, `execution`, `patterns`, `db`, `config`.
- **`db/`** — ALLOWED: `db::*` (internal), `brokers::types`, `events::types`, `patterns::signals`, `patterns::types`. FORBIDDEN: `risk`, `orders`, `positions`, `execution`.
- **`domain/`** — ALLOWED: `domain::*` (internal). FORBIDDEN: all other modules.
- **`infrastructure/`** — ALLOWED: `db::*`, `domain::*`. FORBIDDEN: `risk`, `orders`, `positions`, `execution`, `patterns`, `events`.

### Business Logic Layer
- **`risk/`** — ALLOWED: `brokers::types`, `config`, `db`, `events`, `risk::*` (internal). FORBIDDEN: `orders`, `positions`, `execution`, `patterns`.
- **`patterns/`** — ALLOWED: `brokers::types`, `events`, `db::repositories`, `patterns::*` (internal). FORBIDDEN: `orders`, `positions`, `execution`, `risk`.
- **`positions/`** — ALLOWED: `brokers::types`, `brokers::traits`, `events`, `orders::types`, `positions::*` (internal). FORBIDDEN: `execution`, `risk`, `patterns`.

### Orchestration Layer (top of tree)
- **`orders/`** — ALLOWED: `brokers::*`, `events`, `risk::engine`, `orders::*` (internal). FORBIDDEN: `execution`, `patterns`.
- **`execution/`** — ALLOWED: `brokers::*`, `events`, `orders`, `positions`, `risk`, `patterns::signals`, `config`, `execution::*` (internal). FORBIDDEN: nothing imports execution — it is the TOP of the dependency tree.

### Boundary Rule
Nothing may depend on `execution/`. If you need to reference execution types from a lower module, that is an architectural violation — use the event bus instead.

Exceptions are documented in `.governance/architecture_exceptions.txt` with justifications.

---

## Project Invariants

These rules MUST always hold. Violations are bugs, not style issues.

### Type Safety
- `#![forbid(unsafe_code)]` must remain in `lib.rs` — never remove or weaken
- All monetary values use `rust_decimal::Decimal`, never raw `f64`
- Domain concepts use newtype wrappers: `Price`, `Quantity`, `Symbol`, `Confidence`, `OrderId`, `PatternName`
- All public functions return `Result<T, E>` with typed errors

### Error Handling
- Library errors use `thiserror`; `anyhow` only at application boundaries (`main.rs`)
- Every error variant has a descriptive message with context
- No `.unwrap()` or `.expect()` in production code paths (only allowed in tests)
- No `panic!()` in production code paths

### Concurrency
- `Arc<T>` for shared ownership across async boundaries
- `DashMap` for concurrent hash maps (established pattern)
- `tokio::sync` for async locks — never `std::sync` (prevents blocking the runtime)
- EventBus broadcast channels — subscribers MUST handle `RecvError::Lagged`

### Naming Conventions
- Modules: `snake_case` directories and files
- Types: `PascalCase`, descriptive (`PositionTracker`, not `PosTrack`)
- Functions: `snake_case`, verb-first (`calculate_pnl`, not `pnl_calc`)
- Constants: `SCREAMING_SNAKE_CASE`
- Test functions: `test_<what>_<condition>_<expected>` (e.g., `test_position_sizing_exceeds_limit_returns_error`)

### Security (Financial Trading)
- API keys never in source code — use `keyring` crate or environment variables
- All broker communication over TLS (`rustls`)
- PDT compliance checks MUST NEVER be bypassed or disabled
- Risk limits validated BEFORE order submission (pre-trade, not post-trade)
- Never commit config files containing credentials
- All external inputs validated at the boundary before entering business logic

### Testing Rules
- Every new public function MUST have at least one test
- Tests MUST make meaningful assertions about actual behavior
- `assert!(true)` and `assert!(false, "not implemented")` are FORBIDDEN in new code
- Property-based tests (`proptest`) required for pure functions with numeric inputs
- Integration tests use real types, not just mocks
- Performance tests include hard threshold assertions

---

## Agent Session Rules

### Before Starting Work
1. Read `BACKLOG.md` and identify your task (or create a new one)
2. Update `BACKLOG.md` to mark the task "In Progress" with the current date
3. Read the relevant plan document in `plans/` if one exists
4. Create a branch: `feat/<scope>`, `fix/<scope>`, `refactor/<scope>`, `test/<scope>`, or `docs/<scope>`

### During Work
1. One logical task per session — do not mix unrelated changes
2. Run `cargo check` after every significant code change
3. If you modify a module, verify its tests pass: `cargo test --lib -- <module_name>`
4. Do NOT refactor code outside your task scope
5. Do NOT add dependencies without justification
6. Do NOT modify module boundaries or public interfaces without explicit approval

### Before Committing
1. Run `./scripts/verify_agent_work.sh` — MUST pass completely
2. Update `BACKLOG.md` to mark your task "Done" with a summary of changes
3. Update `plans/progress-tracking.md` if completion percentages changed
4. Write a clear commit message describing what changed and why

### Commit Message Format
```
type(scope): description

Types: feat, fix, docs, style, refactor, test, chore, perf, ci, revert
Scope: module name (e.g., risk, orders, patterns)
Max 72 chars in subject line, lowercase, no trailing period
```

### Accountability
- Your work will be verified by `verify_agent_work.sh` which checks: compilation, linting, test passage, architecture boundaries, and test quality
- If you claim tests pass, they must actually pass when the script runs them
- If you claim you added tests, `verify_test_quality.sh` will check they make real assertions
- If you claim you respected module boundaries, `verify_architecture.sh` will check imports
- All claims are machine-verified. Follow through is not optional.
