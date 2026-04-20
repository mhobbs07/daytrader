#!/usr/bin/env bash
# =============================================================================
# SessionStart Hook: Project Status Dashboard
# Displays branch, pending changes, available skills, and worktree status
# when a new Claude Code session begins.
# =============================================================================

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
cd "$PROJECT_DIR" 2>/dev/null || exit 0

# Gather project state
BRANCH=$(git branch --show-current 2>/dev/null || echo "detached")
DIRTY=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
AHEAD=$(git rev-list --count @{upstream}..HEAD 2>/dev/null || echo "?")
BEHIND=$(git rev-list --count HEAD..@{upstream} 2>/dev/null || echo "?")
LAST_COMMIT=$(git log -1 --format="%h %s (%cr)" 2>/dev/null || echo "none")
WORKTREE_COUNT=$(git worktree list 2>/dev/null | wc -l | tr -d ' ')

# Check for stale worktrees
STALE_WT=0
while IFS= read -r line; do
  if [[ "$line" =~ ^worktree\ (.+) ]]; then
    WT="${BASH_REMATCH[1]}"
    [ "$WT" = "$(git rev-parse --show-toplevel 2>/dev/null)" ] && continue
    WT_DIRTY=$(git -C "$WT" status --porcelain 2>/dev/null | head -1 || true)
    [ -z "$WT_DIRTY" ] && STALE_WT=$((STALE_WT + 1))
  fi
done <<< "$(git worktree list --porcelain 2>/dev/null)"

# Build status message
STATUS="PROJECT STATUS — DayTrader"
STATUS="${STATUS}\n  Branch: ${BRANCH}"
STATUS="${STATUS}\n  Last commit: ${LAST_COMMIT}"
STATUS="${STATUS}\n  Uncommitted files: ${DIRTY}"
STATUS="${STATUS}\n  Ahead/behind upstream: +${AHEAD}/-${BEHIND}"
STATUS="${STATUS}\n  Active worktrees: ${WORKTREE_COUNT}"
[ "$STALE_WT" -gt 0 ] && STATUS="${STATUS}\n  Stale worktrees: ${STALE_WT} (run .claude/hooks/swarm-worktree-cleanup.sh --clean)"
STATUS="${STATUS}\n"
STATUS="${STATUS}\nAVAILABLE SKILLS:"
STATUS="${STATUS}\n  /verify-commit        — Run verification, auto-fix until green"
STATUS="${STATUS}\n  /fix-module <name>    — Fix all issues in a module"
STATUS="${STATUS}\n  /architecture-audit   — Audit module boundaries"
STATUS="${STATUS}\n  /backlog-task <id>    — Pick up and complete a backlog task"
STATUS="${STATUS}\n  /swarm <feat> <desc>  — Parallel agents across layers"
STATUS="${STATUS}\n  /promote-to-staging   — PR feature branch to staging"
STATUS="${STATUS}\n"
STATUS="${STATUS}\nREMINDERS:"
STATUS="${STATUS}\n  - Read BACKLOG.md before starting work"
STATUS="${STATUS}\n  - Read src/<module>/CLAUDE.md for boundary rules"
STATUS="${STATUS}\n  - Run ./scripts/verify_agent_work.sh before any commit"

jq -n --arg status "$STATUS" '{
  systemMessage: $status
}'

exit 0
