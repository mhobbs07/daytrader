---
name: add-typed-error
version: 1.0
description: Replace Box<dyn Error> or Result<_, String> with a thiserror enum variant
applies_to: ["*.rs"]
trigger: "Box<dyn Error> or String error type found"
---

# Skill: Add Typed Error

Use this skill when you find a function returning `Result<T, Box<dyn std::error::Error + Send + Sync>>` or `Result<T, String>`. Both forms are non-compliant — they erase type information and prevent callers from matching on error variants.

## Step 1 — Identify the module and current error

Locate the function and note:
- The module it belongs to (e.g. `src/risk/engine.rs` → module `risk`)
- What errors can actually occur (database errors, validation errors, lock failures, etc.)
- How many functions in the same module share the same error type (group them into one enum)

```bash
grep -n "Box<dyn.*Error" src/<module>/*.rs
grep -n "Result<.*String>" src/<module>/*.rs
```

## Step 2 — Check for an existing error type

Look in the canonical error registry first:

```bash
grep -n "pub enum.*Error" src/utils/errors.rs
grep -n "pub enum.*Error" src/<module>/*.rs
```

The error hierarchy is:
```
AppError (src/utils/errors.rs)
├── BrokerError
├── RiskError
├── RiskEngineError
├── RepositoryError
├── DatabaseError
├── PatternError
├── ConfigError
└── ... (one per module)
```

If the module already has an error enum in `src/utils/errors.rs` and it covers your case, use it. Skip to Step 5.

## Step 3 — Define the new error enum

Add to `src/utils/errors.rs` (preferred — keeps all errors in one canonical location):

```rust
#[derive(Debug, thiserror::Error)]
pub enum RiskEngineError {
    #[error("Risk rule evaluation failed: {0}")]
    RuleEvaluation(String),
    #[error("Signal recording failed: {0}")]
    SignalRecording(String),
    #[error("Event processing failed: {0}")]
    EventProcessing(String),
    #[error("State update failed: {0}")]
    StateUpdate(String),
    #[error(transparent)]
    Risk(#[from] RiskError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
}
```

Guidelines for variants:
- Use `#[error(transparent)]` + `#[from]` for wrapping lower-level error types (enables `?` operator)
- Use descriptive `String` variants for dynamic error messages: `RuleEvaluation(String)`
- Use structured variants when callers need to extract data: `NotFound { entity: &'static str, id: String }`
- Do NOT create a catch-all `Other(String)` — every error should have a semantic variant

## Step 4 — Register the error in AppError

In `src/utils/errors.rs`, add a variant to `AppError`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ... existing variants ...

    #[error("Risk engine error: {0}")]
    RiskEngine(#[from] RiskEngineError),
}
```

The `#[from]` derive enables `?` propagation from `RiskEngineError` into `AppError` automatically.

## Step 5 — Change the function signature

```rust
// BEFORE
async fn evaluate_rules(
    &self,
    context: &TradeContext,
) -> Result<RiskEvaluation, Box<dyn std::error::Error + Send + Sync>> {

// AFTER
async fn evaluate_rules(
    &self,
    context: &TradeContext,
) -> Result<RiskEvaluation, RiskEngineError> {
```

Add the import at the top of the file:

```rust
use crate::utils::errors::RiskEngineError;
```

## Step 6 — Replace error constructions

### Replace `Box::new(e)` / `Box<dyn Error>` constructions

```rust
// BEFORE
Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
// or
return Err(format!("Failed to evaluate: {}", e).into());

// AFTER — use ? with #[from] conversion:
some_fallible_call()?
// or construct explicitly:
return Err(RiskEngineError::RuleEvaluation(format!("Failed to evaluate: {}", e)));
```

### Replace `Result<_, String>` constructions

```rust
// BEFORE
fn check_limit(&self) -> Result<bool, String> {
    if self.value > self.limit {
        return Err(format!("Limit exceeded: {} > {}", self.value, self.limit));
    }
    Ok(true)
}

// AFTER
fn check_limit(&self) -> Result<bool, RiskError> {
    if self.value > self.limit {
        return Err(RiskError::DailyLossLimitExceeded {
            current: self.value,
            limit: self.limit,
        });
    }
    Ok(true)
}
```

### Use `?` for propagation wherever possible

If `SubError` has `#[from]` in the parent enum, `?` works automatically:

```rust
// BEFORE
let result = db_operation().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

// AFTER (DatabaseError is registered in RiskEngineError via #[from])
let result = db_operation()?;
```

## Step 7 — Run cargo check and fix propagation

```bash
cargo check
```

Common fixes after the signature change:
- **Callers that used `?` into `Box<dyn Error>`** — they may now need explicit `.map_err()` if the error types don't align via `#[from]`
- **Functions that return different error types** — ensure the `#[from]` chain covers all conversions, or add explicit `map_err()` calls
- **Tests using `anyhow::Result` or `Box<dyn Error>`** — tests may keep `anyhow::Result` in the test function signature; that's acceptable

## Step 8 — Verify

```bash
./scripts/verify_agent_work.sh
```

**Acceptance check**: Run `grep -rn "Box<dyn.*Error" src/<module>/` and confirm zero results remain in production code.
