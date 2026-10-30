---
name: promote
description: Verify a branch, push it, create a PR to main, and ensure all checks pass before merge. Use when a task branch is ready to merge.
argument-hint: "<branch> [--auto-merge]"
user-invocable: true
allowed-tools: "Bash(git *) Bash(gh *) Bash(cargo *) Bash(./scripts/*) Read Grep Glob"
---

# Promote a Branch to Main

**Current branch:** !`git branch --show-current`
**Uncommitted files:** !`git status --porcelain | wc -l | tr -d ' '`
**Commits ahead of main:**
```
!git log --oneline origin/main..HEAD 2>/dev/null
```

---

You are the release gate agent. Your job is to verify a branch is clean, push it to GitHub, and create a PR targeting `main`.

## Branch Model

```
main                       <- the only long-lived branch (protected, requires PR + green CI)
  └─ <type>/<id>-<slug>    <- one branch per backlog task (from direct work or swarm)
       └─ worktree branches  <- parallel agent work (merged into the task branch)
```

## Process

### 1. Parse Arguments

- `$ARGUMENTS` contains the branch name (e.g., `fix/b085-signal-symbol`)
- If `--auto-merge` is present, merge the PR after checks pass (if user has permission)

### 2. Pre-Flight Checks

Before pushing anything, verify the branch is clean:

```bash
# Ensure we're on the task branch
git checkout <branch>

# Ensure no uncommitted changes
git status --porcelain

# Run full verification
./scripts/verify_agent_work.sh
```

If verification fails, STOP and report. Do not push broken code.

### 3. Rebase on Main

Ensure the branch is up to date with main:

```bash
git checkout <branch>
git fetch origin main
git rebase origin/main
```

If rebase conflicts:
1. List the conflicting files
2. Attempt to resolve automatically
3. If resolution is ambiguous, STOP and ask the user
4. After resolving: `git rebase --continue`

### 4. Push and Create PR

```bash
# Push the branch (if it was already pushed and you rebased, your own
# unshared branch may use --force-with-lease; never force-push main)
git push -u origin <branch>

# Create PR targeting main
gh pr create \
  --base main \
  --head <branch> \
  --title "<pr-title>" \
  --body "$(cat <<'PREOF'
## Summary
<bullet points of what this change does>

## Verification
- [ ] `verify_agent_work.sh` passed
- [ ] Architecture boundaries respected
- [ ] All tests passing
- [ ] No new clippy warnings

## Modules Changed
<list of modules touched>

## Model Budget
<which agent models were used, if swarm>

---
Generated with `/promote`
PREOF
)"
```

### 5. Wait for CI

```bash
# Check required status checks
gh pr checks <pr-number> --watch
```

### 6. Auto-Merge (if requested)

If `--auto-merge` was specified and all checks pass:

```bash
gh pr merge <pr-number> --merge --delete-branch
```

Otherwise, report the PR URL and let the user decide.

### 7. Report

Output:
- PR URL
- Branch status (ahead/behind main)
- Verification results
- List of commits included
- Whether auto-merge was applied

## Rules

- NEVER force-push main
- NEVER merge a PR that hasn't passed `verify_agent_work.sh`
- NEVER skip the rebase step — main must have linear-ish history
- If the branch has merge commits from swarm worktrees, that's fine — use `--merge` not `--rebase` for the PR
- Always delete the branch after merge (keep git clean)
- PR titles follow conventional commit format: `feat(scope): description`
