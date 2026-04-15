# execution/ — TOP OF DEPENDENCY TREE

## Purpose
Automated execution engine. Orchestrates the full trading loop: pattern signals -> risk evaluation -> order placement -> position tracking.

## Dependency Rules
- **ALLOWED**: `brokers::*`, `events`, `orders`, `positions`, `risk`, `patterns::signals`, `config`, `execution::*` (internal)
- **FORBIDDEN**: Nothing — this is the top of the tree.

## THE CRITICAL RULE
**No other module may import from `execution::`.**

If you find yourself wanting to import an execution type from a lower module, STOP. That is an architectural violation. Instead:
1. Move the shared type to a lower-layer module (`events::types`, `brokers::types`)
2. Use the event bus for communication
3. Document an exception in `.governance/architecture_exceptions.txt` (last resort)

## Rules
- This is the only module that ties all layers together.
- The execution engine subscribes to pattern signals and orchestrates the full trade lifecycle.
- All async — uses Tokio tasks for concurrent operation.
- Must handle all error cases gracefully — a crash here stops trading.
