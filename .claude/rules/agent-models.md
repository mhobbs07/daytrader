---
paths:
  - "**/*"
---

# Agent Model Budget Policy

When spawning subagents with the Agent tool, you MUST assign models based on task complexity to control costs. Do not use opus for tasks that sonnet or haiku can handle.

## Model Assignments

| Task Type | Model | Examples |
|-----------|-------|----------|
| **Architecture decisions, complex orchestration, cross-cutting analysis** | `opus` | Swarm orchestration, architecture audits, complex refactoring |
| **Business logic, schema design, algorithm implementation** | `sonnet` | Risk engine changes, pattern detection, order lifecycle, database schema |
| **Template work, simple edits, running commands, test execution** | `haiku` | Frontend components, running verification scripts, formatting fixes |

## Rules

1. **Default to the cheapest model that can do the job.** When in doubt, start with `sonnet`.
2. **Never use opus for subagents** unless the task requires deep architectural reasoning across multiple modules.
3. **haiku is preferred** for: running tests, formatting, simple file edits, frontend template work, command execution.
4. **sonnet is preferred** for: implementing business logic, writing tests, database work, API design.
5. **Maximum 4 concurrent agents** to keep total token spend bounded.
6. When using `/swarm`, follow the model assignments in the swarm skill definition.

## Specifying the Model

Use the `model` parameter on the Agent tool:

```
Agent(model="haiku", prompt="Run cargo test and report results")
Agent(model="sonnet", prompt="Implement the risk calculation for...")
```
