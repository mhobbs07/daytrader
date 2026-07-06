---
paths:
  - "src/**/*.rs"
---

# Documentation Rules

1. **All public functions, types, traits, and modules** MUST have `///` doc comments.
2. **Doc comments** should include: brief description, parameters (if not obvious), return value, errors, and a usage example for complex APIs.
3. **Module-level docs** (`//!`) required for every `mod.rs` or `lib.rs`.
4. Use **`# Examples`** section in doc comments for public APIs.
5. Use **`# Errors`** section when returning `Result<T, E>`.
6. Use **`# Panics`** section if the function can panic (though it shouldn't in this project).
7. **Explain WHY, not WHAT** — keep comments concise; the code shows what.
8. **No commented-out code** in production — use version control instead.
