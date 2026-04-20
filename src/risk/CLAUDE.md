# risk/ — Business Logic Layer

## Purpose
Risk management engine. Enforces position sizing, loss limits, PDT compliance, and pattern-specific risk profiles.

## Dependency Rules
- **ALLOWED**: `brokers::types`, `config`, `db`, `events`, `risk::*` (internal)
- **FORBIDDEN**: `orders`, `positions`, `execution`, `patterns`

## Key Types
- `DollarAmount`, `Percentage`, `Count`, `AccountId` — newtypes for risk domain
- `RiskEngine` — central risk evaluation entry point
- `PatternRiskProfile` — per-pattern risk configuration

## Rules
- **PDT compliance checks MUST NEVER be bypassed.** This is SEC-mandated. No escape hatches, no feature flags, no "just for testing" bypasses.
- Risk limits are validated BEFORE order submission (pre-trade).
- The risk engine is called by the orders module — risk does not call orders.
- All risk calculations use `Decimal` arithmetic, never floating point.
