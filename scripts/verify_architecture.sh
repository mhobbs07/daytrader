#!/usr/bin/env bash
# =============================================================================
# Architecture Boundary Verification
# Validates that no module violates its declared dependency boundaries.
# Reads exceptions from .governance/architecture_exceptions.txt
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

EXCEPTIONS_FILE=".governance/architecture_exceptions.txt"
# Known pre-existing violations (discovered 2026-07-06 when detection was
# tightened to catch inline qualified paths). Format: "src -> target | count".
# This is a ratchet: counts may shrink, never grow. Tracked by B-070.
BASELINE_FILE=".governance/architecture_violations_baseline.txt"
VIOLATIONS=0
EXCEPTIONS_USED=0
BASELINED=0

echo -e "${BLUE}=== Architecture Boundary Verification ===${NC}"
echo ""

# Helper: check if an exception exists for source -> target
is_exception() {
    local src_module="$1"
    local target="$2"
    if [ ! -f "$EXCEPTIONS_FILE" ]; then
        return 1
    fi
    # Match "source -> target" in exceptions file (ignoring comments)
    grep -v '^#' "$EXCEPTIONS_FILE" 2>/dev/null | grep -q "^${src_module} -> ${target}" 2>/dev/null
}

# Helper: baselined violation ceiling for source -> target (prints count, or 0)
baseline_count() {
    local src_module="$1"
    local target="$2"
    if [ ! -f "$BASELINE_FILE" ]; then
        echo 0
        return
    fi
    grep -v '^#' "$BASELINE_FILE" 2>/dev/null \
        | grep "^${src_module} -> ${target} |" \
        | awk -F'|' '{gsub(/ /,"",$2); print $2}' \
        | head -1 | grep -E '^[0-9]+$' || echo 0
}

# Helper: check forbidden dependency
# Args: source_dir, forbidden_module, description
check_forbidden_dep() {
    local source_dir="$1"
    local forbidden="$2"
    local desc="$3"

    [ ! -d "$source_dir" ] && return

    # Extract source module name from path
    local src_module
    src_module=$(echo "$source_dir" | sed 's|src/||' | cut -d'/' -f1)

    # Scan for forbidden imports in non-test code
    # We create a temp file with test-excluded content for each source file
    local found_violations=""
    while IFS= read -r rs_file; do
        [ -z "$rs_file" ] && continue

        # Extract non-test code: remove everything after #[cfg(test)] mod block
        # This is a heuristic - we look for the #[cfg(test)] line and skip from there
        local test_start
        test_start=$(grep -n '#\[cfg(test)\]' "$rs_file" 2>/dev/null | head -1 | cut -d':' -f1 || true)

        local search_content
        if [ -n "$test_start" ]; then
            search_content=$(head -n "$((test_start - 1))" "$rs_file" 2>/dev/null)
        else
            search_content=$(cat "$rs_file" 2>/dev/null)
        fi

        # Search for the forbidden dependency. Match any qualified path, not
        # just `use` statements — inline `crate::<module>::Type::new(...)`
        # calls are imports too (a risk->patterns violation hid this way for
        # months). Comment lines are excluded so docs can mention modules.
        local matches
        matches=$(echo "$search_content" | grep -n "crate::${forbidden}::" 2>/dev/null | grep -vE '^[0-9]+:[[:space:]]*//' || true)
        if [ -n "$matches" ]; then
            while IFS= read -r match; do
                [ -z "$match" ] && continue
                found_violations="${found_violations}${rs_file}:${match}\n"
            done <<< "$matches"
        fi
    done < <(find "$source_dir" -name "*.rs" -type f 2>/dev/null)

    if [ -n "$found_violations" ]; then
        # Check if this is an allowed exception
        if is_exception "$src_module" "$forbidden"; then
            EXCEPTIONS_USED=$((EXCEPTIONS_USED + 1))
            return
        fi

        local count
        count=$(echo -e "$found_violations" | grep -c . 2>/dev/null || echo 0)

        # Known pre-existing debt: pass at or below the baselined ceiling.
        local ceiling
        ceiling=$(baseline_count "$src_module" "$forbidden")
        if [ "$count" -le "$ceiling" ]; then
            BASELINED=$((BASELINED + 1))
            if [ "$count" -lt "$ceiling" ]; then
                echo -e "${YELLOW}KNOWN DEBT: ${desc} improved to ${count} (baseline ${ceiling}) — ratchet the baseline down${NC}"
            else
                echo -e "${YELLOW}KNOWN DEBT: ${desc} (${count}, baselined — see B-070)${NC}"
            fi
            return
        fi

        VIOLATIONS=$((VIOLATIONS + count))
        echo -e "${RED}VIOLATION: ${desc} (${count} occurrence(s), baseline ${ceiling})${NC}"
        # `|| true`: the loop's last [ -n ] test fails on the trailing blank
        # line, and under set -e that silently killed the script mid-scan the
        # first time a violation was ever printed.
        echo -e "$found_violations" | while IFS= read -r line; do
            [ -n "$line" ] && echo -e "  ${RED}  ${line}${NC}"
        done || true
    fi
}

# ===== Foundation Layer: utils must not import from upper layers =====
echo -e "${BLUE}[1/5] Checking Foundation Layer (utils, brokers)...${NC}"
check_forbidden_dep "src/utils" "risk" "utils -> risk"
check_forbidden_dep "src/utils" "orders" "utils -> orders"
check_forbidden_dep "src/utils" "positions" "utils -> positions"
check_forbidden_dep "src/utils" "execution" "utils -> execution"
check_forbidden_dep "src/utils" "patterns" "utils -> patterns"
check_forbidden_dep "src/utils" "events" "utils -> events"
check_forbidden_dep "src/utils" "config" "utils -> config"
check_forbidden_dep "src/utils" "db" "utils -> db"

# ===== Infrastructure Layer =====
echo -e "${BLUE}[2/5] Checking Infrastructure Layer (events, config, db)...${NC}"
check_forbidden_dep "src/events" "risk" "events -> risk"
check_forbidden_dep "src/events" "orders" "events -> orders"
check_forbidden_dep "src/events" "positions" "events -> positions"
check_forbidden_dep "src/events" "execution" "events -> execution"
check_forbidden_dep "src/events" "patterns" "events -> patterns"
check_forbidden_dep "src/events" "config" "events -> config"
check_forbidden_dep "src/events" "db" "events -> db"

check_forbidden_dep "src/config" "orders" "config -> orders"
check_forbidden_dep "src/config" "positions" "config -> positions"
check_forbidden_dep "src/config" "execution" "config -> execution"
check_forbidden_dep "src/config" "patterns" "config -> patterns"
check_forbidden_dep "src/config" "events" "config -> events"
check_forbidden_dep "src/config" "db" "config -> db"

check_forbidden_dep "src/db" "risk" "db -> risk"
check_forbidden_dep "src/db" "orders" "db -> orders"
check_forbidden_dep "src/db" "positions" "db -> positions"
check_forbidden_dep "src/db" "execution" "db -> execution"

# ===== Business Logic Layer =====
echo -e "${BLUE}[3/5] Checking Business Logic Layer (risk, patterns, positions)...${NC}"
check_forbidden_dep "src/risk" "orders" "risk -> orders"
check_forbidden_dep "src/risk" "positions" "risk -> positions"
check_forbidden_dep "src/risk" "execution" "risk -> execution"
check_forbidden_dep "src/risk" "patterns" "risk -> patterns"

check_forbidden_dep "src/patterns" "orders" "patterns -> orders"
check_forbidden_dep "src/patterns" "positions" "patterns -> positions"
check_forbidden_dep "src/patterns" "execution" "patterns -> execution"
check_forbidden_dep "src/patterns" "risk" "patterns -> risk"

check_forbidden_dep "src/positions" "execution" "positions -> execution"
check_forbidden_dep "src/positions" "risk" "positions -> risk"
check_forbidden_dep "src/positions" "patterns" "positions -> patterns"

# ===== Orchestration Layer =====
echo -e "${BLUE}[4/5] Checking Orchestration Layer (orders, execution)...${NC}"
check_forbidden_dep "src/orders" "execution" "orders -> execution"
check_forbidden_dep "src/orders" "patterns" "orders -> patterns"

# ===== Top of tree: nothing imports execution =====
echo -e "${BLUE}[5/5] Checking that nothing depends on execution (top of tree)...${NC}"
for mod in brokers config db events risk patterns orders positions utils; do
    check_forbidden_dep "src/${mod}" "execution" "${mod} -> execution"
done

# ===== Summary =====
echo ""
echo -e "${BLUE}=== Architecture Summary ===${NC}"
echo -e "  Violations:       ${VIOLATIONS}"
echo -e "  Exceptions used:  ${EXCEPTIONS_USED}"
echo -e "  Baselined (B-070): ${BASELINED}"

if [ "$VIOLATIONS" -gt 0 ]; then
    echo ""
    echo -e "${RED}FAILED: ${VIOLATIONS} architectural boundary violation(s) found.${NC}"
    echo -e "${RED}Fix the violations or add justified exceptions to ${EXCEPTIONS_FILE}${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}PASSED: All module boundaries respected.${NC}"
exit 0
