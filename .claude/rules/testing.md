---
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
---

# Testing Rules

1. **Every new public function MUST have at least one test.**
2. **Meaningful assertions only**: `assert!(true)` and `assert!(false, "not implemented")` are FORBIDDEN. Tests must assert actual behavior.
3. **No self-comparisons**: `assert_eq!(x, x)` is a no-op and is forbidden.
4. **Property-based tests** (`proptest`) required for pure functions with numeric inputs.
5. **Integration tests use real types**, not just mocks.
6. **Performance tests** include hard threshold assertions (not just "it ran").
7. **Test naming**: `test_<what>_<condition>_<expected>` (e.g., `test_position_sizing_exceeds_limit_returns_error`).
8. **Baseline exceptions** are tracked in `.governance/known_test_issues.txt`. Do not add new entries without justification.
