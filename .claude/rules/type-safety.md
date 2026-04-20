---
paths:
  - "src/**/*.rs"
---

# Type Safety Rules

These are non-negotiable invariants for all Rust source code:

1. **Newtypes for domain values**: Never use raw `f64`, `u32`, `String`, or `bool` for domain concepts. Use the established newtypes: `Price`, `Quantity`, `Symbol`, `Confidence`, `OrderId`, `PatternName`, `DollarAmount`, `Percentage`, `Count`.
2. **Decimal for money**: All monetary values use `rust_decimal::Decimal`. Never `f64` for prices, amounts, balances, P&L.
3. **Typed errors**: All public functions return `Result<T, E>` with `thiserror` error enums. No `String` errors, no `Box<dyn Error>` in library code. `anyhow` only in `main.rs`.
4. **No unsafe**: `#![forbid(unsafe_code)]` is set in `lib.rs`. Do not add `unsafe` blocks anywhere.
5. **No unwrap/expect in production**: Use `?` operator or explicit error handling. `.unwrap()` and `.expect()` are only allowed inside `#[cfg(test)]` blocks.
6. **No panic in production**: No `panic!()`, `todo!()`, or `unimplemented!()` in non-test code paths.
