#!/usr/bin/env bash
# =============================================================================
# Agent Work Verification Pipeline
# This script MUST pass before any commit of agent-generated code.
# Runs: format → compile → lint → workspace tests → mock-feature tests →
#        docs → architecture → test quality → production safety →
#        frontend → backlog check → sqlx cache → cargo deny
# CI (.github/workflows/ci.yml) runs the same commands (B-089): when a
# command changes here, change it there in the same PR.
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

echo -e "${BOLD}${BLUE}============================================${NC}"
echo -e "${BOLD}${BLUE}  Agent Work Verification Pipeline${NC}"
echo -e "${BOLD}${BLUE}============================================${NC}"
echo ""

# ===== Step 1: Format Check =====
echo -e "${BLUE}[1/${TOTAL_STEPS}] Checking code formatting (cargo fmt --all)...${NC}"
if cargo fmt --all -- --check 2>&1; then
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
echo -e "${BLUE}[3/${TOTAL_STEPS}] Running linter (cargo clippy --workspace --all-targets)...${NC}"
# Warn-only until B-090..B-094 land; B-094 flips this and CI to -D warnings
# together. Hard clippy errors still fail.
CLIPPY_OUTPUT=$(cargo clippy --workspace --all-targets 2>&1)
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

# ===== Step 4: Workspace Tests =====
echo -e "${BLUE}[4/${TOTAL_STEPS}] Running workspace tests (cargo test --workspace --all-targets)...${NC}"
# The exit code is the source of truth. --all-targets covers lib, bins and
# every integration test binary; it deliberately excludes doctests, which are
# tracked by B-081(d)/B-068 and not yet green.
# The known-failures escape hatch (.governance/known_test_issues.txt) is gone
# (B-089): every listed test passed on main at e2a042d, so any failure here
# is a real failure.
# Compile test binaries untimed first: on a cold build cache (fresh worktree)
# compilation alone exceeds the timeout, failing the run with exit 124 even
# though every test passes. The timeout below then covers only test execution.
cargo test --workspace --all-targets --no-run 2>&1 | tail -3
TEST_OUTPUT=$(timeout 300 cargo test --workspace --all-targets 2>&1)
TEST_STATUS=$?
TEST_PASSED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s+0}')
TEST_FAILED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END {print s+0}')
if [ "$TEST_STATUS" -eq 124 ]; then
    echo -e "  ${RED}Timed out after 300s of test execution (build was already complete)${NC}"
    step_fail "Workspace tests (timed out)"
elif [ "$TEST_STATUS" -eq 0 ]; then
    step_pass "Workspace tests (${TEST_PASSED} passed)"
else
    echo "$TEST_OUTPUT" | grep -E "FAILED|error" | head -20
    step_fail "Workspace tests (exit ${TEST_STATUS}, ${TEST_FAILED} failed)"
fi

# ===== Step 5: Mock-Feature Tests =====
echo -e "${BLUE}[5/${TOTAL_STEPS}] Running workspace tests with mock feature (cargo test --workspace --all-targets --features mock)...${NC}"
# The mock feature (src/db/mock.rs) is compiled by nothing else in this
# pipeline, so it rotted silently for months with compile errors. The feature
# is a superset of the default `database` feature, so this also re-runs the
# step-4 tests; the extra build is the price of keeping the feature alive.
# Same cold-cache handling as step 4.
cargo test --workspace --all-targets --features mock --no-run 2>&1 | tail -3
MOCK_TEST_OUTPUT=$(timeout 300 cargo test --workspace --all-targets --features mock 2>&1)
MOCK_TEST_STATUS=$?
MOCK_TEST_PASSED=$(echo "$MOCK_TEST_OUTPUT" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s+0}')
MOCK_TEST_FAILED=$(echo "$MOCK_TEST_OUTPUT" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END {print s+0}')
if [ "$MOCK_TEST_STATUS" -eq 124 ]; then
    echo -e "  ${RED}Timed out after 300s of test execution (build was already complete)${NC}"
    step_fail "Mock-feature tests (timed out)"
elif [ "$MOCK_TEST_STATUS" -eq 0 ]; then
    step_pass "Mock-feature tests (${MOCK_TEST_PASSED} passed)"
else
    echo "$MOCK_TEST_OUTPUT" | grep -E "FAILED|error" | head -20
    step_fail "Mock-feature tests (exit ${MOCK_TEST_STATUS}, ${MOCK_TEST_FAILED} failed)"
fi

# ===== Step 6: Documentation =====
# Warn-only until B-090..B-094 land: the crate's #![warn(missing_docs)] emits
# 681 warnings today, so RUSTDOCFLAGS=-Dwarnings cannot pass before that debt
# is paid. B-094 flips this and CI to -Dwarnings together with clippy.
echo -e "${BLUE}[6/${TOTAL_STEPS}] Checking documentation compiles (cargo doc --workspace --no-deps)...${NC}"
if cargo doc --workspace --no-deps 2>&1; then
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

# ===== Step 12: sqlx offline cache is current =====
# A query changed without re-running `cargo sqlx prepare` would build from a
# stale cache. --check regenerates into a temp dir and compares.
echo -e "${BLUE}[12/${TOTAL_STEPS}] Checking sqlx offline cache is current (cargo sqlx prepare --check)...${NC}"
if ! cargo sqlx --version >/dev/null 2>&1; then
    step_skip "sqlx offline cache" "sqlx-cli not installed: cargo install sqlx-cli --no-default-features --features sqlite"
elif SQLX_OFFLINE=false cargo sqlx prepare --check -- --lib --tests 2>&1; then
    step_pass "sqlx offline cache"
else
    echo -e "  ${YELLOW}Run: cargo sqlx prepare -- --lib --tests, then commit .sqlx/${NC}"
    step_fail "sqlx offline cache"
fi

# ===== Step 13: Dependency advisories and licences =====
# Configured by deny.toml; every ignored advisory there names the backlog
# task whose landing removes it.
echo -e "${BLUE}[13/${TOTAL_STEPS}] Checking dependency advisories and licences (cargo deny check)...${NC}"
if ! command -v cargo-deny >/dev/null 2>&1; then
    step_skip "cargo deny" "cargo-deny not installed: cargo install cargo-deny --locked"
elif cargo deny check 2>&1; then
    step_pass "cargo deny"
else
    step_fail "cargo deny"
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
