#!/usr/bin/env bash
# =============================================================================
# Post-Edit Hook: PDT Compliance Guard
# Warns when edits to risk/ or execution/ files appear to bypass or disable
# Pattern Day Trading (PDT) compliance checks. SEC-mandated — defense-in-depth.
#
# Receives tool input as JSON on stdin.
# Exit 0 = allow (with optional warning)
# =============================================================================

INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
NEW_CONTENT=$(echo "$INPUT" | jq -r '.tool_input.new_string // .tool_input.content // empty')

# Only check Edit and Write tool events
if [[ "$TOOL_NAME" != "Edit" && "$TOOL_NAME" != "Write" ]]; then
  exit 0
fi

# Only check Rust source files in src/risk/ or src/execution/
if [[ ! "$FILE_PATH" =~ \.rs$ ]]; then
  exit 0
fi

if [[ ! "$FILE_PATH" =~ src/risk/ && ! "$FILE_PATH" =~ src/execution/ ]]; then
  exit 0
fi

# Skip if no new content to check
if [ -z "$NEW_CONTENT" ]; then
  exit 0
fi

WARNINGS=""

# Check 1: Commenting out or removing PDT-related function calls
# Look for commented-out pdt/PDT references (e.g., "// check_pdt", "// pdt_compliance")
COMMENTED_PDT=$(echo "$NEW_CONTENT" | grep -iE '//\s*(check_pdt|validate_pdt|pdt_compliance|pdt_check|enforce_pdt|is_pdt)' 2>/dev/null || true)
if [ -n "$COMMENTED_PDT" ]; then
  WARNINGS="${WARNINGS}Commented-out PDT function call(s) detected.\n"
fi

# Check 2: Setting PDT-related booleans to hardcoded false
HARDCODED_FALSE=$(echo "$NEW_CONTENT" | grep -iE '(pdt_check|is_pdt_restricted|pdt_enabled|pdt_enforced|pdt_compliance|skip_pdt|bypass_pdt)\s*=\s*false' 2>/dev/null || true)
if [ -n "$HARDCODED_FALSE" ]; then
  WARNINGS="${WARNINGS}PDT-related boolean set to hardcoded false.\n"
fi

# Check 3: Removing/skipping PDT validation function invocations
# Detect patterns where PDT checks might be bypassed with early returns or skips
BYPASS_PATTERNS=$(echo "$NEW_CONTENT" | grep -iE '(return\s+Ok|return\s+true).*//.*pdt|skip.*pdt|bypass.*pdt|disable.*pdt' 2>/dev/null || true)
if [ -n "$BYPASS_PATTERNS" ]; then
  WARNINGS="${WARNINGS}Potential PDT bypass pattern detected (early return or skip).\n"
fi

# Check 4: Look for TODO/FIXME/HACK comments related to PDT
HACK_PDT=$(echo "$NEW_CONTENT" | grep -iE '(TODO|FIXME|HACK|XXX).*pdt' 2>/dev/null || true)
if [ -n "$HACK_PDT" ]; then
  WARNINGS="${WARNINGS}TODO/FIXME/HACK comment related to PDT compliance found.\n"
fi

# If any warnings were found, emit a warning (not a block)
if [ -n "$WARNINGS" ]; then
  jq -n --arg file "$FILE_PATH" --arg warnings "$WARNINGS" '{
    hookSpecificOutput: {
      hookEventName: "PostToolUse",
      notification: ("⚠️ PDT COMPLIANCE WARNING in " + $file + ":\n" + $warnings + "PDT compliance is SEC-mandated. Ensure these changes do not bypass Pattern Day Trading rules. See docs/pdt-compliance-checklist.md.")
    }
  }'
  exit 0
fi

exit 0
