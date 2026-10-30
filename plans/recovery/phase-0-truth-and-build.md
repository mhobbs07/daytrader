# Phase 0: Truth and Build

Goal: a clean clone of the trunk builds, lints, tests and documents green in CI, with no excused failures, and the repository's documents describe what actually runs. Nothing else lands until this phase exits.

Baseline measured 2026-10-30 on `staging`: library compiles with 691 warnings when `DATABASE_URL` is provisioned; 323 unit tests and all integration suites pass; `cargo clippy -- -D warnings` = 1,792 errors; 3 bench targets do not compile; 27 tests `#[ignore]`d; CI provisions no database and triggers only on `main`/`develop`.

Order: B-086 and B-087 first (they change where PRs go). B-088 and B-089 next (they make CI meaningful). The rest in any order; B-090–B-094 can run in parallel, one agent per module group, because they touch disjoint files.

---

## B-086 Consolidate to one trunk — **human**

**Goal:** `main` is the only long-lived branch and contains everything on `staging`.
**Files:** none (GitHub settings and git operations).
**Steps:**
1. Merge [#29](https://github.com/mhobbs07/daytrader/pull/29) and its dependents into `staging` first (see merge order in README).
2. `main` is an ancestor of `staging` (verified 2026-10-30), so open a PR `staging → main` and merge it with "Rebase and merge" or "Create a merge commit" (either is fine; no squash, to keep history).
3. In GitHub settings: default branch = `main`; branch protection on `main`: require a PR, no force-push, no deletion, include administrators. **Do not require status checks yet.** CI is red until B-088 to B-094 land, so requiring `test`/`docs`/`governance` now would block every PR, including the ones that make CI green. Add the required checks as the last step of B-089, once CI is green on `main`.
4. Delete `staging`, `staging-01`, and every branch whose PR is merged or closed. Dry run first:
   ```bash
   git fetch --prune
   git branch -r --merged origin/main | grep -vE 'origin/(main|HEAD)' 
   ```
   Branches not merged but abandoned (all `convoy/*`, `gt/*`, `claude/strange-dubinsky`, `claude/interesting-neumann`, `claude/epic-grothendieck-*`, `claude/check-last-commit-*`, `feat/staging-test-unblock`, `docs/backlog-*`, `chore/remove-branch-naming-workflow`) are deleted explicitly after a glance at `git log origin/main..<branch> --oneline`.
5. Close [#6](https://github.com/mhobbs07/daytrader/pull/6) with a note that B-057's scope was absorbed.
**Proof:** `git ls-remote --heads origin | wc -l` ≤ 5; `gh pr list` shows only live work.
**Exit:** `main` is default and protected; `staging` does not exist.
**Size:** S (no code).

## B-087 Update governance for one trunk — agent

**Goal:** Every document, rule, skill and workflow refers to `main` as the trunk and nothing refers to `staging`.
**Files:** `CLAUDE.md`, `AGENTS.md`, `.claude/rules/branching.md`, `.claude/skills/promote-to-staging/**` (rename to `promote`), `.claude/skills/backlog-task/**`, `.claude/skills/swarm/**`, `.claude/skills/feature-branch/**`, `.github/workflows/branch-protection.yml`, `README.md`, `docs/developer-onboarding.md`, `plans/git-workflow-*.md` (move to `docs/history/`).
**Steps:** replace the `main ← staging ← feat/*` model with `main ← <type>/<id>-<slug>`; PRs target `main`; delete `/promote-to-staging`, add `/promote` that verifies, pushes and opens a PR to `main`; keep "never commit directly to the trunk" and "never force-push the trunk".
**Proof:** `grep -rn "staging" CLAUDE.md AGENTS.md .claude README.md docs/ --include=*.md | grep -v history` prints nothing.
**Exit:** as Proof.
**Size:** M.

## B-088 Make the sqlx build hermetic — agent

**Goal:** `cargo check`, `clippy`, `test`, `doc` succeed on a clean clone with no database and no `sqlite3` CLI.
**Files:** `.gitignore`, `.sqlx/**` (new, committed), `.env.example`, `scripts/setup_db.sh`, `scripts/verify_agent_work.sh` (lines 30–45 only), `Dockerfile`, `docs/developer-onboarding.md` (DATABASE_URL section), `src/db/connection.rs` (one change: `create_if_missing(true)` via `SqliteConnectOptions`).
**Steps:**
1. `cargo install sqlx-cli --no-default-features --features sqlite`.
2. `scripts/setup_db.sh` provisions via `sqlx database create` + applies `src/db/schema.sql` and the `schema_migrations` DDL using `sqlx` (no `sqlite3` CLI). Fail loudly if it cannot; never `|| true`.
3. `cargo sqlx prepare --workspace` → commit `.sqlx/`. Remove `.sqlx/` from `.gitignore`.
4. `SQLX_OFFLINE=true` is the default in `.env.example`, in `verify_agent_work.sh`, in the Dockerfile, and in CI (B-089). The verify script runs `cargo sqlx prepare --check` so a stale cache fails the pipeline.
5. `Database::new` sets `create_if_missing(true)` and applies pragmas through `SqliteConnectOptions` (`journal_mode`, `synchronous`, `busy_timeout`) so they apply to every pooled connection. Fix `cache_size` to `-64000` (64 MiB). This is the only production code change in this task.
6. Dockerfile: copy the whole workspace (it includes `daytrader/src-tauri`), use `SQLX_OFFLINE=true`, stop copying `schema.sql` into the image.
**Proof:** in a fresh clone with no `DATABASE_URL`: `SQLX_OFFLINE=true cargo check --workspace --all-targets` succeeds; `cargo sqlx prepare --check` succeeds; `cargo test --lib db::` passes (tests that need a file DB now create it).
**Exit:** as Proof. `Cargo.lock` is committed (remove from `.gitignore`).
**Size:** M.

## B-089 CI mirrors the verify script and runs on every PR — agent (CI change; owner has approved this program)

**Goal:** One workflow, triggered on PRs to the trunk and pushes to it, that runs exactly the steps the local verify script runs, on the whole workspace including the Tauri crate, with no excused failures.
**Files:** `.github/workflows/ci.yml`, `.github/workflows/docs.yml` (delete; fold into ci), `.gitlab-ci.yml` (delete), `scripts/verify_agent_work.sh`, `.governance/known_test_issues.txt` (delete), `deny.toml` (new). `.governance/production_safety_baseline.txt` stays as the ratchet until B-094 deletes it.
**Steps:**
1. `ci.yml` `on:` = `pull_request:` (all branches) + `push: branches: [main]`. Remove `develop`.
2. Jobs: `fmt` (`cargo fmt --all -- --check`), `clippy` (`cargo clippy --workspace --all-targets`, warn-only per step 5 until B-094 flips it), `test` (`cargo test --workspace`, then `cargo test --workspace --features mock`), `doc` (`RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps`), `tauri-check` (`apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev ...` then `cargo check -p daytrader-app`), `frontend` (`npm ci && npm run build` in `daytrader/`), `governance` (`verify_architecture.sh`, `verify_test_quality.sh` until Phase 1 deletes them). `SQLX_OFFLINE=true` in `env:` at the top.
3. Remove `cargo test --test integration` (the target does not exist). Remove `continue-on-error` everywhere except the optional coverage upload.
4. Make `verify_agent_work.sh` call the same commands in the same order. Delete the "known failures ≤ N" branch (lines ~112–145) and the `.governance/known_test_issues.txt` file; the 27 `#[ignore]` tests are tracked by B-081 and must either pass or be deleted with a reason in the PR.
5. Clippy stays warn-only locally **until** B-090–B-094 land, then B-094's last step flips both CI and local to `-D warnings` together.
**Proof:** a PR touching only a comment gets a full green run on every job. (The "one `unused variable` goes red in `clippy`" proof belongs to B-094, which flips clippy to `-D warnings`; a warn-only clippy job cannot pass it.)
**Exit:** as Proof.
**Size:** M.

**Added by the plan merge (roadmap R0.5, B-206):** CI also runs `cargo sqlx prepare --check` (the local script already does, since B-088) and `cargo deny check` (advisories and licences, configured by `deny.toml`). An advisory that only a version upgrade fixes is listed in `deny.toml` with the B-208 upgrade that removes it, so this PR can go green before the upgrades run. Since #39 the verify script matches known failures by test name instead of count; this task still deletes the escape hatch entirely.

Two R0.5 items are scheduled elsewhere so that no PR goes red on work another task owns:
- **`cargo machete` (unused dependencies)** becomes a CI gate in **B-095**, the PR that removes the dependencies it would flag.
- **The `std::sync` hard zero** (`tokio::sync::RwLock` where a critical section awaits, `std::sync::OnceLock`/`LazyLock` for initialise-once data such as `utils/holidays.rs`) is done in **B-094**, which already deletes the production-safety ratchet.

## B-090 … B-094 Clippy to zero under `-D warnings` — agent, one PR per module group, parallelisable

**Goal:** `cargo clippy --workspace --all-targets -- -D warnings` is clean.
**Policy (decided):** `#![warn(clippy::pedantic)]` stays. Lints are fixed, not allowed, except these crate-level allows with a one-line justification each in `lib.rs`: `clippy::module_name_repetitions`, `clippy::must_use_candidate`, `clippy::missing_errors_doc`, `clippy::missing_panics_doc`. Anything else that seems to need an `allow` goes in the PR description for the reviewer to accept or reject. Add `clippy::unwrap_used` and `clippy::expect_used` as **deny** for non-test code (`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`) in B-094 and fix what that surfaces; this replaces `production_safety_baseline.txt`.

| ID | Files | Measure first |
|---|---|---|
| B-090 | `src/utils/**`, `src/brokers/**`, `src/config/**`, `src/data/**` | `cargo clippy --lib -- -D warnings 2>&1 \| grep -c "src/utils\|src/brokers\|src/config\|src/data"` |
| B-091 | `src/events/**`, `src/db/**` | same pattern |
| B-092 | `src/patterns/**` | same |
| B-093 | `src/risk/**` | same |
| B-094 | `src/orders/**`, `src/positions/**`, `src/execution/**`, `src/lib.rs`, `src/main.rs`, `daytrader/src-tauri/src/**`, `tests/**`, `benches/**`, `examples/**`; then flip CI + verify script to `-D warnings`. Also the `std::sync` hard zero: `src/utils/holidays.rs`, `daytrader/src-tauri/src/{state,events,main}.rs`, `clippy.toml` (new), `scripts/verify_agent_work.sh`, and delete `.governance/production_safety_baseline.txt` | same, then full. The `std::sync` zero is enforced by clippy, not grep: `clippy.toml` sets `disallowed-types = ["std::sync::Mutex", "std::sync::RwLock"]`, which resolves grouped (`use std::sync::{Arc, RwLock}`) and aliased imports. Prove it by adding one `std::sync::Mutex` locally and seeing `clippy::disallowed_types` fail |

**Rules:** behaviour-preserving only; if a lint reveals a real bug (e.g. an unused `Result`), do not fix the bug here: add a BACKLOG row referencing Phase 2 and apply the minimal `let _ =` with a `// B-1xx` comment. No new `allow` attributes on items without a PR-description justification.
**Proof:** the group's grep count is 0; `cargo test --workspace` unchanged (same pass count as before the PR). B-094 additionally proves the `-D warnings` flip it performs (moved here from B-089): a PR introducing one `unused variable` goes red in `clippy`, in CI and in `verify_agent_work.sh`.
**Size:** M each (split further if a group exceeds 300 lines; `src/risk` probably will: split by file).

## B-095 Delete dead code and unused dependencies — agent

**Goal:** Nothing in the tree is unreachable from a binary, a test, or a public API that something uses.
**Files:** `Cargo.toml`, `src/utils/retry.rs` (delete), `src/utils/mod.rs`, `src/patterns/streaming.rs` (`SignalStreamingPipeline` and its tests; also its `num_cpus` use), `src/db/migrations.rs` (delete), `migrations/` (delete), `src/db/repositories.rs` (`OrderRepository`, `update_order`), `src/db/mod.rs`, `src/orders/types.rs` (`OrderEvent`, `OrderMetrics`), `src/utils/errors.rs` (`rusqlite` variant), `tests/database_integration.rs` (delete; it tests nothing real), `src/brokers/mock.rs` (`parking_lot` → `tokio::sync`), `src/utils/logging.rs`, `src/utils/metrics.rs`, `src/utils/error_context.rs` (`lazy_static` → `LazyLock`), `src/brokers/alpaca/websocket.rs` (`log` → `tracing`, `futures-util` → `futures`), `src/risk/rules.rs` (`futures-util` → `futures`), `src/utils/performance_monitor.rs` and `src/utils/stats.rs` (new; the `statistical` replacement), `.github/workflows/ci.yml` and `scripts/verify_agent_work.sh` (the `cargo machete` gate, added to both so local verification and CI run the same steps).
**Dependencies to remove** (verified zero uses 2026-10-30): `crossbeam`, `tower`, `keyring`, `once_cell`, `sha2`, `url`, `tracing-appender`, `metrics-exporter-prometheus`, `ta`, `tokio-retry`, `rusqlite`, `parking_lot`, `lazy_static` (replace 3 uses with `std::sync::LazyLock`), `log` (switch `websocket.rs` to `tracing`), `futures-util` (use `futures`).
**Also from roadmap R0.6 (B-208):** replace `lazy_static` with `LazyLock`, `num_cpus` with `std::thread::available_parallelism`, and move the two `statistical` uses into `src/utils/stats.rs` (Phase 4 needs it for metrics). Version upgrades (reqwest, tokio-tungstenite, thiserror 2, sqlx 0.9, rand, metrics) are **not** part of B-095; they stay B-208 and run after Phase 0 exits, each as its own PR.
**Decision needed in the PR:** `src/data/` is unused. Keep it only if Phase 3 will adopt `DataIngestionPipeline` as the market-data ingress; otherwise delete. Recommendation: keep `validation.rs`, delete `cache.rs` and `storage.rs` (duplicates), decide `ingestion.rs` in Phase 3.
**Proof:** `cargo machete` reports nothing, and both CI and `verify_agent_work.sh` now run it as a gate; `cargo test --workspace` passes; `cargo build --release` passes.
**Size:** M (deletions).

## B-096 Fix or delete the benches — agent — **Done** (B-221, landed by #39)

**Goal:** `cargo bench --no-run` compiles.
**Files:** `benches/*.rs`, `Cargo.toml` `[[bench]]` entries.
**Steps:** update to the newtype APIs (`Price::new(..)?`, `Quantity::new(..)?`, current `ManagedOrder`/`FillInfo` fields). Delete `phase_3d_benchmarks.rs` if it benchmarks deleted code. No new `unwrap` outside `#[cfg(test)]`/bench setup helpers (benches may `expect` in setup with a message).
**Proof:** `cargo bench --no-run` exits 0; `cargo clippy --benches -- -D warnings` clean.
**Size:** S–M.

## B-097 Fix `config/default.toml` so it loads completely — agent — **partly done** by B-205 (#39): `RawRiskConfig` → `TryFrom` → `RiskConfig` rejects invalid and all-zero limits. Still open: the `risk.*` nesting of `pattern_profiles` / `confidence_multipliers` / `pattern_performance_thresholds`, `deny_unknown_fields` on the top-level config, and count assertions in `config_validation`

**Goal:** `AppConfig::load_default()` succeeds and `risk.pattern_profiles`, `risk.confidence_multipliers`, `risk.pattern_performance_thresholds` are populated from the file.
**Files:** `config/default.toml`, `src/config/mod.rs`, `tests/config_validation.rs`.
**Steps:** move the three blocks under `[risk.*]` (or add `#[serde(flatten)]`), add `#[serde(default)]` to credentials (already on CTL), add struct fields for `broker.base_url`, `broker.ws_url`, `database.enable_wal` or delete those keys; add `deny_unknown_fields` to the top-level config so silent drops become load errors. Make `test_load_default_config` assert `profiles.len() == 6` and `confidence_multipliers.len() == 5` instead of swallowing `Err`.
**Proof:** `cargo test --test config_validation` passes with the strengthened assertions.
**Size:** S.

## B-098 Documents tell the truth — agent

**Goal:** A newcomer (or an agent) reading the repo learns what runs, not what was planned.
**Files:** `plans/**` → `docs/history/plans/`, `reports/phase-*` and `reports/project-strengths-*` → `docs/history/reports/`, new `STATUS.md`, `README.md` (Rust 1.92 / edition 2024, actual quick start), `docs/developer-onboarding.md`, `docs/faq.md`, `IMPORTANT-CARGO-FIX.md` (delete), `scripts/cargo.sh` (delete), `docker-socket-fix.sh` (delete), `AGENTS.md` (point at `plans/recovery/`).
**`STATUS.md` format:** a table with columns `Capability | Runs end to end? | Proven by (test or command) | Notes`. First version lists: unit tests, integration tests, Tauri app with mock data, backtest replay (after #29), paper trading (no), live (no).
**Proof:** every row in `STATUS.md` names a test that exists (`grep -q "fn <name>"`) or a command that was run in the PR.
**Size:** M (moves + two new files).

---

## Phase 0 exit checklist

- [x] B-086 trunk consolidated, `staging` deleted
- [x] B-087 no references to `staging` outside history
- [x] B-088 clean clone builds with `SQLX_OFFLINE=true`, `.sqlx/` and `Cargo.lock` committed
- [ ] B-089 CI runs on every PR; local script == CI; no excused failures; `cargo deny` and `cargo machete` in CI; `std::sync` count zero
- [ ] B-090–B-094 `cargo clippy --workspace --all-targets -- -D warnings` clean; `unwrap_used`/`expect_used` denied in non-test code
- [ ] B-095 dead code and 15 unused dependencies gone
- [x] B-096 benches compile (B-221)
- [ ] B-097 config loads completely
- [ ] B-098 `STATUS.md` exists; old plans archived
- [ ] B-081 (existing) 27 ignored tests pass or are deleted with reasons
