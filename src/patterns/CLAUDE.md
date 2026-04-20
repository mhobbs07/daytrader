# patterns/ — Business Logic Layer

## Purpose
Technical pattern recognition. Detects trading patterns from market data and emits signals.

## Dependency Rules
- **ALLOWED**: `brokers::types`, `events`, `db::repositories`, `patterns::*` (internal)
- **FORBIDDEN**: `orders`, `positions`, `execution`, `risk`

## Rules
- Pattern detection is read-only analysis. It produces signals but never places orders directly.
- Signals are published via the event bus for the execution engine to consume.
- Each pattern has a confidence score using the `Confidence` newtype (0.0-1.0).
- Pattern implementations should be stateless where possible.
- New patterns must include unit tests with known market data fixtures.
