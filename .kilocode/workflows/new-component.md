---
name: new-component
version: 1.0
description: Create a new module/component following all standards
steps:
  - create-module-structure
  - define-types
  - define-traits
  - implement
  - add-tests
  - create-docs
  - create-backlog
  - verify
---

# Workflow: New Component

Workflow for adding a new module to the codebase. Follow every step in order — skipping steps results in tech debt on day one. Each step enforces a coding standard from `.kilocode/coding-standards.md`.

---

## Step 1 — Create Module Structure

Create the directory and `mod.rs` entry point.

```bash
mkdir -p src/<component>
touch src/<component>/mod.rs
touch src/<component>/types.rs
touch src/<component>/traits.rs
touch src/<component>/errors.rs
```

Write the module-level doc comment in `src/<component>/mod.rs`:

```rust
//! # <Component> Module
//!
//! <One sentence describing the module's responsibility in the system.>
//!
//! ## Key Types
//!
//! - [`<PrimaryType>`] — <what it represents>
//! - [`<SecondaryType>`] — <what it represents>
//!
//! ## Usage
//!
//! ```rust,no_run
//! // Brief example showing the primary use case
//! ```

pub mod errors;
pub mod traits;
pub mod types;
// Add implementation submodules here as needed
```

Register the new module in `src/lib.rs` or the appropriate parent `mod.rs`.

---

## Step 2 — Define Types

In `src/<component>/types.rs`, define all domain types using newtypes from the start. **No raw `f64`, `u32`, `String`, or `i64` in public struct fields for domain values.**

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// Import existing newtypes rather than redefining
use crate::brokers::types::{Price, Quantity, Symbol};
use crate::risk::types::{DollarAmount, Percentage};

/// <Description of what this type represents>
///
/// # Invariants
/// - <List any invariants the type enforces>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct <PrimaryType> {
    pub field: <NewtypeOrBuiltin>,
    // ...
}
```

Rules:
- Use newtypes from `src/brokers/types.rs` and `src/risk/types.rs` before defining new ones
- If a new newtype is needed, define it following the pattern in those files (inner value, `new()` with validation, `value()` accessor, `Display`, derive macros)
- All public struct fields for domain values must use newtypes
- `HashMap<String, f64>` for config-driven arbitrary parameters is acceptable — document the exception

---

## Step 3 — Define Traits

In `src/<component>/traits.rs`, define the trait interface **before** writing the implementation.

```rust
use async_trait::async_trait;
use crate::<component>::errors::<ComponentError>;
use crate::<component>::types::<PrimaryType>;

/// <Description of what this trait represents and why it exists as a trait>
///
/// # Design Note
/// <Explain why this is a trait rather than a concrete struct — e.g., testability,
/// multiple implementations, dependency inversion.>
#[async_trait]
pub trait <ComponentTrait>: Send + Sync {
    /// <Description>
    ///
    /// # Errors
    /// Returns `<ComponentError>::<Variant>` if <condition>.
    async fn <method>(&self, param: <Type>) -> Result<<ReturnType>, <ComponentError>>;
}
```

Rules:
- Define the trait before the implementation — it clarifies the contract
- All public methods must document `# Errors` if they return `Result`
- Trait objects (`Arc<dyn ComponentTrait>`) are only justified if the concrete type must be selected at runtime — document the justification inline

---

## Step 4 — Implement

Create `src/<component>/<impl_name>.rs` (e.g., `engine.rs`, `service.rs`). Implement the trait defined in Step 3.

Rules:
- Use iterator chains over imperative loops
- Use `match` with exhaustive arms — avoid `_` catch-alls unless truly unreachable (document why)
- All `pub fn` that can fail return `Result<T, <ComponentError>>`
- No `.unwrap()` or `.expect()` in production code paths
- No `panic!()` in production code paths
- All `pub fn` have `///` doc comments; fallible functions have `# Errors` section

```rust
use crate::<component>::errors::<ComponentError>;
use crate::<component>::traits::<ComponentTrait>;
use crate::<component>::types::<PrimaryType>;

pub struct <ConcreteImpl> {
    // fields
}

#[async_trait::async_trait]
impl <ComponentTrait> for <ConcreteImpl> {
    async fn <method>(&self, param: <Type>) -> Result<<ReturnType>, <ComponentError>> {
        // Implementation using iterator chains and typed errors
    }
}
```

---

## Step 5 — Add Tests

Add unit tests in the same file under `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_<what>_<condition>_<expected>() {
        // Arrange
        let subject = <ConcreteImpl>::new(...);

        // Act
        let result = subject.<method>(...);

        // Assert — must be meaningful, not assert!(true)
        assert_eq!(result, expected_value);
    }
}
```

Add integration tests in `tests/<component>_integration.rs`:

```rust
#[tokio::test]
async fn test_<component>_<scenario>() {
    // Use real types, not just mocks
    // Test the full flow through the component
}
```

Rules (enforced by `verify_test_quality.sh`):
- Every new `pub fn` must have at least one test
- `assert!(true)` is forbidden
- `assert!(false, "not implemented")` is forbidden
- Property-based tests (`proptest`) required for pure functions with numeric inputs
- Test function names follow: `test_<what>_<condition>_<expected>`

---

## Step 6 — Create Architecture Doc

Create `docs/components/<component>.md`:

```markdown
# Component: <Name>

## Responsibility
<1-2 sentences describing what this module owns and what it does not own.>

## Key Types
| Type | Kind | Description |
|------|------|-------------|
| `<Type>` | struct/enum/newtype | <description> |

## Key Traits
| Trait | Description | Implementations |
|-------|-------------|----------------|
| `<Trait>` | <description> | `<Impl1>`, `<Impl2>` |

## Design Decisions
- <Why X was chosen over Y — e.g., why Arc<dyn Trait> is used here>
- <Any justified uses of `dyn Trait` with explanation>

## Known Tech Debt
- None on creation — all coding standards applied from the start

## Module Dependency Graph
<List the modules this component imports from and which modules import it.>
Allowed imports (per CLAUDE.md module boundaries): <list>
```

---

## Step 7 — Create Backlog

Create `docs/backlogs/<component>.md`:

```markdown
---
component: <name>
status: active
last_updated: <YYYY-MM-DD>
last_worked: <YYYY-MM-DD>
progress: "0/0 tasks complete"
owner: unassigned
blocks: []
blocked_by: []
---

# Backlog: <Component>

## Completed Tasks
_None yet — component freshly created._

## Pending Tasks
_None — all coding standards applied at creation._

## Notes
Created via `new-component` workflow on <date>.
```

---

## Step 8 — Verify

Run the full verification script. All checks must pass before committing.

```bash
./scripts/verify_agent_work.sh
```

Fix any failures before committing. A new component that fails verification on day one is not acceptable.

Once all checks pass, commit with message:

```
feat(<component>): add <component> module with types, traits, and tests
```
