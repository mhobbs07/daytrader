---
paths:
  - "**/*"
---

# Git Branching Model

One trunk. `main` is the only long-lived branch; there is no integration branch.

## Branch Hierarchy

```
main                       Protected. The only long-lived branch. Requires PR + green CI.
  └─ <type>/<id>-<slug>    One branch per backlog task, cut from main.
       └─ worktree branches  Parallel agent work, merged into the task branch.
```

## Rules

1. **Never commit directly to `main`.** All changes go through PRs.
2. **Every PR targets `main`.** Use `/promote` to create it.
3. **Merging the PR is the release.** There is no staging step.
4. **Task branches are deleted after merge** to keep the repo clean.
5. **Swarm worktree branches** merge to their parent task branch, NOT to `main` directly.
6. **One logical task per branch and per PR.** Do not mix unrelated changes.

## Branch Naming (advisory)

These prefixes are conventions for readability, NOT enforced rules. There is
no CI or hook validation of branch names (a stale `branch-naming.yml` workflow
that enforced a different scheme was removed 2026-07-07 by owner decision —
do not reintroduce enforcement).

Preferred form is `<type>/<id>-<slug>`, where `<id>` is the BACKLOG task id in
lowercase without the hyphen (`B-085` becomes `b085`) and `<slug>` is a few
lowercase words joined by hyphens.

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<id>-<slug>` | `feat/b130-order-history` |
| Bug fix | `fix/<id>-<slug>` | `fix/b085-signal-symbol` |
| Refactor | `refactor/<id>-<slug>` | `refactor/b107-risk-on-core` |
| Chore (CI, docs, tooling) | `chore/<id>-<slug>` | `chore/b087-one-trunk-governance` |
| Test | `test/<id>-<slug>` | `test/b137-scripted-day` |
| Docs | `docs/<id>-<slug>` | `docs/b098-status` |
| Worktree (auto) | `claude/<worktree-name>` | `claude/interesting-neumann` |

## Push Rules

- **Never force-push** to `main` or shared task branches
- **Force-push is acceptable** only on your own unshared branches, and then only with `--force-with-lease`
- **Always use `git push -u origin <branch>`** for first push to set tracking

## PR Requirements

- PR title follows conventional commit format
- PR body includes: summary, verification status, modules changed
- All CI checks must pass before merge
- PRs target `main`
