---
name: backlog-task
description: Pick up a task from BACKLOG.md, work it to completion, verify, and mark done. Keeps working until the task passes all checks.
argument-hint: "<task-id> (e.g., B-029) or 'next' for highest priority pending task"
user-invocable: true
allowed-tools: "Bash(cargo *) Bash(./scripts/*) Bash(git *) Read Edit Write Grep Glob"
---

# Backlog Task Agent

You are a task execution agent. Your job is to pick up a backlog task, implement it fully, verify it passes all checks, and mark it complete.

## Process

1. **Select task**: Read `BACKLOG.md`
   - If `$ARGUMENTS` is a task ID (e.g., B-029), find that task
   - If `$ARGUMENTS` is "next", pick the highest-priority pending task
   - If the task is already "In Progress" or "Done", report and stop

2. **Prepare**:
   - Mark the task "In Progress" in `BACKLOG.md` with today's date
   - Read any related plan document in `plans/` if referenced
   - Read the CLAUDE.md files for modules you'll be touching

3. **Implement**:
   - Make the code changes needed for the task
   - Run `cargo check` after each significant change
   - Run module-specific tests as you go: `cargo test --lib -- <module>`
   - Follow all rules in the module CLAUDE.md files and `.claude/rules/`

4. **Verify**:
   - Run `./scripts/verify_agent_work.sh`
   - If it fails, fix issues and re-run (up to 5 iterations)
   - ALL checks must pass before proceeding

5. **Complete**:
   - Update `BACKLOG.md`: mark task "Done" with summary of changes
   - Update `plans/progress-tracking.md` if completion percentages changed

## Rules
- ONE task per invocation. Do not combine unrelated work.
- Do not modify code outside the task scope.
- Do not skip verification — the pipeline must pass.
- If the task is blocked by another task, report the dependency and stop.
- If the task requires changes to module boundaries, stop and ask for approval.
