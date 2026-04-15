#!/usr/bin/env bash
# =============================================================================
# SessionEnd Hook: Auto-Cleanup
# Checks for uncommitted work, stale worktrees, and reports session summary.
# =============================================================================

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
cd "$PROJECT_DIR" 2>/dev/null || exit 0

WARNINGS=""

# Check for uncommitted changes
DIRTY=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
if [ "$DIRTY" -gt 0 ]; then
  WARNINGS="${WARNINGS}\n- WARNING: ${DIRTY} uncommitted file(s). Your changes may be lost."
fi

# Check for unmerged worktree branches
while IFS= read -r line; do
  if [[ "$line" =~ ^worktree\ (.+) ]]; then
    WT="${BASH_REMATCH[1]}"
    [ "$WT" = "$(git rev-parse --show-toplevel 2>/dev/null)" ] && continue
    WT_BRANCH=$(git -C "$WT" branch --show-current 2>/dev/null || echo "unknown")
    WT_DIRTY=$(git -C "$WT" status --porcelain 2>/dev/null | head -1 || true)
    if [ -n "$WT_DIRTY" ]; then
      WARNINGS="${WARNINGS}\n- WARNING: Worktree '${WT}' (${WT_BRANCH}) has uncommitted changes."
    fi
  fi
done <<< "$(git worktree list --porcelain 2>/dev/null)"

if [ -n "$WARNINGS" ]; then
  jq -n --arg warnings "$WARNINGS" '{
    systemMessage: ("SESSION END WARNINGS:" + $warnings)
  }'
fi

exit 0
