---
paths:
  - "src/**/*.rs"
---

# Coding Style

1. **Immutability by default**: `let` not `let mut` unless mutation is required. Prefer `&T` over `&mut T`.
2. **Functional paradigms**: Prefer `.iter().map().collect()` over `for` loops with mutation. Use pattern matching exhaustively.
3. **Static dispatch by default**: Generic `T: Trait` over `dyn Trait` unless dynamic dispatch is required for heterogeneous collections.
4. **Trait boundaries on public APIs**: Public functions accept trait bounds, not concrete types, where applicable.
5. **Naming conventions**:
   - Modules: `snake_case`
   - Types: `PascalCase`, descriptive (`PositionTracker`, not `PosTrack`)
   - Functions: `snake_case`, verb-first (`calculate_pnl`, not `pnl_calc`)
   - Constants: `SCREAMING_SNAKE_CASE`
6. **No `Cell`/`RefCell`/interior mutability** unless at async boundaries where `Arc<Mutex<T>>` is the documented pattern.
