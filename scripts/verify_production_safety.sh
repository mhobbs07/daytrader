#!/usr/bin/env bash
# =============================================================================
# Production Safety Verification
#
# Enforces the two project rules that previously had no tooling behind them:
#   1. type-safety.md: no .unwrap() / .expect( / panic!( in production code
#   2. concurrency.md:  no std::sync::Mutex / std::sync::RwLock (tokio::sync only;
#      std::sync::Arc remains allowed)
#
# Heuristics (same convention as verify_architecture.sh):
#   - Lines after the first `#[cfg(test)]` in a file are treated as test code.
#   - Comment lines (//, ///, //!) are ignored, so doc examples don't count.
#
# Existing debt is recorded per-file in .governance/production_safety_baseline.txt
# (format: <check>\t<file>\t<count>) and tracked by backlog tasks B-059/B-063.
# The baseline is a ceiling: a file may only shrink or hold its count. Any new
# file or increased count fails. When you fix violations, lower or remove the
# baseline entry in the same commit.
# =============================================================================
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}ERROR: Must be run from project root${NC}"
    exit 1
fi

BASELINE_FILE=".governance/production_safety_baseline.txt"
FAILURES=0
IMPROVEMENTS=0

scan_file() {
    # $1 = file, $2 = extended regex to count
    awk -v pattern="$2" '
        /^[[:space:]]*#\[cfg\(test\)\]/ { intest = 1 }
        intest { next }
        /^[[:space:]]*\/\// { next }
        { cnt += gsub(pattern, "&") }
        END { print cnt + 0 }
    ' "$1"
}

baseline_for() {
    # $1 = check name, $2 = file
    grep -E "^$1	$2	" "$BASELINE_FILE" 2>/dev/null | awk -F'\t' '{print $3}' | head -1
}

run_check() {
    # $1 = check name, $2 = human label, $3 = awk regex, $4 = grep regex (for context display)
    local check="$1" label="$2" regex="$3" grep_regex="$4"
    echo -e "${BLUE}Checking: ${label}...${NC}"
    while IFS= read -r file; do
        local count baseline
        count=$(scan_file "$file" "$regex")
        [ "$count" -eq 0 ] && continue
        baseline=$(baseline_for "$check" "$file")
        baseline=${baseline:-0}
        if [ "$count" -gt "$baseline" ]; then
            echo -e "  ${RED}FAIL: $file has $count (baseline: $baseline)${NC}"
            grep -nE "$grep_regex" "$file" | grep -vE '^[0-9]+:[[:space:]]*//' | head -5 | sed 's/^/    /'
            FAILURES=$((FAILURES + 1))
        elif [ "$count" -lt "$baseline" ]; then
            echo -e "  ${YELLOW}NOTE: $file improved to $count (baseline: $baseline) — lower the baseline entry${NC}"
            IMPROVEMENTS=$((IMPROVEMENTS + 1))
        fi
    done < <(find src daytrader/src-tauri/src -name '*.rs' 2>/dev/null)
}

echo -e "${BLUE}=== Production Safety Verification ===${NC}"
echo ""

run_check "unwrap" \
    "no .unwrap()/.expect()/panic!() in production code" \
    '\\.unwrap\\(\\)|\\.expect\\(|panic!\\(' \
    '\.unwrap\(\)|\.expect\(|panic!\('

run_check "std_sync" \
    "no std::sync Mutex/RwLock (tokio::sync only)" \
    'std::sync::[A-Za-z{, ]*(Mutex|RwLock)' \
    'std::sync::[A-Za-z{, ]*(Mutex|RwLock)'

echo ""
if [ "$FAILURES" -gt 0 ]; then
    echo -e "${RED}FAILED: $FAILURES file(s) exceed the production-safety baseline.${NC}"
    echo -e "${RED}Fix the violations (preferred) or justify a baseline change in review.${NC}"
    exit 1
fi
if [ "$IMPROVEMENTS" -gt 0 ]; then
    echo -e "${YELLOW}$IMPROVEMENTS file(s) are better than baseline — please ratchet the baseline down.${NC}"
fi
echo -e "${GREEN}PASSED: No new production-safety violations.${NC}"
exit 0
