---
name: promote-to-staging
description: Verify a feature branch, push it, create a PR to staging, and ensure all checks pass before merge. Use when a feature is ready for staging integration.
argument-hint: "<feature-branch> [--auto-merge]"
user-invocable: true
allowed-tools: "Bash(git *) Bash(gh *) Bash(cargo *) Bash(./scripts/*) Read Grep Glob"
---

# Promote Feature Branch to Staging

You are the release gate agent. Your job is to verify a feature branch is clean, push it to GitHub, and create a PR targeting the `staging` branch.

## Branch Model

```
main          ← production (protected, requires PR + review)
  └─ staging  ← integration testing (PR from feature branches)
       └─ feat/<name>  ← feature work (from swarm or direct)
            └─ worktree branches  ← parallel agent work (merged to feat/)
```

## Process

### 1. Parse Arguments

- `$ARGUMENTS` contains the feature branch name (e.g., `feat/order-history`)
- If `--auto-merge` is present, merge the PR after checks pass (if user has permission)

### 2. Pre-Flight Checks

Before pushing anything, verify the branch is clean:

```bash
# Ensure we're on the feature branch
git checkout <feature-branch>

# Ensure no uncommitted changes
git status --porcelain

# Run full verification
./scripts/verify_agent_work.sh
```

If verification fails, STOP and report. Do not push broken code.

### 3. Ensure Staging Branch Exists

```bash
# Check if staging exists remotely
git fetch origin
git branch -r | grep origin/staging

# If staging doesn't exist, create it from main
# (Ask user for confirmation first)
git checkout -b staging origin/main
git push -u origin staging
```

### 4. Rebase on Staging

Ensure the feature branch is up to date with staging:

```bash
git checkout <feature-branch>
git fetch origin staging
git rebase origin/staging
```

If rebase conflicts:
1. List the conflicting files
2. Attempt to resolve automatically
3. If resolution is ambiguous, STOP and ask the user
4. After resolving: `git rebase --continue`

### 5. Push and Create PR

```bash
# Push the feature branch
git push -u origin <feature-branch>

# Create PR targeting staging
gh pr create \
  --base staging \
  --head <feature-branch> \
  --title "<pr-title>" \
  --body "$(cat <<'PREOF'
## Summary
<bullet points of what this feature adds>

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
Generated with `/promote-to-staging`
PREOF
)"
```

### 6. Wait for CI (if configured)

```bash
# Check if there are any required status checks
gh pr checks <pr-number> --watch
```

### 7. Auto-Merge (if requested)

If `--auto-merge` was specified and all checks pass:

```bash
gh pr merge <pr-number> --merge --delete-branch
```

Otherwise, report the PR URL and let the user decide.

### 8. Report

Output:
- PR URL
- Branch status (ahead/behind staging)
- Verification results
- List of commits included
- Whether auto-merge was applied

## Rules

- NEVER force-push to staging or main
- NEVER merge a PR that hasn't passed `verify_agent_work.sh`
- NEVER skip the rebase step — staging must have linear-ish history
- If the feature branch has merge commits from swarm worktrees, that's fine — use `--merge` not `--rebase` for the PR
- Always delete the feature branch after merge (keep git clean)
- PR titles follow conventional commit format: `feat(scope): description`
