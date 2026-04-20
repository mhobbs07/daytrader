#!/usr/bin/env bash
# =============================================================================
# Swarm Worktree Cleanup
# Lists and optionally removes stale worktrees from completed swarm runs.
# Usage: .claude/hooks/swarm-worktree-cleanup.sh [--dry-run|--clean]
# =============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

MODE="${1:---dry-run}"

echo -e "${BLUE}=== Swarm Worktree Status ===${NC}"
echo ""

# List all worktrees
WORKTREES=$(git worktree list --porcelain 2>/dev/null || true)
if [ -z "$WORKTREES" ]; then
  echo -e "${GREEN}No worktrees found.${NC}"
  exit 0
fi

STALE=0
ACTIVE=0

while IFS= read -r line; do
  if [[ "$line" =~ ^worktree\ (.+) ]]; then
    WT_PATH="${BASH_REMATCH[1]}"
    # Skip main worktree
    if [ "$WT_PATH" = "$(git rev-parse --show-toplevel 2>/dev/null)" ]; then
      continue
    fi

    # Get branch
    WT_BRANCH=$(git -C "$WT_PATH" branch --show-current 2>/dev/null || echo "detached")

    # Check if there are uncommitted changes
    WT_DIRTY=$(git -C "$WT_PATH" status --porcelain 2>/dev/null | head -1 || true)

    # Check age of last commit
    LAST_COMMIT_AGE=$(git -C "$WT_PATH" log -1 --format="%cr" 2>/dev/null || echo "unknown")

    if [ -n "$WT_DIRTY" ]; then
      echo -e "  ${YELLOW}DIRTY${NC}  $WT_PATH ($WT_BRANCH) — has uncommitted changes"
      ACTIVE=$((ACTIVE + 1))
    else
      echo -e "  ${GREEN}CLEAN${NC}  $WT_PATH ($WT_BRANCH) — last commit: $LAST_COMMIT_AGE"
      STALE=$((STALE + 1))

      if [ "$MODE" = "--clean" ]; then
        echo -e "    ${RED}Removing worktree: $WT_PATH${NC}"
        git worktree remove "$WT_PATH" --force 2>/dev/null || true
        # Delete the branch if it was merged
        if git branch --merged 2>/dev/null | grep -q "$WT_BRANCH"; then
          git branch -d "$WT_BRANCH" 2>/dev/null || true
          echo -e "    ${RED}Deleted merged branch: $WT_BRANCH${NC}"
        fi
      fi
    fi
  fi
done <<< "$WORKTREES"

echo ""
echo -e "${BLUE}Summary:${NC} ${ACTIVE} active, ${STALE} stale (cleanable)"
if [ "$MODE" = "--dry-run" ] && [ "$STALE" -gt 0 ]; then
  echo -e "${YELLOW}Run with --clean to remove stale worktrees.${NC}"
fi
