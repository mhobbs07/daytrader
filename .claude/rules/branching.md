---
paths:
  - "**/*"
---

# Git Branching Model

## Branch Hierarchy

```
main              Protected. Production-ready code. Requires PR + review.
  └─ staging      Integration branch. PRs from feature branches land here.
       └─ feat/*  Feature branches. Created per-task or per-swarm.
       └─ fix/*   Bug fix branches.
       └─ refactor/*  Refactoring branches.
```

## Rules

1. **Never commit directly to `main` or `staging`.** All changes go through PRs.
2. **Feature branches target `staging`**, not `main`. Use `/promote-to-staging` to create the PR.
3. **`staging` is merged to `main`** only after integration testing passes. This is a manual/human decision.
4. **Feature branches are deleted after merge** to keep the repo clean.
5. **Swarm worktree branches** merge to their parent feature branch, NOT to staging directly.

## Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<name>` | `feat/order-history` |
| Bug fix | `fix/<name>` | `fix/pdt-calculation` |
| Refactor | `refactor/<name>` | `refactor/risk-engine` |
| Test | `test/<name>` | `test/pattern-coverage` |
| Docs | `docs/<name>` | `docs/api-reference` |
| Worktree (auto) | `claude/<worktree-name>` | `claude/interesting-neumann` |

## Push Rules

- **Never force-push** to `main`, `staging`, or shared feature branches
- **Force-push is acceptable** only on your own unshared worktree branches
- **Always use `git push -u origin <branch>`** for first push to set tracking

## PR Requirements

- PR title follows conventional commit format
- PR body includes: summary, verification status, modules changed
- All CI checks must pass before merge
- Feature PRs target `staging`, not `main`
