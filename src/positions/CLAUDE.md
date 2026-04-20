# positions/ — Business Logic Layer

## Purpose
Position tracking, P&L calculation, and portfolio state management.

## Dependency Rules
- **ALLOWED**: `brokers::types`, `brokers::traits`, `events`, `orders::types`, `positions::*` (internal)
- **Exception**: `positions -> orders` for fill processing (`FillInfo`, `ManagedOrder`) — documented in `.governance/architecture_exceptions.txt`
- **FORBIDDEN**: `execution`, `risk`, `patterns`

## Rules
- `PositionTracker` processes order fills to maintain current position state.
- P&L calculations use `Decimal` arithmetic exclusively.
- Position state is eventually consistent with broker state — reconciliation logic handles discrepancies.
- All position mutations go through explicit methods on `PositionTracker`, never direct field access.
