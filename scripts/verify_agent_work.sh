#!/usr/bin/env bash
# =============================================================================
# Agent Work Verification Pipeline
# This script MUST pass before any commit of agent-generated code.
# Runs: format → compile → lint → unit tests → integration tests →
#        docs → architecture → test quality → backlog check
# =============================================================================
set -uo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Ensure we're at the project root
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}ERROR: Must be run from project root (where Cargo.toml is)${NC}"
    exit 1
fi

TOTAL_STEPS=9
PASSED=0
FAILED=0
SKIPPED=0
START_TIME=$(date +%s)

# Track results for summary
declare -a RESULTS

step_pass() {
    PASSED=$((PASSED + 1))
    RESULTS+=("${GREEN}PASS${NC} $1")
    echo -e "  ${GREEN}✓ PASSED${NC}"
    echo ""
}

step_fail() {
    FAILED=$((FAILED + 1))
    RESULTS+=("${RED}FAIL${NC} $1")
    echo -e "  ${RED}✗ FAILED${NC}"
    echo ""
}

step_skip() {
    SKIPPED=$((SKIPPED + 1))
    RESULTS+=("${YELLOW}SKIP${NC} $1")
    echo -e "  ${YELLOW}⊘ SKIPPED${NC} ($2)"
    echo ""
}

echo -e "${BOLD}${BLUE}============================================${NC}"
echo -e "${BOLD}${BLUE}  Agent Work Verification Pipeline${NC}"
echo -e "${BOLD}${BLUE}============================================${NC}"
echo ""

# ===== Step 1: Format Check =====
echo -e "${BLUE}[1/${TOTAL_STEPS}] Checking code formatting (cargo fmt)...${NC}"
if cargo fmt -- --check 2>&1; then
    step_pass "Code formatting"
else
    step_fail "Code formatting"
fi

# ===== Step 2: Compilation =====
echo -e "${BLUE}[2/${TOTAL_STEPS}] Checking compilation (cargo check)...${NC}"
if cargo check 2>&1; then
    step_pass "Compilation"
else
    step_fail "Compilation"
    echo -e "${RED}Compilation failed. Remaining steps may not be meaningful.${NC}"
fi

# ===== Step 3: Linting =====
echo -e "${BLUE}[3/${TOTAL_STEPS}] Running linter (cargo clippy -D warnings)...${NC}"
if cargo clippy -- -D warnings 2>&1; then
    step_pass "Linting"
else
    step_fail "Linting (warnings treated as errors)"
fi

# ===== Step 4: Unit Tests =====
echo -e "${BLUE}[4/${TOTAL_STEPS}] Running unit tests (cargo test --lib)...${NC}"
if cargo test --lib 2>&1; then
    step_pass "Unit tests"
else
    step_fail "Unit tests"
fi

# ===== Step 5: Integration Tests =====
echo -e "${BLUE}[5/${TOTAL_STEPS}] Running integration tests...${NC}"
# Check if source files were changed (optimization: skip if only docs/plans changed)
CHANGED_SRC=$(git diff --cached --name-only 2>/dev/null | grep -E '\.(rs|toml)$' || true)
if [ -n "$CHANGED_SRC" ]; then
    if cargo test --test '*' 2>&1; then
        step_pass "Integration tests"
    else
        step_fail "Integration tests"
    fi
else
    step_skip "Integration tests" "no .rs/.toml files in staged changes"
fi

# ===== Step 6: Documentation =====
echo -e "${BLUE}[6/${TOTAL_STEPS}] Checking documentation compiles (cargo doc)...${NC}"
if cargo doc --no-deps 2>&1; then
    step_pass "Documentation"
else
    step_fail "Documentation"
fi

# ===== Step 7: Architecture Boundaries =====
echo -e "${BLUE}[7/${TOTAL_STEPS}] Verifying architecture boundaries...${NC}"
if [ -x "./scripts/verify_architecture.sh" ]; then
    if ./scripts/verify_architecture.sh 2>&1; then
        step_pass "Architecture boundaries"
    else
        step_fail "Architecture boundaries"
    fi
else
    step_skip "Architecture boundaries" "verify_architecture.sh not found or not executable"
fi

# ===== Step 8: Test Quality =====
echo -e "${BLUE}[8/${TOTAL_STEPS}] Verifying test quality...${NC}"
if [ -x "./scripts/verify_test_quality.sh" ]; then
    if ./scripts/verify_test_quality.sh 2>&1; then
        step_pass "Test quality"
    else
        step_fail "Test quality"
    fi
else
    step_skip "Test quality" "verify_test_quality.sh not found or not executable"
fi

# ===== Step 9: Backlog Updated =====
echo -e "${BLUE}[9/${TOTAL_STEPS}] Checking BACKLOG.md was updated...${NC}"
BACKLOG_CHANGED=$(git diff --cached --name-only 2>/dev/null | grep 'BACKLOG.md' || true)
SRC_CHANGED=$(git diff --cached --name-only 2>/dev/null | grep -E '^src/' || true)
if [ -n "$BACKLOG_CHANGED" ]; then
    step_pass "Backlog updated"
elif [ -z "$SRC_CHANGED" ]; then
    step_skip "Backlog check" "no source file changes staged"
else
    echo -e "  ${YELLOW}WARNING: Source files changed but BACKLOG.md was not updated${NC}"
    echo -e "  ${YELLOW}Agent-generated code MUST update the backlog.${NC}"
    RESULTS+=("${YELLOW}WARN${NC} Backlog not updated (source files changed)")
    SKIPPED=$((SKIPPED + 1))
fi

# ===== Summary =====
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo ""
echo -e "${BOLD}${BLUE}============================================${NC}"
echo -e "${BOLD}${BLUE}  Verification Summary${NC}"
echo -e "${BOLD}${BLUE}============================================${NC}"
echo ""
for result in "${RESULTS[@]}"; do
    echo -e "  ${result}"
done
echo ""
echo -e "  Passed:  ${GREEN}${PASSED}${NC}"
echo -e "  Failed:  ${RED}${FAILED}${NC}"
echo -e "  Skipped: ${YELLOW}${SKIPPED}${NC}"
echo -e "  Time:    ${ELAPSED}s"
echo ""

# Save verification report
REPORT_DIR="reports/verification"
mkdir -p "$REPORT_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="${REPORT_DIR}/verification_${TIMESTAMP}.txt"
{
    echo "Agent Work Verification Report"
    echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Branch: $(git branch --show-current 2>/dev/null || echo 'unknown')"
    echo "Passed: ${PASSED}"
    echo "Failed: ${FAILED}"
    echo "Skipped: ${SKIPPED}"
    echo "Duration: ${ELAPSED}s"
    echo ""
    echo "Staged files:"
    git diff --cached --name-only 2>/dev/null || echo "  (no staged files)"
} > "$REPORT_FILE"
echo -e "  Report: ${REPORT_FILE}"

if [ "$FAILED" -gt 0 ]; then
    echo ""
    echo -e "${RED}${BOLD}VERIFICATION FAILED: ${FAILED} check(s) did not pass.${NC}"
    echo -e "${RED}Fix all issues before committing.${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}${BOLD}VERIFICATION PASSED: All checks succeeded.${NC}"
exit 0
