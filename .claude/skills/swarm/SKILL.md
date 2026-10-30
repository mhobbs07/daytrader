---
name: swarm
description: Orchestrate parallel worktree agents across architectural layers on a shared feature branch. Use when a feature spans db, backend, and/or frontend layers.
argument-hint: "<feature-name> <description>"
user-invocable: true
allowed-tools: "Bash(git *) Bash(cargo *) Bash(./scripts/*) Read Edit Write Grep Glob Agent"
---

# Swarm: Multi-Layer Feature Development

## Current Project State

**Branch:** !`git branch --show-current`
**Uncommitted files:** !`git status --porcelain | wc -l | tr -d ' '`
**Recent commits:**
```
!git log --oneline -5
```
**Existing worktrees:**
```
!git worktree list
```
**Architecture exceptions:**
```
!cat .governance/architecture_exceptions.txt 2>/dev/null | grep -v '^#' | grep -v '^$'
```

---

You are the swarm orchestrator. Your job is to coordinate parallel agents working on different architectural layers of a single feature, each in their own worktree, merging back to a shared feature branch.

## Model Budget Policy

Costs must be controlled. Each agent tier has a model assignment:

| Agent Role | Model | Rationale |
|------------|-------|-----------|
| **Orchestrator** (you) | opus | Needs full architectural understanding to coordinate |
| **Backend agent** | sonnet | Complex business logic |
| **Database agent** | sonnet | Schema design needs good reasoning |
| **Frontend agent** | sonnet | Haiku is prohibited in this project (see .claude/rules/agent-models.md) |
| **Test/verify agent** | sonnet | Haiku is prohibited in this project |

Override these defaults ONLY if the user explicitly requests it. **Never** pass `model="haiku"`.

## Process

### 1. Parse the Request

Extract from `$ARGUMENTS`:
- **Feature name**: Used for branch naming (e.g., `order-history`)
- **Description**: What the feature does across layers

### 2. Create the Feature Branch

```bash
git checkout -b feat/<feature-name>
```

If the branch already exists, check it out instead.

### 3. Plan the Work

Before spawning agents, analyze the feature and determine:
- Which layers are affected (db, backend, frontend, or subset)
- What each layer needs to do
- What shared types or interfaces the layers need to agree on
- The order of dependencies (db schema first? shared types first?)

Present the plan to the user and get approval before spawning agents.

### 4. Spawn Layer Agents

Spawn agents in parallel using the Agent tool with `isolation: "worktree"`. Each agent gets:
- A clear, self-contained task description
- The shared type contracts they must implement
- Their module boundary rules (reference the module's CLAUDE.md)
- The `model` parameter matching the budget policy above

**Example spawn pattern** (adapt to the actual feature):

```
Agent 1 (database): model=sonnet, isolation=worktree
  "Implement the database schema and repository for <feature>.
   Create migration, models, and repository methods in src/db/.
   Follow src/db/CLAUDE.md boundary rules.
   Commit with: feat(db): <description>"

Agent 2 (backend): model=sonnet, isolation=worktree
  "Implement the business logic for <feature>.
   Work in src/<relevant-modules>/ (risk, orders, patterns, etc).
   Follow each module's CLAUDE.md boundary rules.
   Commit with: feat(<module>): <description>"

Agent 3 (frontend): model=sonnet, isolation=worktree
  "Implement the Tauri frontend components for <feature>.
   Work in daytrader/src-tauri/.
   Commit with: feat(ui): <description>"
```

### 5. Monitor and Collect Results

Wait for all agents to complete. Each agent returns:
- Its worktree path and branch name
- A summary of changes made
- Whether it committed successfully

### 6. Merge Branches Back

For each completed agent, merge its worktree branch back to the feature branch:

```bash
git checkout feat/<feature-name>
git merge --no-ff <worktree-branch-1> -m "merge(db): <feature> database layer"
git merge --no-ff <worktree-branch-2> -m "merge(backend): <feature> business logic"
git merge --no-ff <worktree-branch-3> -m "merge(ui): <feature> frontend"
```

If a merge conflicts:
1. Read the conflicting files
2. Resolve based on the feature plan
3. Commit the resolution

### 7. Integration Verification

After all merges, run the full verification:

```bash
cargo check
cargo test --lib
cargo clippy -- -D warnings
./scripts/verify_architecture.sh
```

If anything fails, fix it on the feature branch directly.

### 8. Report

Summarize:
- Feature branch name
- Agents spawned and their models
- What each agent implemented
- Merge status (clean or conflicts resolved)
- Verification results
- Estimated token cost tier (low/medium/high based on agent models used)

## Mandatory Agent Prompt Boilerplate

Every spawned agent's prompt MUST include the following block verbatim. These
are not style preferences — each one corresponds to a failure observed in a
real swarm run on 2026-07-28 where all three agents finished with **zero
commits** and substantial work stranded in worktrees.

```
COMMIT DISCIPLINE (read before you start working):
1. Commit EARLY and commit OFTEN. An uncommitted change in a worktree is one
   cleanup away from being lost. Commit as soon as a coherent unit of work
   compiles — do not wait until everything is perfect.
2. NEVER run a long verification (verify_agent_work.sh, full cargo test,
   cargo bench, cargo test --doc) in the background and then stop your turn.
   Stopping with a background job pending ends your turn with the work
   UNCOMMITTED. Run verification in the FOREGROUND, or commit first and
   verify afterwards.
3. Verification is not a precondition for committing. Commit, then verify,
   then commit the fixes. Never the reverse.
4. `cargo test --doc` is NOT part of the gate (backlog B-068). Never block on it.
5. Scope your test runs. `cargo test --lib <module>` is usually enough; the
   full workspace build across the Tauri sub-crate takes minutes.

SHELL ENVIRONMENT:
- Every Bash call is a FRESH shell. `export` does NOT persist between calls.
- Chain env with the command in the SAME call:
  `export DATABASE_URL="sqlite://$(pwd)/data/db/daytrader.db" && cargo test --lib`
- sqlx uses compile-time-checked macros; a wrong DATABASE_URL produces
  confusing "unable to open database" errors from cargo check and from the
  pre-commit hook.

HONESTY REQUIREMENTS:
- If a test cannot be made to pass, LEAVE IT FAILING and report exactly what
  fails and why. Never delete a test or weaken an assertion to reach green.
- If you cannot verify an external API's shape, implement against the
  documented shape, mark the live test `#[ignore]`d with the reason, and SAY
  SO in your report. Do not present a guess as verified.
- Report measured numbers, not claims. "Benchmarks pass" is not a result;
  "hammer_detection: 1.28 µs" is.
- If something in your task description turns out to be wrong, say so.
```

## Rules

- ALWAYS get user approval on the plan before spawning agents
- ALWAYS use the model budget policy (sonnet for all subagents, opus only for orchestration — haiku is prohibited)
- ALWAYS include the Mandatory Agent Prompt Boilerplate above in every agent prompt
- Each agent MUST commit before its worktree is cleaned up — verify this yourself
  with `git -C <worktree> log` rather than trusting the agent's report
- Each agent MUST respect its module's CLAUDE.md boundary rules
- Do NOT spawn more than 4 agents at once (cost control)
- If only one layer is affected, do NOT use a swarm — just do the work directly
- Reserve `BACKLOG.md`, `.governance/*`, and `scripts/verify_agent_work.sh` to
  yourself. Three agents editing the same file is a guaranteed merge conflict;
  update them centrally after merging.
- Assign migration numbers up front if more than one agent adds a migration
- Clean up worktrees after successful merge

## Orchestrator Responsibilities

- **Verify the worktree base.** `worktree.baseRef` is set to `head` in
  `.claude/settings.json` so agents inherit the current branch. If that setting
  is ever removed, worktrees branch from the remote default branch instead and
  agents will work against stale code — one agent in the 2026-07-28 run found
  `src/data/` missing entirely and invented a stub to compensate. Check with
  `git -C <worktree> log --oneline -1` immediately after spawning.
- **Take over rather than re-prompting indefinitely.** If an agent fails to
  commit after two explicit instructions, inspect its worktree and commit the
  work yourself. Two attempts is the limit.
- **Re-verify after merging.** An agent's local verification ran against its own
  base. Rebase onto the feature branch and run the full gate there before
  trusting any result.
