---
name: refactor-to-newtype
version: 1.0
description: Convert a primitive field/parameter to a newtype wrapper
applies_to: ["*.rs"]
trigger: "primitive used as domain value"
---

# Skill: Refactor Primitive to Newtype

Use this skill when you find a raw primitive (`f64`, `u32`, `i64`, `String`) being used as a domain concept in a struct field or function parameter. Example: `win_rate: f64` should be `win_rate: WinRate`.

## Step 1 — Check for an existing newtype

Search canonical newtype registries first:

```bash
grep -n "pub struct" src/brokers/types.rs src/risk/types.rs
```

Look for a type that matches the concept. Common existing newtypes:
- `DollarAmount` — monetary values in USD
- `Percentage` — 0.0–100.0 ratio values
- `Count` — non-negative integer counts
- `Confidence` — 0.0–1.0 probability values
- `Symbol` — ticker strings (uppercase, 1–10 chars)
- `Price` — positive market prices
- `Quantity` — positive share counts
- `OrderId` — string order identifiers
- `PatternName` — pattern identifier strings

If a suitable type exists, skip to Step 3.

## Step 2 — Define the newtype

Add to `src/risk/types.rs` (for risk/trading concepts) or `src/brokers/types.rs` (for broker/market concepts). Follow this exact template:

```rust
/// Newtype for <description>. Range: <min>..<max>.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WinRate(f64);

impl WinRate {
    /// Creates a new `WinRate`.
    ///
    /// # Errors
    /// Returns `Err` if `value` is not in the range 0.0..=100.0.
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !(0.0..=100.0).contains(&value) {
            return Err("WinRate must be between 0.0 and 100.0");
        }
        Ok(Self(value))
    }

    /// Returns the inner `f64` value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Returns 0% win rate.
    pub fn zero() -> Self {
        Self(0.0)
    }
}

impl fmt::Display for WinRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.0)
    }
}
```

Adjust the inner type (`f64`/`u32`/`String`) and validation range for the concept.

For newtypes with no meaningful validation (e.g. a count that can be any `u32`), use an infallible constructor:

```rust
pub fn new(value: u32) -> Self {
    Self(value)
}
```

## Step 3 — Update the struct field

Change the field declaration:

```rust
// BEFORE
pub win_rate: f64,

// AFTER
pub win_rate: WinRate,
```

Add the import at the top of the file:

```rust
use crate::risk::types::WinRate;
// or
use crate::brokers::types::WinRate;
```

## Step 4 — Find and fix construction sites

```bash
grep -rn "win_rate:" src/
grep -rn "WinRate {" src/    # if any struct-init syntax
```

Update each construction site to use the validated constructor with `?` propagation:

```rust
// BEFORE
PatternStats { win_rate: 0.75, .. }

// AFTER
PatternStats { win_rate: WinRate::new(75.0)?, .. }
// Note: if the original was a ratio (0.0–1.0), multiply by 100 to convert to percentage
```

In `#[cfg(test)]` blocks, `.unwrap()` is acceptable on known-valid values:

```rust
win_rate: WinRate::new(75.0).unwrap(),
```

## Step 5 — Find and fix usage sites

```bash
grep -rn "\.win_rate" src/
```

For each read of the field, update to call `.value()`:

```rust
// BEFORE
if stats.win_rate >= 0.6 {

// AFTER
if stats.win_rate.value() >= 60.0 {
```

Or use the `Display` impl where the value is formatted:

```rust
// BEFORE
format!("Win rate: {}", stats.win_rate)

// AFTER
format!("Win rate: {}", stats.win_rate)  // unchanged — Display impl handles formatting
```

## Step 6 — Run cargo check

```bash
cargo check
```

Fix any remaining type mismatches. Common patterns:
- Arithmetic on newtypes requires `.value()` first: `a.value() + b.value()`
- Comparisons require both sides to be the same type — either compare raw values or implement `PartialOrd` with a primitive
- SQLx queries need `.value()` when binding: `query.bind(self.win_rate.value())`
- Serde serializes/deserializes transparently if `Serialize`/`Deserialize` are derived

## Step 7 — Verify

```bash
./scripts/verify_agent_work.sh
```

All checks must pass. If architecture checks fail, verify that the newtype is defined in the correct layer (`brokers/types.rs` or `risk/types.rs`) and not in a higher-level module.
