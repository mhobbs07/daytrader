---
paths:
  - "src/**/*.rs"
---

# Architecture Boundary Rules

The dependency graph flows strictly downward. Higher layers depend on lower layers, never the reverse.

## Layer Hierarchy (top to bottom)

```
Orchestration:  execution -> orders
Business Logic: risk, patterns, positions
Infrastructure: config, events, db, domain, infrastructure
Foundation:     utils, brokers
```

## The Golden Rule

**Nothing may import `execution::`**. It is the top of the dependency tree. If you need execution types from a lower module, use the event bus instead.

## Boundary Verification

- `./scripts/verify_architecture.sh` enforces these boundaries
- Approved exceptions are in `.governance/architecture_exceptions.txt`
- Adding a new exception requires explicit justification in that file
- Per-module CLAUDE.md files in `src/<module>/` list exact allowed/forbidden imports

## When You Need a Cross-Module Type

If your change requires importing from a forbidden module:
1. Check if the type exists in a shared location (`brokers::types`, `events::types`)
2. Consider moving the type to a lower-layer shared module
3. Use the event bus for communication across layers
4. As a last resort, document an exception in `.governance/architecture_exceptions.txt`
