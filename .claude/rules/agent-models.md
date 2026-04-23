---
paths:
  - "**/*"
---

# Agent Model Budget Policy

When spawning subagents with the Agent tool, you MUST assign models based on task complexity to control costs. Haiku is **NOT** permitted for this project — it has repeatedly ignored isolation/worktree boundaries and committed to shared branches in violation of governance rules.

## Model Assignments

| Task Type | Model | Examples |
|-----------|-------|----------|
| **Architecture decisions, complex orchestration, cross-cutting analysis** | `opus` | Swarm orchestration, architecture audits, complex refactoring |
| **Everything else (business logic, schema design, algorithm implementation, frontend, simple edits, verification)** | `sonnet` | All subagent work |

## Rules

1. **Default to `sonnet`.** It is the cheapest model approved for subagent work in this project.
2. **Never use `opus` for subagents** unless the task requires deep architectural reasoning across multiple modules.
3. **`haiku` is prohibited.** Do not pass `model="haiku"` to the Agent tool. Past haiku agents bypassed `isolation: "worktree"` and committed directly to shared branches (e.g., `staging`), violating branching rules and requiring manual cleanup.
4. **Maximum 4 concurrent agents** to keep total token spend bounded.
5. When using `/swarm`, follow the model assignments in the swarm skill definition.

## Specifying the Model

Use the `model` parameter on the Agent tool:

```
Agent(model="sonnet", prompt="Implement the risk calculation for...")
Agent(model="opus", prompt="Audit architecture boundaries across modules...")
```
