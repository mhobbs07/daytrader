# domain/ — Infrastructure Layer (Isolated)

## Purpose
Pure domain models. This module is fully isolated — it depends on nothing outside itself.

## Dependency Rules
- **ALLOWED**: `domain::*` (internal only)
- **FORBIDDEN**: ALL other modules (`brokers`, `config`, `db`, `events`, `execution`, `infrastructure`, `orders`, `patterns`, `positions`, `risk`, `utils`)

## Rules
- This is the most isolated module in the codebase. It has zero external dependencies within the project.
- Domain models defined here should be pure data structures with business logic methods.
- If you need types from other modules, you are violating domain isolation. Re-think your approach.
- External crate dependencies should be minimal (serde, rust_decimal at most).
