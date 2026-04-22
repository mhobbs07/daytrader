# events/ — Infrastructure Layer

## Purpose
Event bus for async pub/sub communication between modules. Uses `tokio::sync::broadcast` channels.

## Dependency Rules
- **ALLOWED**: `brokers::types` (for shared type re-exports)
- **FORBIDDEN**: `risk`, `orders`, `positions`, `execution`, `patterns`, `db`, `config`

## Rules
- The event bus is the primary mechanism for cross-layer communication.
- All subscribers MUST handle `RecvError::Lagged` — dropped messages are expected under load.
- Event types are defined here and shared upward. Lower modules must not depend on events.
- Use broadcast channels for fan-out, mpsc for directed messages.
