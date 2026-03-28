---
name: update-backlog
version: 1.0
description: Update a component backlog file after completing work
applies_to: ["docs/backlogs/*.md"]
trigger: "task completed"
---

# Skill: Update Backlog

Use this skill after completing any refactoring task. Backlog files in `docs/backlogs/` are the authoritative record of what has been done, what is in progress, and what remains. Keep them current — stale backlogs mislead the next agent.

**Important**: The backlog update must be included in the same commit as the code change. Do not create a separate commit just for the backlog.

## Step 1 — Open the relevant backlog file

Each component has one backlog file:

| Component | File |
|-----------|------|
| Brokers | `docs/backlogs/brokers.md` |
| Events | `docs/backlogs/events.md` |
| Risk | `docs/backlogs/risk.md` |
| Patterns | `docs/backlogs/patterns.md` |
| Orders | `docs/backlogs/orders.md` |
| Positions | `docs/backlogs/positions.md` |
| Execution | `docs/backlogs/execution.md` |
| Database | `docs/backlogs/db.md` |
| Config | `docs/backlogs/config.md` |
| Utils | `docs/backlogs/utils.md` |
| Domain | `docs/backlogs/domain.md` |

## Step 2 — Find the task by ID

Each task has an ID in the format `RF-<COMPONENT>-<NUMBER>` (e.g. `RF-R-001`, `RF-P-002`). Search for it in the Active or Pending table:

```bash
grep -n "RF-R-001" docs/backlogs/risk.md
```

## Step 3 — Update the YAML frontmatter

At the top of the file, update:
- `last_updated`: set to today's date in `YYYY-MM-DD` format
- `last_worked`: set to today's date
- `progress`: update the fraction (e.g. `"3/10 tasks complete"` → `"4/10 tasks complete"`)

```yaml
---
component: risk
status: needs-refactor
last_updated: 2026-03-28
last_worked: 2026-03-28
progress: "4/10 tasks complete"
owner: unassigned
blocks: []
blocked_by: []
---
```

If all tasks in the file are now complete, change `status` from `needs-refactor` to `stable`.

## Step 4 — Move the task row to Completed

Find the task row in the **Active** or **Pending** table and move it to the **Completed** table. If there is no Completed table yet, add one.

**Active/Pending table row (before):**
```markdown
| RF-R-001 | Replace `Box<dyn Error>` in `risk/engine.rs` (11 methods) | `src/risk/engine.rs` | high |
```

**Completed table row (after):**
```markdown
| RF-R-001 | Replace `Box<dyn Error>` in `risk/engine.rs` | 2026-03-28 | `feat/risk-engine-errors` |
```

The Completed table format:

```markdown
## Completed

| ID | Description | Completed | Branch |
|----|-------------|-----------|--------|
| RF-R-001 | Replace `Box<dyn Error>` in `risk/engine.rs` (11 methods) | 2026-03-28 | feat/risk-engine-errors |
```

## Step 5 — Update the progress field

Count the remaining tasks in Active/Pending and the total tasks (Active + Completed), and update the `progress` field in the YAML frontmatter accordingly.

```yaml
progress: "4/10 tasks complete"
```

## Step 6 — Include in the same commit

Stage both the code changes and the backlog update together:

```bash
git add src/risk/engine.rs src/utils/errors.rs docs/backlogs/risk.md
git commit -m "feat(risk): replace Box<dyn Error> with RiskEngineError in engine.rs

RF-R-001: All 11 internal methods now return Result<_, RiskEngineError>.
Updates docs/backlogs/risk.md to mark RF-R-001 as complete."
```

The backlog update and the code change must share a single commit. This keeps the history coherent — if the commit is reverted, the backlog reverts with it.
