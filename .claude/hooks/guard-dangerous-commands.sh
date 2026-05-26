#!/usr/bin/env bash
# =============================================================================
# Pre-Bash Hook: Dangerous Command Guard
# Blocks destructive or unsafe shell commands before execution.
#
# Receives tool input as JSON on stdin.
# Exit 0 = allow, Exit 2 = block
# =============================================================================

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

if [ -z "$COMMAND" ]; then
  exit 0
fi

# Block patterns that could damage the project
BLOCKED_PATTERNS=(
  "rm -rf /"
  "rm -rf \."
  "rm -rf \*"
  "> /dev/"
  "mkfs\."
  "dd if="
  ":(){.*};"
  "chmod -R 777"
  "git push.*--force.*main"
  "git push.*--force.*master"
  "git reset --hard"
  "cargo publish"
  "curl.*\|.*sh"
  "wget.*\|.*sh"
)

for pattern in "${BLOCKED_PATTERNS[@]}"; do
  if echo "$COMMAND" | grep -qE "$pattern" 2>/dev/null; then
    jq -n --arg cmd "$COMMAND" --arg pattern "$pattern" '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: ("BLOCKED: Dangerous command detected matching pattern: " + $pattern + "\nCommand: " + $cmd)
      }
    }'
    exit 0
  fi
done

# Warn (but allow) for git push to any branch
if echo "$COMMAND" | grep -qE "^git push" 2>/dev/null; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "ask",
      permissionDecisionReason: "Git push requires explicit user approval."
    }
  }'
  exit 0
fi

# Warn when --no-verify or -n flag is used with git commit (bypasses pre-commit hooks)
if echo "$COMMAND" | grep -qE "git commit.*(--no-verify|-n( |$))" 2>/dev/null; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "ask",
      permissionDecisionReason: "⚠️ --no-verify bypasses pre-commit hooks (commitlint + Husky). Ensure verify_agent_work.sh has been run manually."
    }
  }'
  exit 0
fi

# Warn for git commit without running verification first
if echo "$COMMAND" | grep -qE "^git commit" 2>/dev/null; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      additionalContext: "REMINDER: You MUST run ./scripts/verify_agent_work.sh before committing. If you have not done so, abort this commit and run verification first."
    }
  }'
  exit 0
fi

exit 0
