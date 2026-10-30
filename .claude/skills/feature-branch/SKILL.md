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

1. Determine branch type from the work description. Prefer `<type>/<id>-<slug>`
   where `<id>` is the BACKLOG task id (`B-085` -> `b085`) when one exists:
   - `feat/<id>-<slug>` — New features
   - `fix/<id>-<slug>` — Bug fixes
   - `refactor/<id>-<slug>` — Code restructuring
   - `chore/<id>-<slug>` — Maintenance, CI, docs, tooling
   - `test/<id>-<slug>` — Test additions/improvements
   - `perf/<id>-<slug>` — Performance improvements

2. Stash or commit any uncommitted work on the current branch

3. Switch to `main` and pull latest:
   ```bash
   git checkout main && git pull origin main
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
- ALWAYS branch from an up-to-date `main`
- NEVER commit directly to `main`
- Reference: `.claude/rules/branching.md`
