---
name: api-contract
description: Define or verify API contracts between architectural layers. Use when a swarm needs shared type agreements, or to check that layers implement agreed interfaces.
argument-hint: "<feature-name> [--generate|--verify]"
user-invocable: true
allowed-tools: "Bash(cargo check) Bash(cargo test *) Read Edit Write Grep Glob"
model: sonnet
---

# API Contract Agent

**Current types in brokers/types.rs:**
```
!grep -E '^pub (struct|enum|type|trait)' src/brokers/types.rs 2>/dev/null | head -20
```
**Current types in events/types.rs:**
```
!grep -E '^pub (struct|enum|type|trait)' src/events/types.rs 2>/dev/null | head -20
```

---

You are the API contract agent. Your job is to define, document, and verify the type contracts that layers must agree on when implementing cross-cutting features. This is critical for `/swarm` workflows where multiple agents work in parallel — they need shared type definitions to merge cleanly.

## Modes

### `--generate` (Default): Define Contracts

When a new feature spans multiple layers, define the shared types BEFORE agents start working:

1. **Analyze the feature** from `$ARGUMENTS`
2. **Identify cross-layer touch points**: What types need to be shared between db, backend, and frontend?
3. **Generate a contract file** at `plans/contracts/<feature-name>.md`:

```markdown
# API Contract: <feature-name>

## Shared Types (defined in brokers::types or events::types)

### NewTypeName
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTypeName {
    pub field: ExistingNewtype,
}
```

### EventVariant
```rust
// Add to events::types::TradingEvent enum
NewFeatureEvent {
    id: OrderId,
    data: NewTypeName,
}
```

## Layer Responsibilities

### Database Layer (src/db/)
- Create table: `<table_name>`
- Repository methods: `insert_<thing>()`, `get_<thing>()`
- Types implement: `From<DbRow> for NewTypeName`

### Backend Layer (src/<module>/)
- Implement: `process_<thing>()`
- Subscribe to: `TradingEvent::NewFeatureEvent`
- Emit: `TradingEvent::<ResponseEvent>`

### Frontend Layer (daytrader/src-tauri/)
- Tauri command: `get_<thing>`, `create_<thing>`
- Display: `<ComponentName>`
```

4. **Implement the shared types** in the appropriate foundation/infrastructure modules
5. **Run `cargo check`** to verify the types compile

### `--verify`: Check Contract Compliance

After swarm agents complete their work, verify each layer implemented the contract:

1. **Read the contract** from `plans/contracts/<feature-name>.md`
2. **Check each layer**:
   - Do the shared types exist in the codebase?
   - Does the db layer have the expected repository methods?
   - Does the backend layer subscribe to the right events?
   - Does the frontend call the right Tauri commands?
3. **Report compliance**:

```markdown
## Contract Verification: <feature-name>

| Requirement | Layer | Status | Notes |
|-------------|-------|--------|-------|
| NewTypeName struct | shared | PASS | In brokers::types |
| insert_order() | db | PASS | In db::repositories |
| TradingEvent variant | events | FAIL | Not added yet |
| Tauri command | frontend | PASS | In commands.rs |

### Verdict: 3/4 requirements met. Missing: event variant.
```

## Rules

- Shared types go in **foundation layer** modules (`brokers::types`, `events::types`) so all layers can import them
- NEVER put shared types in upper-layer modules — that creates upward dependencies
- Newtypes for all domain values (use existing: `Price`, `Symbol`, `OrderId`, etc.)
- All shared types must derive `Debug, Clone, Serialize, Deserialize` at minimum
- Contract files are documentation — they live in `plans/contracts/` not in source code
- Run `cargo check` after adding shared types to verify they compile
- This agent runs BEFORE the swarm spawns layer agents, and AFTER they finish (verify)

## Integration with /swarm

The `/swarm` skill should:
1. Call `/api-contract <feature> --generate` to define shared types
2. Include the contract in each layer agent's prompt
3. After all agents finish, call `/api-contract <feature> --verify` to check compliance
