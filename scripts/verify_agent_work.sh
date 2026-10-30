#!/usr/bin/env bash
# =============================================================================
# Agent Work Verification Pipeline
# This script MUST pass before any commit of agent-generated code.
# Runs: format → compile → lint → unit tests → mock-feature tests →
#        integration tests → docs → architecture → test quality → backlog check
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

TOTAL_STEPS=13
PASSED=0
FAILED=0
SKIPPED=0
START_TIME=$(date +%s)

# sqlx compile-time macros build from the committed offline cache in .sqlx/,
# so no database is needed to compile. A dev database is still provisioned so
# the last step can check that the cache matches the current queries.
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"
if [ -z "${DATABASE_URL:-}" ]; then
    if ./scripts/setup_db.sh --if-missing >/dev/null; then
        export DATABASE_URL="sqlite://$PWD/data/db/daytrader.db"
    else
        echo -e "${RED}ERROR: scripts/setup_db.sh failed; cannot check the sqlx offline cache${NC}"
        exit 1
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

# Prints the names of failing tests that are NOT listed in the known-issues
# baseline, one per line. Empty output means every failure is a known one.
#
# Previously the gate only compared the NUMBER of failures against the NUMBER
# of lines in the baseline, so any N new failures passed silently as long as N
# stayed under the ceiling. That is how a real regression in data::ingestion
# reached a green run on 2026-07-28. Match by test name instead.
unlisted_test_failures() {
    local test_output="$1"
    local known_file="$2"
    local expected_failures="${3:-0}"
    local failure_line='^test [^ ]+( - should panic)? \.\.\. FAILED'
    local parsed name
    while IFS= read -r name; do
        [ -z "$name" ] && continue
        if [ ! -f "$known_file" ] || ! grep -q "^${name}:" "$known_file" 2>/dev/null; then
            echo "$name"
        fi
    done < <(echo "$test_output" | grep -oE "$failure_line" | awk '{print $2}' | sort -u)

    # Fail closed: a failure whose name could not be parsed must never pass as
    # a known one (libtest prints `test NAME - should panic ... FAILED`).
    parsed=$(echo "$test_output" | grep -cE "$failure_line" || true)
    if [ "$parsed" -lt "$expected_failures" ]; then
        echo "<$((expected_failures - parsed)) failing test(s) whose names could not be parsed>"
    fi
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
# --workspace is required: the root is itself a package, so a bare
# `cargo check` skips daytrader/src-tauri (the app binary sat broken for
# months without the pipeline noticing).
echo -e "${BLUE}[2/${TOTAL_STEPS}] Checking compilation (cargo check --workspace)...${NC}"
if cargo check --workspace 2>&1; then
    step_pass "Compilation"
else
    step_fail "Compilation"
    echo -e "${RED}Compilation failed. Remaining steps may not be meaningful.${NC}"
fi

# ===== Step 3: Linting =====
echo -e "${BLUE}[3/${TOTAL_STEPS}] Running linter (cargo clippy)...${NC}"
# Use -W warnings (warn, don't deny) to avoid blocking on pre-existing warnings.
# TODO: Tighten to -D warnings once codebase warnings are cleaned up.
CLIPPY_OUTPUT=$(cargo clippy --workspace 2>&1)
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
# Compile test binaries untimed first: on a cold build cache (fresh worktree)
# compilation alone exceeds the timeout, failing the run with exit 124 even
# though every test passes. The timeout below then covers only test execution.
cargo test --lib --no-run 2>&1 | tail -3
TEST_OUTPUT=$(timeout 120 cargo test --lib 2>&1)
TEST_STATUS=$?
TEST_PASSED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s+0}')
TEST_FAILED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END {print s+0}')
if [ "$TEST_STATUS" -eq 124 ]; then
    echo -e "  ${RED}Timed out after 120s of test execution (build was already complete)${NC}"
    step_fail "Unit tests (timed out)"
elif [ "$TEST_STATUS" -eq 0 ]; then
    step_pass "Unit tests (${TEST_PASSED} passed)"
elif [ "$TEST_FAILED" -gt 0 ] && [ -f "$KNOWN_ISSUES_FILE" ]; then
    UNLISTED=$(unlisted_test_failures "$TEST_OUTPUT" "$KNOWN_ISSUES_FILE" "$TEST_FAILED")
    if [ -z "$UNLISTED" ]; then
        echo -e "  ${YELLOW}${TEST_FAILED} known failures (see ${KNOWN_ISSUES_FILE})${NC}"
        echo -e "  ${YELLOW}Known failures are a ceiling, not a licence — shrink the file when you touch these areas.${NC}"
        step_pass "Unit tests (${TEST_PASSED} passed, ${TEST_FAILED} known failures)"
    else
        echo -e "  ${RED}Failing tests not listed in ${KNOWN_ISSUES_FILE}:${NC}"
        echo "$UNLISTED" | sed 's/^/    /'
        step_fail "Unit tests ($(echo "$UNLISTED" | wc -l | tr -d ' ') unlisted failure(s))"
    fi
else
    echo "$TEST_OUTPUT" | grep -E "FAILED|error" | head -10
    step_fail "Unit tests (exit ${TEST_STATUS}, ${TEST_FAILED} failed)"
fi

# ===== Step 5: Mock-Feature Unit Tests =====
echo -e "${BLUE}[5/${TOTAL_STEPS}] Running unit tests with mock feature (cargo test --lib --features mock)...${NC}"
# The mock feature (src/db/mock.rs) is compiled by nothing else in this
# pipeline, so it rotted silently for months with compile errors. The feature
# is a superset of the default `database` feature, so this also re-runs the
# step-4 tests; the extra lib build is the price of keeping the feature alive.
# Same cold-cache handling as step 4: the mock feature is a distinct cfg set,
# so compile untimed first and let the timeout cover only test execution.
cargo test --lib --features mock --no-run 2>&1 | tail -3
MOCK_TEST_OUTPUT=$(timeout 120 cargo test --lib --features mock 2>&1)
MOCK_TEST_STATUS=$?
MOCK_TEST_PASSED=$(echo "$MOCK_TEST_OUTPUT" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s+0}')
MOCK_TEST_FAILED=$(echo "$MOCK_TEST_OUTPUT" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END {print s+0}')
if [ "$MOCK_TEST_STATUS" -eq 124 ]; then
    echo -e "  ${RED}Timed out after 120s of test execution (build was already complete)${NC}"
    step_fail "Mock-feature unit tests (timed out)"
elif [ "$MOCK_TEST_STATUS" -eq 0 ]; then
    step_pass "Mock-feature unit tests (${MOCK_TEST_PASSED} passed)"
elif [ "$MOCK_TEST_FAILED" -gt 0 ] && [ -f "$KNOWN_ISSUES_FILE" ]; then
    MOCK_UNLISTED=$(unlisted_test_failures "$MOCK_TEST_OUTPUT" "$KNOWN_ISSUES_FILE" "$MOCK_TEST_FAILED")
    if [ -z "$MOCK_UNLISTED" ]; then
        echo -e "  ${YELLOW}${MOCK_TEST_FAILED} known failures (see ${KNOWN_ISSUES_FILE})${NC}"
        step_pass "Mock-feature unit tests (${MOCK_TEST_PASSED} passed, ${MOCK_TEST_FAILED} known failures)"
    else
        echo -e "  ${RED}Failing tests not listed in ${KNOWN_ISSUES_FILE}:${NC}"
        echo "$MOCK_UNLISTED" | sed 's/^/    /'
        step_fail "Mock-feature unit tests ($(echo "$MOCK_UNLISTED" | wc -l | tr -d ' ') unlisted failure(s))"
    fi
else
    echo "$MOCK_TEST_OUTPUT" | grep -E "FAILED|error" | head -10
    step_fail "Mock-feature unit tests (exit ${MOCK_TEST_STATUS}, ${MOCK_TEST_FAILED} failed)"
fi

# ===== Step 6: Integration Tests =====
echo -e "${BLUE}[6/${TOTAL_STEPS}] Running integration tests...${NC}"
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

# ===== Step 7: Documentation =====
echo -e "${BLUE}[7/${TOTAL_STEPS}] Checking documentation compiles (cargo doc)...${NC}"
if cargo doc --no-deps 2>&1; then
    step_pass "Documentation"
else
    step_fail "Documentation"
fi

# ===== Step 8: Architecture Boundaries =====
echo -e "${BLUE}[8/${TOTAL_STEPS}] Verifying architecture boundaries...${NC}"
if [ -x "./scripts/verify_architecture.sh" ]; then
    if ./scripts/verify_architecture.sh 2>&1; then
        step_pass "Architecture boundaries"
    else
        step_fail "Architecture boundaries"
    fi
else
    step_skip "Architecture boundaries" "verify_architecture.sh not found or not executable"
fi

# ===== Step 9: Test Quality =====
echo -e "${BLUE}[9/${TOTAL_STEPS}] Verifying test quality...${NC}"
if [ -x "./scripts/verify_test_quality.sh" ]; then
    if ./scripts/verify_test_quality.sh 2>&1; then
        step_pass "Test quality"
    else
        step_fail "Test quality"
    fi
else
    step_skip "Test quality" "verify_test_quality.sh not found or not executable"
fi

# ===== Step 10: Production Safety =====
echo -e "${BLUE}[10/${TOTAL_STEPS}] Verifying production safety (unwrap/std::sync)...${NC}"
if [ -x "./scripts/verify_production_safety.sh" ]; then
    if ./scripts/verify_production_safety.sh 2>&1; then
        step_pass "Production safety"
    else
        step_fail "Production safety"
    fi
else
    step_skip "Production safety" "verify_production_safety.sh not found or not executable"
fi

# ===== Step 11: Frontend Build =====
echo -e "${BLUE}[11/${TOTAL_STEPS}] Checking frontend build (tsc + vite)...${NC}"
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

# ===== Step 12: Backlog Updated =====
echo -e "${BLUE}[12/${TOTAL_STEPS}] Checking BACKLOG.md was updated...${NC}"
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

# ===== Step 13: sqlx offline cache is current =====
# A query changed without re-running `cargo sqlx prepare` would build from a
# stale cache. --check regenerates into a temp dir and compares.
echo -e "${BLUE}[13/${TOTAL_STEPS}] Checking sqlx offline cache is current (cargo sqlx prepare --check)...${NC}"
if ! cargo sqlx --version >/dev/null 2>&1; then
    step_skip "sqlx offline cache" "sqlx-cli not installed: cargo install sqlx-cli --no-default-features --features sqlite"
elif SQLX_OFFLINE=false cargo sqlx prepare --check -- --lib --tests 2>&1; then
    step_pass "sqlx offline cache"
else
    echo -e "  ${YELLOW}Run: cargo sqlx prepare -- --lib --tests, then commit .sqlx/${NC}"
    step_fail "sqlx offline cache"
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
