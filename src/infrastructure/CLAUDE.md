# infrastructure/ — Infrastructure Layer

## Purpose
Repository implementations that bridge `domain::` port contracts with `db::` operations.

## Dependency Rules
- **ALLOWED**: `db::*`, `domain::*`
- **Both are documented exceptions** in `.governance/architecture_exceptions.txt`
- **FORBIDDEN**: `risk`, `orders`, `positions`, `execution`, `patterns`, `events`

## Rules
- This module implements the repository pattern: domain defines the interface, infrastructure implements it using db.
- Keep implementations thin — business logic belongs in the domain or business logic layer.
- All repository methods must be async and return typed `Result<T, E>`.
