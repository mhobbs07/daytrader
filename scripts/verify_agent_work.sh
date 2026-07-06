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

TOTAL_STEPS=11
PASSED=0
FAILED=0
SKIPPED=0
START_TIME=$(date +%s)

# sqlx compile-time macros validate queries against DATABASE_URL. Fresh
# worktrees have neither the env var nor the database — provision both so the
# pipeline works anywhere without manual setup (see scripts/setup_db.sh).
if [ -z "${DATABASE_URL:-}" ]; then
    if [ ! -f "data/db/daytrader.db" ] && command -v sqlite3 >/dev/null 2>&1; then
        mkdir -p data/db
        sqlite3 data/db/daytrader.db < src/db/schema.sql 2>/dev/null || true
        sqlite3 data/db/daytrader.db "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, description TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT (datetime('now')));" 2>/dev/null || true
    fi
    if [ -f "data/db/daytrader.db" ]; then
        export DATABASE_URL="sqlite://$PWD/data/db/daytrader.db"
        echo -e "${YELLOW}DATABASE_URL was unset — using ${DATABASE_URL}${NC}"
    fi
fi

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
echo -e "${BLUE}[3/${TOTAL_STEPS}] Running linter (cargo clippy)...${NC}"
# Use -W warnings (warn, don't deny) to avoid blocking on pre-existing warnings.
# TODO: Tighten to -D warnings once codebase warnings are cleaned up.
CLIPPY_OUTPUT=$(cargo clippy 2>&1)
CLIPPY_ERRORS=$(echo "$CLIPPY_OUTPUT" | grep -c "^error\[" || true)
if [ "$CLIPPY_ERRORS" -eq 0 ]; then
    CLIPPY_WARNINGS=$(echo "$CLIPPY_OUTPUT" | grep -c "^warning\[" || true)
    if [ "$CLIPPY_WARNINGS" -gt 0 ]; then
        echo -e "  ${YELLOW}${CLIPPY_WARNINGS} clippy warnings (non-blocking)${NC}"
    fi
    step_pass "Linting (no errors)"
else
    echo "$CLIPPY_OUTPUT"
    step_fail "Linting (clippy errors found)"
fi

# ===== Step 4: Unit Tests =====
echo -e "${BLUE}[4/${TOTAL_STEPS}] Running unit tests (cargo test --lib)...${NC}"
# Use timeout to prevent hanging tests from blocking the pipeline.
# The exit code is the source of truth: grepping for "test result: ok" passed
# when ANY test binary was ok even if another failed, and grep -oP is GNU-only
# (always reported "0 passed" on macOS).
KNOWN_ISSUES_FILE=".governance/known_test_issues.txt"
TEST_OUTPUT=$(timeout 120 cargo test --lib 2>&1)
TEST_STATUS=$?
TEST_PASSED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s+0}')
TEST_FAILED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END {print s+0}')
if [ "$TEST_STATUS" -eq 0 ]; then
    step_pass "Unit tests (${TEST_PASSED} passed)"
elif [ "$TEST_FAILED" -gt 0 ] && [ -f "$KNOWN_ISSUES_FILE" ]; then
    KNOWN_COUNT=$(grep -c "^[^#]" "$KNOWN_ISSUES_FILE" 2>/dev/null || echo "0")
    if [ "$TEST_FAILED" -le "$KNOWN_COUNT" ]; then
        echo -e "  ${YELLOW}${TEST_FAILED} known failures (see ${KNOWN_ISSUES_FILE})${NC}"
        echo -e "  ${YELLOW}Known failures are a ceiling, not a licence — shrink the file when you touch these areas.${NC}"
        step_pass "Unit tests (${TEST_PASSED} passed, ${TEST_FAILED} known failures)"
    else
        echo "$TEST_OUTPUT" | grep "FAILED"
        step_fail "Unit tests (${TEST_FAILED} failures exceed ${KNOWN_COUNT} known issues)"
    fi
else
    echo "$TEST_OUTPUT" | grep -E "FAILED|error" | head -10
    step_fail "Unit tests (exit ${TEST_STATUS}, ${TEST_FAILED} failed)"
fi

# ===== Step 5: Integration Tests =====
echo -e "${BLUE}[5/${TOTAL_STEPS}] Running integration tests...${NC}"
# Skip only when neither the staged set NOR the working tree touches .rs/.toml
# (checking staged-only silently skipped integration tests for unstaged work).
CHANGED_SRC=$( { git diff --cached --name-only; git diff HEAD --name-only; } 2>/dev/null | sort -u | grep -E '\.(rs|toml)$' || true)
if [ -n "$CHANGED_SRC" ]; then
    if cargo test --test '*' 2>&1; then
        step_pass "Integration tests"
    else
        step_fail "Integration tests"
    fi
else
    step_skip "Integration tests" "no .rs/.toml changes staged or in working tree"
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

# ===== Step 9: Production Safety =====
echo -e "${BLUE}[9/${TOTAL_STEPS}] Verifying production safety (unwrap/std::sync)...${NC}"
if [ -x "./scripts/verify_production_safety.sh" ]; then
    if ./scripts/verify_production_safety.sh 2>&1; then
        step_pass "Production safety"
    else
        step_fail "Production safety"
    fi
else
    step_skip "Production safety" "verify_production_safety.sh not found or not executable"
fi

# ===== Step 10: Frontend Build =====
echo -e "${BLUE}[10/${TOTAL_STEPS}] Checking frontend build (tsc + vite)...${NC}"
CHANGED_FRONTEND=$( { git diff --cached --name-only; git diff HEAD --name-only; } 2>/dev/null | sort -u | grep -E '^daytrader/(src/|package\.json|tsconfig|vite)' || true)
if [ -z "$CHANGED_FRONTEND" ]; then
    step_skip "Frontend build" "no daytrader/ frontend changes staged or in working tree"
elif ! command -v npm >/dev/null 2>&1; then
    step_skip "Frontend build" "npm not available"
else
    if [ ! -d "daytrader/node_modules" ]; then
        echo -e "  ${YELLOW}node_modules missing — running npm install...${NC}"
        (cd daytrader && npm install --no-audit --no-fund >/dev/null 2>&1)
    fi
    if (cd daytrader && npm run build 2>&1); then
        step_pass "Frontend build"
    else
        step_fail "Frontend build"
    fi
fi

# ===== Step 11: Backlog Updated =====
echo -e "${BLUE}[11/${TOTAL_STEPS}] Checking BACKLOG.md was updated...${NC}"
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
