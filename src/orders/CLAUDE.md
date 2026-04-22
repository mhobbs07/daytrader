# orders/ — Orchestration Layer

## Purpose
Order lifecycle management. Creates, validates, submits, and tracks orders through the broker.

## Dependency Rules
- **ALLOWED**: `brokers::*`, `events`, `risk::engine`, `orders::*` (internal)
- **FORBIDDEN**: `execution`, `patterns`

## Rules
- Orders are validated through the risk engine BEFORE submission to the broker.
- Order state transitions are published via the event bus.
- The order manager coordinates between risk validation and broker submission.
- Never bypass risk checks — if `RiskEngine::evaluate()` rejects an order, it must not be submitted.
- Order IDs use the `OrderId` newtype.
