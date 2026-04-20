#!/usr/bin/env bash
# =============================================================================
# Pre-Edit Hook: Module Boundary Guard
# Inspects Edit/Write tool input to detect forbidden cross-module imports
# before the edit is applied.
#
# Receives tool input as JSON on stdin.
# Exit 0 = allow, Exit 2 = block
# =============================================================================

INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
NEW_CONTENT=$(echo "$INPUT" | jq -r '.tool_input.new_string // .tool_input.content // empty')

# Only check Rust source files
if [[ ! "$FILE_PATH" =~ \.rs$ ]]; then
  exit 0
fi

# Only check files in src/
if [[ ! "$FILE_PATH" =~ /src/ ]]; then
  exit 0
fi

# Extract the module being edited (first directory under src/)
MODULE=$(echo "$FILE_PATH" | sed -n 's|.*/src/\([^/]*\)/.*|\1|p')
if [ -z "$MODULE" ]; then
  exit 0
fi

# Skip if no new content to check (e.g., deletion)
if [ -z "$NEW_CONTENT" ]; then
  exit 0
fi

# Get forbidden imports for this module (portable — no associative arrays)
get_forbidden() {
  case "$1" in
    utils)          echo "risk|orders|positions|execution|patterns|events|config|db" ;;
    domain)         echo "orders|risk|patterns|execution|positions|events|config|db|brokers|utils" ;;
    events)         echo "risk|orders|positions|execution|patterns|config|db" ;;
    config)         echo "orders|positions|execution|patterns|events|db" ;;
    db)             echo "risk|orders|positions|execution" ;;
    risk)           echo "orders|positions|execution|patterns" ;;
    patterns)       echo "orders|positions|execution|risk" ;;
    positions)      echo "execution|risk|patterns" ;;
    orders)         echo "execution|patterns" ;;
    infrastructure) echo "risk|orders|positions|execution|patterns|events" ;;
    *)              echo "" ;;
  esac
}

FORBIDDEN_PATTERN=$(get_forbidden "$MODULE")

# If module has no restrictions defined, allow
if [ -z "$FORBIDDEN_PATTERN" ]; then
  exit 0
fi

# Check if new content introduces a forbidden import
VIOLATIONS=$(echo "$NEW_CONTENT" | grep -E "use crate::(${FORBIDDEN_PATTERN})" 2>/dev/null || true)

if [ -n "$VIOLATIONS" ]; then
  # Find the project root by looking for .governance/ relative to the file
  PROJECT_DIR=$(echo "$FILE_PATH" | sed 's|/src/.*||')
  EXCEPTIONS_FILE="${PROJECT_DIR}/.governance/architecture_exceptions.txt"

  BLOCKED=""
  while IFS= read -r violation; do
    TARGET=$(echo "$violation" | sed -n 's|.*use crate::\([a-z_]*\).*|\1|p')
    if [ -n "$TARGET" ]; then
      # Check if exception exists
      if [ -f "$EXCEPTIONS_FILE" ] && grep -q "^${MODULE} -> ${TARGET}" "$EXCEPTIONS_FILE" 2>/dev/null; then
        continue  # Exception exists, allow
      fi
      BLOCKED="${BLOCKED}${violation}\n"
    fi
  done <<< "$VIOLATIONS"

  if [ -n "$BLOCKED" ]; then
    jq -n --arg module "$MODULE" --arg violations "$BLOCKED" '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: ("ARCHITECTURE VIOLATION: Module \"" + $module + "\" cannot import these modules. Check src/" + $module + "/CLAUDE.md for allowed dependencies.\nViolating imports:\n" + $violations)
      }
    }'
    exit 0
  fi
fi

exit 0
