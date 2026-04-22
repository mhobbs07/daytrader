# brokers/ — Foundation Layer

## Purpose
Broker abstraction: traits, shared types, and implementations (Alpaca live, Mock for testing).

## Dependency Rules
- **`brokers/types.rs`, `brokers/traits.rs`** — ALLOWED: `utils::errors`. FORBIDDEN: all other modules.
- **`brokers/alpaca/`, `brokers/mock.rs`** — ALLOWED: `brokers::types`, `brokers::traits`, `utils::errors`. FORBIDDEN: all other modules.

## Key Types
- `Symbol`, `Price`, `Quantity`, `Confidence`, `OrderId`, `PatternName`, `Reason`, `Message`, `Notes` — canonical newtypes for the entire codebase
- `BrokerTrait` — the trait that all broker implementations must satisfy
- `MarketData`, `OrderRequest`, `OrderStatus` — shared data structures

## Rules
- Newtypes defined here are the source of truth. Do not redefine them elsewhere.
- New broker implementations must implement `BrokerTrait`.
- All broker communication must use TLS (`rustls`).
- API keys handled via `keyring` or environment variables, never hardcoded.
