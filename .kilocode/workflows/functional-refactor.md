---
name: functional-refactor
version: 1.0
description: Full functional-paradigm refactor of a module
steps:
  - audit-primitives
  - refactor-to-newtype
  - refactor-loop-to-iterator
  - add-typed-error
  - add-trait-boundary
  - verify
---

# Workflow: Functional Refactor

End-to-end workflow for bringing a module into compliance with coding standards. Run each step in order. Do not skip steps — each builds on the previous.

---

## Step 1 — Audit

Identify all non-compliant patterns in the target module before touching any code.

```bash
# Find primitive fields used as domain values
grep -n 'f64\|u32\|i64\|String' src/<module>/types.rs

# Find imperative for-loops
grep -n 'for ' src/<module>/*.rs

# Find dynamic dispatch (must be justified)
grep -n 'Box<dyn' src/<module>/*.rs

# Find panic sites
grep -n 'unwrap()\|expect(' src/<module>/*.rs

# Find untyped error returns
grep -n 'Box<dyn.*Error\|Result<.*String' src/<module>/*.rs
```

Record findings grouped by category:
- **Primitive fields**: `<file>:<line>` — `<field_name>: <type>`
- **For-loops**: `<file>:<line>` — brief description of what the loop does
- **Dynamic dispatch**: `<file>:<line>` — whether justified or not
- **Panic sites**: `<file>:<line>` — in production code or test?
- **Untyped errors**: `<file>:<line>` — current return type

---

## Step 2 — Newtypes

For each primitive field identified in Step 1, invoke the `refactor-to-newtype` skill.

**Order matters**: Start with the most foundational types (used by the most other types) and work outward. Fixing a shared type like `Percentage` or `DollarAmount` in `src/risk/types.rs` propagates improvements to all modules that import it.

**Check first**: Before defining a new newtype, check `src/brokers/types.rs` and `src/risk/types.rs` for existing newtypes. Common ones already defined:
- `Price`, `Quantity`, `Symbol`, `Confidence`, `OrderId`, `PatternName` — in `src/brokers/types.rs`
- `DollarAmount`, `Percentage`, `Count`, `AccountId` — in `src/risk/types.rs`

Apply `refactor-to-newtype` skill for each field not already covered.

---

## Step 3 — Iterators

For each for-loop identified in Step 1, invoke the `refactor-loop-to-iterator` skill.

**Priority order**:
1. Loops building a `Vec` — highest impact, simplest transformation
2. Loops computing a scalar aggregate — replace with `.fold()`, `.sum()`, `.count()`
3. Loops with early returns — replace with `.find()`, `.any()`, `.all()`
4. Stateful loops (EMA, rolling averages) — replace with `.scan()`
5. Date/calendar while-loops — replace with `(0..).map(...).take_while(...).filter(...)` or delegate to existing calendar methods

Run `cargo check` after each transformation. Do not batch all loops and check once — catch errors one at a time.

---

## Step 4 — Errors

For each `Box<dyn Error>` or `Result<_, String>` identified in Step 1, invoke the `add-typed-error` skill.

**Sequencing**:
- Fix errors in lower-layer modules first (`db/`, `risk/engine.rs`) before fixing call sites in higher layers
- A single typed error enum (`RiskEngineError`, `RepositoryError`) may cover multiple methods — define it once, apply to all

After applying the skill, verify there are zero remaining occurrences:
```bash
grep -n 'Box<dyn.*Error\|Result<.*String' src/<module>/*.rs
```

---

## Step 5 — Trait Bounds

For each concrete type in a public API identified in Step 1, invoke the `add-trait-boundary` skill.

**When to apply**:
- Public `fn` parameters that accept a concrete struct where a trait would decouple the caller
- Struct fields storing a concrete type that could be stored as `Arc<dyn Trait>` for testability
- `impl Trait` in return position to avoid exposing concrete types

**When NOT to apply** (document the exception):
- `Arc<dyn PatternDetector>` in `PatternRegistry` — already correct, dynamic dispatch justified for runtime-registered patterns
- Internal helpers not part of public API — trait abstraction adds complexity without benefit

---

## Step 6 — Verify

Run the full verification script. All checks must pass before committing.

```bash
./scripts/verify_agent_work.sh
```

This runs:
- `cargo check` — compilation
- `cargo clippy -- -D warnings` — linting (warnings are errors)
- `cargo test --lib` — unit tests
- `cargo fmt -- --check` — formatting
- Architecture boundary checks
- Test quality checks (no `assert!(true)` or stub assertions)

**If any check fails**: Fix the issue before proceeding. Do not commit with failing checks.

---

## Step 7 — Update Backlog

Invoke the `update-backlog` skill for the affected component.

Move each completed refactoring task (RF-xxx ID) from Pending/Active to Done in the relevant `docs/backlogs/<component>.md` file. Update the `progress` and `last_updated` frontmatter fields.

Commit the backlog update in the same commit as the code change.
