# db/ — Infrastructure Layer

## Purpose
Database persistence layer. SQLite via SQLx. Repository pattern for data access.

## Dependency Rules
- **ALLOWED**: `db::*` (internal), `brokers::types`, `events::types`, `patterns::signals`, `patterns::types`
- **All four non-internal imports are documented exceptions** in `.governance/architecture_exceptions.txt`
- **FORBIDDEN**: `risk`, `orders`, `positions`, `execution`

## Rules
- Database models mirror domain types for serialization — this requires importing from `brokers`, `events`, and `patterns`.
- Use SQLx compile-time checked queries where possible.
- All database operations must be async.
- Repository functions return `Result<T, DbError>` with typed errors.
- Never expose raw SQL outside this module.
