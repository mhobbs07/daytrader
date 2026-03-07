#!/usr/bin/env bash
# =============================================================================
# Test Quality Verification
# Detects trivial, fake, or placeholder tests that don't verify real behavior.
# Uses a baseline file to exempt known pre-existing issues.
# =============================================================================
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Ensure we're at the project root
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}ERROR: Must be run from project root (where Cargo.toml is)${NC}"
    exit 1
fi

BASELINE_FILE=".governance/known_test_issues.txt"
NEW_ISSUES=0
KNOWN_ISSUES=0
WARNINGS=0

echo -e "${BLUE}=== Test Quality Verification ===${NC}"
echo ""

# Helper: check if a file:pattern combo is in the baseline
is_known() {
    local file="$1"
    local pattern="$2"
    if [ ! -f "$BASELINE_FILE" ]; then
        return 1
    fi
    # Check if the baseline contains this file and pattern
    grep -qF "${file}:${pattern}" "$BASELINE_FILE" 2>/dev/null
}

# ---------------------------------------------------------------------------
# Check 1: assert!(true) — always passes, tests nothing
# ---------------------------------------------------------------------------
echo -e "${BLUE}[1/5] Checking for assert!(true)...${NC}"
while IFS=: read -r file line_num content; do
    [ -z "${file:-}" ] && continue
    if is_known "$file" "assert!(true)"; then
        KNOWN_ISSUES=$((KNOWN_ISSUES + 1))
        echo -e "  ${YELLOW}KNOWN: ${file}:${line_num} (baselined)${NC}"
    else
        NEW_ISSUES=$((NEW_ISSUES + 1))
        echo -e "  ${RED}NEW ISSUE: ${file}:${line_num}: ${content}${NC}"
    fi
done < <(grep -rn 'assert!(true)' src/ tests/ 2>/dev/null || true)

# ---------------------------------------------------------------------------
# Check 2: assert!(false, ...) — placeholder tests
# ---------------------------------------------------------------------------
echo -e "${BLUE}[2/5] Checking for assert!(false, ...) placeholders...${NC}"
while IFS=: read -r file line_num content; do
    [ -z "${file:-}" ] && continue
    if is_known "$file" "assert!(false"; then
        KNOWN_ISSUES=$((KNOWN_ISSUES + 1))
        echo -e "  ${YELLOW}KNOWN: ${file}:${line_num} (baselined)${NC}"
    else
        NEW_ISSUES=$((NEW_ISSUES + 1))
        echo -e "  ${RED}NEW ISSUE: ${file}:${line_num}: ${content}${NC}"
    fi
done < <(grep -rn 'assert!(false' src/ tests/ 2>/dev/null || true)

# ---------------------------------------------------------------------------
# Check 3: TODO/FIXME/HACK in test/source code
# ---------------------------------------------------------------------------
echo -e "${BLUE}[3/5] Checking for TODO/FIXME/HACK in code...${NC}"
TODO_COUNT=$(grep -rn 'TODO\|FIXME\|HACK' src/ tests/ --include="*.rs" 2>/dev/null | grep -v '^#' | wc -l | tr -d ' ')
if [ "$TODO_COUNT" -gt 0 ]; then
    WARNINGS=$((WARNINGS + TODO_COUNT))
    echo -e "  ${YELLOW}WARNING: Found ${TODO_COUNT} TODO/FIXME/HACK comments in source/test code${NC}"
    grep -rn 'TODO\|FIXME\|HACK' src/ tests/ --include="*.rs" 2>/dev/null | head -10
    if [ "$TODO_COUNT" -gt 10 ]; then
        echo -e "  ${YELLOW}  ... and $((TODO_COUNT - 10)) more${NC}"
    fi
else
    echo -e "  ${GREEN}No TODO/FIXME/HACK comments found${NC}"
fi

# ---------------------------------------------------------------------------
# Check 4: Empty test functions (no assertions)
# ---------------------------------------------------------------------------
echo -e "${BLUE}[4/5] Checking for test functions with no assertions...${NC}"
EMPTY_TESTS=0
while IFS=: read -r file line_num _; do
    [ -z "${file:-}" ] && continue
    # Read the next 50 lines after the test attribute to find the function body
    BODY=$(sed -n "${line_num},$((line_num + 50))p" "$file" 2>/dev/null || true)
    # Check if body contains any assertion or panic
    if ! echo "$BODY" | grep -qE 'assert|panic!|should_panic|\.expect\(|\.unwrap\(\)'; then
        # Check if it's a setup/smoke test that calls a function (acceptable)
        if ! echo "$BODY" | grep -qE '\.await|::new\(|::create\(|::build\('; then
            EMPTY_TESTS=$((EMPTY_TESTS + 1))
            FUNC_NAME=$(echo "$BODY" | grep -m1 'fn ' | sed 's/.*fn \([a-z_]*\).*/\1/' || echo "unknown")
            echo -e "  ${YELLOW}WARNING: ${file}:${line_num} - fn ${FUNC_NAME} may have no assertions${NC}"
        fi
    fi
done < <(grep -rn '#\[test\]\|#\[tokio::test\]' src/ tests/ --include="*.rs" 2>/dev/null || true)

if [ "$EMPTY_TESTS" -eq 0 ]; then
    echo -e "  ${GREEN}All test functions contain assertions${NC}"
else
    WARNINGS=$((WARNINGS + EMPTY_TESTS))
fi

# ---------------------------------------------------------------------------
# Check 5: Tests that compare values to themselves
# ---------------------------------------------------------------------------
echo -e "${BLUE}[5/5] Checking for self-comparisons (assert_eq!(x, x))...${NC}"
SELF_CMP=$(grep -rEn 'assert_eq!\s*\(\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*,\s*\1\s*\)' src/ tests/ --include="*.rs" 2>/dev/null || true)
if [ -n "$SELF_CMP" ]; then
    SELF_COUNT=$(echo "$SELF_CMP" | wc -l | tr -d ' ')
    NEW_ISSUES=$((NEW_ISSUES + SELF_COUNT))
    echo -e "  ${RED}Found ${SELF_COUNT} self-comparison assertions:${NC}"
    echo "$SELF_CMP" | head -5
else
    echo -e "  ${GREEN}No self-comparison assertions found${NC}"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo -e "${BLUE}=== Test Quality Summary ===${NC}"
echo -e "  Known issues (baselined): ${KNOWN_ISSUES}"
echo -e "  Warnings:                 ${WARNINGS}"
echo -e "  New issues:               ${NEW_ISSUES}"

if [ "$NEW_ISSUES" -gt 0 ]; then
    echo ""
    echo -e "${RED}FAILED: ${NEW_ISSUES} new test quality issue(s) found.${NC}"
    echo -e "${RED}Fix the issues above or add justification to ${BASELINE_FILE}${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}PASSED: No new test quality issues.${NC}"
exit 0
