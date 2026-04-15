# config/ — Infrastructure Layer

## Purpose
Configuration management. Loads and validates application settings including risk parameters.

## Dependency Rules
- **ALLOWED**: `risk::types` (for config struct fields — documented exception)
- **FORBIDDEN**: `orders`, `positions`, `execution`, `patterns`, `events`, `db`

## Rules
- Config structs embed risk types (e.g., `ConfidenceMultiplier`, `PatternRiskProfile`) — this is an approved exception.
- Never store credentials in config files. Use environment variables or `keyring`.
- All config values must have sensible defaults and validation.
