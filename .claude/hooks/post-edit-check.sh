#!/usr/bin/env bash
# =============================================================================
# Post-Edit Hook: Quality Check
# After any Edit/Write to a .rs file, checks for common violations:
# - unsafe code
# - unwrap()/expect() in production code
# - raw f64 for monetary values
# - assert!(true) in tests
#
# Receives tool result as JSON on stdin.
# Exit 0 = non-blocking feedback
# =============================================================================

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Only check Rust source files
if [[ ! "$FILE_PATH" =~ \.rs$ ]]; then
  exit 0
fi

if [ ! -f "$FILE_PATH" ]; then
  exit 0
fi

WARNINGS=""

# Find the #[cfg(test)] boundary to separate production from test code
TEST_START=$(grep -n '#\[cfg(test)\]' "$FILE_PATH" 2>/dev/null | head -1 | cut -d':' -f1 || true)

if [ -n "$TEST_START" ]; then
  PROD_CONTENT=$(head -n "$((TEST_START - 1))" "$FILE_PATH" 2>/dev/null)
else
  PROD_CONTENT=$(cat "$FILE_PATH" 2>/dev/null)
fi

# Check for unsafe code in production
if echo "$PROD_CONTENT" | grep -qE '^\s*unsafe\s' 2>/dev/null; then
  WARNINGS="${WARNINGS}\n- UNSAFE CODE detected. This project uses #![forbid(unsafe_code)]."
fi

# Check for .unwrap() in production code (exclude comments)
UNWRAP_COUNT=$(echo "$PROD_CONTENT" | grep -v '^\s*//' | grep -cE '\.(unwrap|expect)\(' 2>/dev/null || echo 0)
if [ "$UNWRAP_COUNT" -gt 0 ]; then
  WARNINGS="${WARNINGS}\n- Found ${UNWRAP_COUNT} .unwrap()/.expect() call(s) in production code. Use proper error handling with ? operator."
fi

# Check for raw f64 in monetary contexts (heuristic: price, amount, balance, cost, pnl)
MONETARY_F64=$(echo "$PROD_CONTENT" | grep -v '^\s*//' | grep -iE '(price|amount|balance|cost|pnl|profit|loss).*:\s*f64' 2>/dev/null || true)
if [ -n "$MONETARY_F64" ]; then
  WARNINGS="${WARNINGS}\n- Raw f64 detected for monetary field. Use Decimal or appropriate newtype (Price, DollarAmount)."
fi

# Check for assert!(true) in test code
if [ -n "$TEST_START" ]; then
  TEST_CONTENT=$(tail -n "+$TEST_START" "$FILE_PATH" 2>/dev/null)
  if echo "$TEST_CONTENT" | grep -q 'assert!(true)' 2>/dev/null; then
    WARNINGS="${WARNINGS}\n- assert!(true) found in tests. Tests must make meaningful assertions."
  fi
fi

# Check for panic!() in production code
if echo "$PROD_CONTENT" | grep -v '^\s*//' | grep -qE 'panic!\(' 2>/dev/null; then
  WARNINGS="${WARNINGS}\n- panic!() detected in production code. Use Result<T, E> error handling instead."
fi

if [ -n "$WARNINGS" ]; then
  jq -n --arg warnings "$WARNINGS" '{
    systemMessage: ("POST-EDIT QUALITY WARNINGS:" + $warnings + "\n\nFix these issues before continuing. These will cause verify_agent_work.sh to fail.")
  }'
fi

exit 0
