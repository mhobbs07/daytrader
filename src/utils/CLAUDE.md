# utils/ — Foundation Layer

## Purpose
Shared utilities, error types, and helpers used across the entire codebase.

## Dependency Rules
- **ALLOWED imports**: `utils::*` (internal), external crates only
- **EXCEPTION**: `brokers::types` (for error context in `BrokerError`) — documented in `.governance/architecture_exceptions.txt`
- **FORBIDDEN**: `risk`, `orders`, `positions`, `execution`, `patterns`, `events`, `config`, `db`

This is the bottom of the dependency tree. If you need to reference types from upper layers, you are creating a circular dependency. Use a trait or type parameter instead.

## Key Types
- `CorrelationId` — newtype for request tracing
- Error types in `errors.rs` — project-wide error definitions using `thiserror`

## Rules
- No upward dependencies. Ever.
- Error types must include descriptive context messages.
- Utility functions should be pure where possible.
