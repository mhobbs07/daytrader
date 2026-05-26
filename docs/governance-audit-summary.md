# Governance Audit Summary

## Overview

Comprehensive audit and enhancement of the `.claude/` code governance directory,
conducted on 2026-05-26. The existing system was graded A (Excellent) — these
changes address identified gaps without disrupting the existing defense-in-depth
architecture.

## Changes Made

### Hooks Enhanced (3 changes)

| File | Change | Why |
|------|--------|-----|
| `guard-dangerous-commands.sh` | Added `--no-verify` warning | Closes governance bypass via `git commit --no-verify` |
| `guard-module-boundaries.sh` | Added qualified path detection | Catches `crate::forbidden_module::Type` without `use` statements |
| `guard-pdt-compliance.sh` (NEW) | PDT bypass detection | SEC-mandated PDT compliance had no automated enforcement |

### Rules Added (3 new files)

| File | Scope | Purpose |
|------|-------|---------|
| `documentation.md` | `src/**/*.rs` | Doc comment standards for public APIs |
| `dependencies.md` | `**/Cargo.toml` | Dependency management (pinning, auditing, justification) |
| `mcp-development.md` | `mcp-servers/**/*` | MCP server security and development patterns |

### Skills Added (2 new files)

| File | Purpose |
|------|---------|
| `create-mcp-server/SKILL.md` | Scaffold MCP servers following project conventions |
| `feature-branch/SKILL.md` | Proper feature branch workflow with naming rules |

### Settings Fixed

| File | Fix |
|------|-----|
| `settings.local.json` | Removed stale `sed` command; fixed `//tmp/**` → `/tmp/**` |
| `settings.json` | Registered `guard-pdt-compliance.sh` hook |

## Audit Findings (Addressed)

- **PDT compliance** had no automated hook → Now has `guard-pdt-compliance.sh`
- **`--no-verify` bypass** was undetected → Now triggers a warning
- **Module boundary check** missed qualified paths → Now detects `crate::module::` patterns
- **No MCP development rule** → Now has `mcp-development.md`
- **No documentation rule** → Now has `documentation.md`
- **No dependency management rule** → Now has `dependencies.md`
- **No MCP server creation skill** → Now has `create-mcp-server`
- **No feature branch skill** → Now has `feature-branch`
- **Stale settings entries** → Cleaned `settings.local.json`

## Remaining Recommendations (Not Addressed Here)

These were noted by the audit but deferred to future work:

1. **23 known test issues** in `.governance/known_test_issues.txt` — backlog items
2. **Husky v4 → v9 upgrade** — breaking change requiring coordination
3. **Performance/benchmark rule** — lower priority, benchmarks work without governance
4. **Frontend/Tauri rule** — deferred until Tauri development resumes
5. **Observability rule** — deferred until logging infrastructure is established

## Context Window Impact

All new files use path-scoped rules (only loaded when relevant files are touched):
- New rules add ~350 tokens total when all three fire simultaneously
- New skills add 0 tokens until explicitly invoked
- PDT hook adds ~0 tokens (runs silently unless violation detected)
- Net always-on increase: 0 tokens (nothing added to global scope)
