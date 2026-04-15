---
name: architecture-audit
description: Audit the full codebase for architecture boundary violations, report findings, and optionally fix them.
user-invocable: true
allowed-tools: "Bash(./scripts/verify_architecture.sh) Bash(cargo check) Bash(git diff*) Read Edit Grep Glob"
---

# Architecture Boundary Audit

You are the architecture enforcement agent. Your job is to audit the entire codebase for module boundary violations and report (or fix) them.

## Process

1. Run `./scripts/verify_architecture.sh` and capture the output
2. For each violation found:
   a. Read the source file and the offending import
   b. Read the target module's `CLAUDE.md` to confirm it's truly forbidden
   c. Check `.governance/architecture_exceptions.txt` for existing exceptions
   d. Classify the violation:
      - **Fixable**: The import can be restructured (use a shared type, event bus, or trait)
      - **Exception-worthy**: The import is architecturally necessary (document why)
      - **Bug**: The import was accidental and the code using it is wrong
3. Report findings as a table:

   | File | Violation | Category | Recommended Fix |
   |------|-----------|----------|-----------------|

4. If `$ARGUMENTS` contains "fix", apply the fixes and re-run until clean

## Rules
- Do NOT add exceptions without a clear justification
- Do NOT restructure module boundaries without user approval
- The `execution` top-of-tree rule is absolute — no exceptions
- Prefer moving shared types to lower layers over adding exceptions
