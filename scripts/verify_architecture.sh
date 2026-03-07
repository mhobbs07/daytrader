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
VIOLATIONS=0
EXCEPTIONS_USED=0

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

        # Search for the forbidden import
        local matches
        matches=$(echo "$search_content" | grep -n "use crate::${forbidden}" 2>/dev/null || true)
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
        VIOLATIONS=$((VIOLATIONS + count))
        echo -e "${RED}VIOLATION: ${desc} (${count} occurrence(s))${NC}"
        echo -e "$found_violations" | while IFS= read -r line; do
            [ -n "$line" ] && echo -e "  ${RED}  ${line}${NC}"
        done
    fi
}

# ===== Foundation Layer: utils must not import from upper layers =====
echo -e "${BLUE}[1/6] Checking Foundation Layer (utils, brokers)...${NC}"
check_forbidden_dep "src/utils" "risk" "utils -> risk"
check_forbidden_dep "src/utils" "orders" "utils -> orders"
check_forbidden_dep "src/utils" "positions" "utils -> positions"
check_forbidden_dep "src/utils" "execution" "utils -> execution"
check_forbidden_dep "src/utils" "patterns" "utils -> patterns"
check_forbidden_dep "src/utils" "events" "utils -> events"
check_forbidden_dep "src/utils" "config" "utils -> config"
check_forbidden_dep "src/utils" "db" "utils -> db"

# ===== Infrastructure Layer =====
echo -e "${BLUE}[2/6] Checking Infrastructure Layer (events, config, db)...${NC}"
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
echo -e "${BLUE}[3/6] Checking Business Logic Layer (risk, patterns, positions)...${NC}"
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
echo -e "${BLUE}[4/6] Checking Orchestration Layer (orders, execution)...${NC}"
check_forbidden_dep "src/orders" "execution" "orders -> execution"
check_forbidden_dep "src/orders" "patterns" "orders -> patterns"

# ===== Top of tree: nothing imports execution =====
echo -e "${BLUE}[5/6] Checking that nothing depends on execution (top of tree)...${NC}"
for mod in brokers config db events risk patterns orders positions domain utils infrastructure; do
    check_forbidden_dep "src/${mod}" "execution" "${mod} -> execution"
done

# ===== Domain layer should be isolated =====
echo -e "${BLUE}[6/6] Checking Domain layer isolation...${NC}"
check_forbidden_dep "src/domain" "orders" "domain -> orders"
check_forbidden_dep "src/domain" "risk" "domain -> risk"
check_forbidden_dep "src/domain" "patterns" "domain -> patterns"
check_forbidden_dep "src/domain" "execution" "domain -> execution"
check_forbidden_dep "src/domain" "positions" "domain -> positions"
check_forbidden_dep "src/domain" "events" "domain -> events"
check_forbidden_dep "src/domain" "config" "domain -> config"
check_forbidden_dep "src/domain" "db" "domain -> db"

# ===== Summary =====
echo ""
echo -e "${BLUE}=== Architecture Summary ===${NC}"
echo -e "  Violations:       ${VIOLATIONS}"
echo -e "  Exceptions used:  ${EXCEPTIONS_USED}"

if [ "$VIOLATIONS" -gt 0 ]; then
    echo ""
    echo -e "${RED}FAILED: ${VIOLATIONS} architectural boundary violation(s) found.${NC}"
    echo -e "${RED}Fix the violations or add justified exceptions to ${EXCEPTIONS_FILE}${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}PASSED: All module boundaries respected.${NC}"
exit 0
