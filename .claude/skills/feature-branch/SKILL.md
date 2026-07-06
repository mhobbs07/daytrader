---
model: default
description: Create a feature branch with proper workflow and naming
---

# /feature-branch

Creates a properly named feature branch following project branching rules.

## Current State
Branch: `!git branch --show-current`
Status: `!git status --short | head -5`

## Steps

1. Determine branch type from the work description:
   - `feat/<name>` — New features
   - `fix/<name>` — Bug fixes
   - `refactor/<name>` — Code restructuring
   - `chore/<name>` — Maintenance, CI, docs, tooling
   - `test/<name>` — Test additions/improvements
   - `perf/<name>` — Performance improvements

2. Stash or commit any uncommitted work on the current branch

3. Switch to `staging` and pull latest:
   ```bash
   git checkout staging && git pull origin staging
   ```

4. Create the feature branch:
   ```bash
   git checkout -b {type}/{descriptive-name}
   ```

5. Push with upstream tracking:
   ```bash
   git push --set-upstream origin {type}/{descriptive-name}
   ```

## Rules
- Branch names: lowercase, hyphens only, max 50 chars
- NEVER branch directly from `main` — always from `staging`
- NEVER commit directly to `staging` or `main`
- Reference: `.claude/rules/branching.md`
