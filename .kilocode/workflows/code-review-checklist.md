---
name: code-review-checklist
version: 1.0
description: Checklist for reviewing any code change in DayTrader
trigger: before-commit
---

# Workflow: Code Review Checklist

Ordered checklist for reviewing any code change in DayTrader. Work through items top to bottom. Each item references the relevant rule in `.kilocode/coding-standards.md`. A change is not ready to commit until all items are checked.

---

## Type Safety

- [ ] No `f64`/`u32`/`String` in public struct fields or function signatures for domain values
  - Domain values: prices, quantities, percentages, dollar amounts, symbols, confidence scores, latency measurements
  - Exception: `HashMap<String, f64>` for config-driven arbitrary parameters is acceptable — must have a comment documenting the exception
  - Exception: SQLx projection types in `domain/models.rs` use primitives for row mapping — must have the module-level doc comment
  - Check: `grep -n 'pub.*: f64\|pub.*: u32\|pub.*: String' src/<changed_file>.rs`

- [ ] No `unwrap()` or `expect()` outside `#[cfg(test)]`
  - In production code paths, propagate errors with `?` or map them to typed variants
  - `expect()` is permitted in `main.rs` startup paths where a panic is the correct failure mode — must have a comment explaining why
  - Check: `grep -n 'unwrap()\|expect(' src/<changed_file>.rs`

- [ ] No `Box<dyn std::error::Error>` in public API signatures
  - Replace with a typed `thiserror` enum variant
  - `Box<dyn Error>` in private internal helpers is a warning, not a hard block — document the tech debt and create a backlog item

- [ ] No `Result<_, String>` anywhere
  - String errors lose type information and cannot be matched on by callers
  - Replace with a typed error variant: `Result<T, ModuleError>`

---

## Functional Paradigms

- [ ] All `for` loops pushing to `Vec` replaced with `.map().collect()`
  - Applies to: `for x in items { vec.push(transform(x)); }`
  - Exception: async loops where `.map().collect()` cannot be used without `futures::future::join_all` — document why the loop is kept

- [ ] All `match` arms are exhaustive (no unnecessary `_` catch-all)
  - A `_` arm that silently ignores variants hides bugs when new variants are added
  - If a catch-all is genuinely needed, add a comment explaining which variants it covers and why they are safe to group

- [ ] All `pub fn` use trait bounds or `impl Trait`, not concrete types, where applicable
  - `fn new(bus: EventBus)` → `fn new(bus: impl EventPublisher)` enables testing without a real bus
  - Exception: if there is only one implementation and none is anticipated, concrete type is acceptable — document this

---

## Error Handling

- [ ] New error variants added to appropriate error enum with `thiserror`
  - New variants go in the module's error enum (e.g., `RiskEngineError`, `RepositoryError`)
  - The module error enum must be a variant of `AppError` via `#[from]`
  - Variant messages must be descriptive: `#[error("Position not found: symbol={symbol}")]`, not `#[error("not found")]`

- [ ] `dyn Trait` usage is documented with justification comment
  - Justified uses: `Arc<dyn PatternDetector>` in `PatternRegistry` (runtime-registered patterns), `Arc<dyn ExecutionStrategy>` (runtime-swappable strategies)
  - Unjustified: using `Box<dyn Trait>` where a generic `<T: Trait>` would work at zero runtime cost

---

## Documentation

- [ ] New `pub` items have `///` doc comments with `# Errors` section if fallible
  - Every new `pub struct`, `pub enum`, `pub fn`, `pub trait` needs a doc comment
  - Fallible functions (`-> Result<T, E>`) must have a `# Errors` section listing when each error variant is returned
  - Example:
    ```rust
    /// Calculates the position size for a given trade context.
    ///
    /// # Errors
    /// Returns `RiskEngineError::PositionSizing` if the account equity is zero.
    /// Returns `RiskEngineError::LimitExceeded` if the calculated size exceeds configured limits.
    pub async fn calculate_position_size(&self, ctx: &TradeContext) -> Result<Quantity, RiskEngineError>
    ```

---

## Tests

- [ ] Every new `pub fn` has at least one test
  - Test function names: `test_<what>_<condition>_<expected>`
  - Tests must make meaningful assertions — `assert!(true)` is forbidden
  - `assert!(false, "not implemented")` is forbidden

- [ ] Property-based tests (`proptest`) added for pure functions with numeric inputs
  - If a function takes `Decimal` or numeric newtypes and returns a computed value, add a `proptest` to catch edge cases

---

## Final Gate

- [ ] `./scripts/verify_agent_work.sh` passes
  - Run this last, after all other items are checked
  - This is the machine-enforced gate — if it fails, the change is not ready
  - Do not bypass with `--no-verify` or equivalent

---

## Quick Reference — Common Grep Commands

```bash
# Find raw primitives in changed file
grep -n 'pub.*: f64\|pub.*: u32\|pub.*: i64\|pub.*: String' src/<file>.rs

# Find unwraps in changed file (excluding tests)
grep -n 'unwrap()\|\.expect(' src/<file>.rs

# Find untyped errors in changed file
grep -n 'Box<dyn.*Error\|Result<.*,.*String' src/<file>.rs

# Find for-loops building Vecs
grep -n '\.push(' src/<file>.rs

# Find missing doc comments on pub items
grep -n '^pub ' src/<file>.rs | grep -v '//'

# Run verify
./scripts/verify_agent_work.sh
```
