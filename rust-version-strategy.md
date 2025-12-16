# Rust Version & Edition Strategy

**Context:** Late 2025  
**Target:** High-performance Day Trading Platform  
**Status:** Strategic Decision

---

## Executive Summary

Given the current date (Late 2025) and the project's performance requirements, we recommend using **Rust Edition 2024** with a minimum toolchain version of **Rust 1.85+**.

This decision is critical for achieving the architectural goals of **zero-cost abstractions** and **static dispatch**, particularly for the async broker interactions and functional pipelines.

---

## Recommended Configuration

### Cargo.toml
```toml
[package]
name = "daytrader"
version = "0.1.0"
edition = "2024"  # ✅ Use the latest stable edition
rust-version = "1.85"  # Minimum Supported Rust Version (MSRV)
```

### Toolchain Setup
```bash
# Ensure you are on the latest stable version (likely 1.90+ in late 2025)
rustup update stable
rustc --version
```

---

## Why Edition 2024?

The 2024 Edition introduces features that directly support our architectural constraints:

### 1. Native Async Traits (Zero-Cost Abstractions)

**The Problem (Pre-2024):**
Previously, using `async` in traits required the `async_trait` crate, which forced **dynamic dispatch** (`Box<dyn Future>`) and heap allocation on every call. This violates our "avoid dynamic dispatch" rule.

**The Solution (Edition 2024):**
Rust 2024 supports `async fn` in traits natively. This allows the compiler to desugar these into state machines without boxing, enabling **static dispatch** and inlining.

```rust
// ✅ Rust 2024: Zero-cost async trait
pub trait BrokerProvider {
    async fn place_order(&self, order: Order) -> Result<OrderId>;
}

// Compiles to static dispatch when used with generics:
pub async fn execute_strategy<B: BrokerProvider>(broker: &B) {
    broker.place_order(order).await; // Inlined, no vtable, no allocation
}
```

### 2. Return Position Impl Trait in Trait (RPITIT)

**The Problem:**
Our functional architecture relies on returning iterators (`impl Iterator`) to create lazy pipelines. Previously, traits could not return `impl Trait`, forcing us to return `Box<dyn Iterator>` (dynamic dispatch) or complex associated types.

**The Solution:**
Rust 2024 allows `-> impl Iterator` in trait methods.

```rust
pub trait PatternDetector {
    // ✅ Zero-cost abstraction: returns a concrete iterator type, not a Box
    fn detect(&self, candles: &[Candle]) -> impl Iterator<Item = PatternSignal>;
}

impl PatternDetector for HammerDetector {
    fn detect(&self, candles: &[Candle]) -> impl Iterator<Item = PatternSignal> {
        // ... implementation
    }
}
```

### 3. Perfect Capture in Closures

**The Benefit:**
Improved closure capture rules make it easier to write the functional combinators (map, filter, fold) used in our signal processing pipelines without fighting the borrow checker, leading to cleaner, more maintainable code.

---

## Stability & Risk Assessment

**Stability:**
By late 2025, Rust 2024 (released late 2024) has been stable for ~1 year. The ecosystem (tokio, serde, sqlx) will have fully adopted it.

**Risk:**
- **Toolchain Sync:** All developers and CI/CD pipelines must use Rust 1.85+.
- **Mitigation:** The `rust-version` field in `Cargo.toml` enforces this automatically.

---

## Implementation Guidelines

1.  **Remove `async_trait` Dependency:** Do not include the `async-trait` crate in `Cargo.toml`. Use native syntax.
2.  **Use `impl Trait` Return Types:** Leverage RPITIT for all functional pipelines to ensure zero-cost composition.
3.  **Enforce MSRV:** Keep `rust-version = "1.85"` in `Cargo.toml` to prevent accidental usage of older compilers that would fail to compile 2024 syntax.

## Conclusion

Using **Rust Edition 2024** is not just "newer"—it is **architecturally necessary** to meet the user's requirement of avoiding dynamic dispatch and using higher-order functions efficiently. It enables true zero-cost async abstractions that were previously impossible or required heavy workarounds.
