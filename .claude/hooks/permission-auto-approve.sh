#!/usr/bin/env bash
# =============================================================================
# PermissionRequest Hook: Auto-Approve Safe Commands
# Automatically approves known-safe tool calls to reduce user friction.
# Returns "allow" for safe patterns, "defer" for everything else.
# =============================================================================

INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // empty')

# Auto-approve read-only and build tools
case "$TOOL_NAME" in
  Read|Glob|Grep)
    jq -n '{
      hookSpecificOutput: {
        hookEventName: "PermissionRequest",
        permissionDecision: "allow",
        permissionDecisionReason: "Read-only tool, auto-approved"
      }
    }'
    exit 0
    ;;
  Bash)
    COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

    # Auto-approve safe cargo commands
    if echo "$COMMAND" | grep -qE '^cargo (check|test|build|fmt|clippy|bench|doc|audit)' 2>/dev/null; then
      jq -n --arg cmd "$COMMAND" '{
        hookSpecificOutput: {
          hookEventName: "PermissionRequest",
          permissionDecision: "allow",
          permissionDecisionReason: ("Safe cargo command auto-approved: " + $cmd)
        }
      }'
      exit 0
    fi

    # Auto-approve read-only git commands
    if echo "$COMMAND" | grep -qE '^git (status|log|diff|branch|show|rev-parse|worktree list)' 2>/dev/null; then
      jq -n --arg cmd "$COMMAND" '{
        hookSpecificOutput: {
          hookEventName: "PermissionRequest",
          permissionDecision: "allow",
          permissionDecisionReason: ("Read-only git command auto-approved: " + $cmd)
        }
      }'
      exit 0
    fi

    # Auto-approve verification scripts
    if echo "$COMMAND" | grep -qE '^\./scripts/verify_' 2>/dev/null; then
      jq -n --arg cmd "$COMMAND" '{
        hookSpecificOutput: {
          hookEventName: "PermissionRequest",
          permissionDecision: "allow",
          permissionDecisionReason: ("Verification script auto-approved: " + $cmd)
        }
      }'
      exit 0
    fi

    # Auto-approve ls, wc, mkdir for reports
    if echo "$COMMAND" | grep -qE '^(ls|wc|mkdir -p reports)' 2>/dev/null; then
      jq -n '{
        hookSpecificOutput: {
          hookEventName: "PermissionRequest",
          permissionDecision: "allow",
          permissionDecisionReason: "Safe filesystem command auto-approved"
        }
      }'
      exit 0
    fi
    ;;
esac

# Everything else: defer to default permission handling
jq -n '{
  hookSpecificOutput: {
    hookEventName: "PermissionRequest",
    permissionDecision: "defer",
    permissionDecisionReason: "Not in auto-approve list, deferring to user"
  }
}'
exit 0
