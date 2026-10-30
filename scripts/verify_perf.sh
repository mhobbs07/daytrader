#!/usr/bin/env bash
# =============================================================================
# Performance Bench Verification (B-103 / R0.5)
#
# Default (compile gate): cargo bench --no-run
#   Fails if any criterion bench in benches/ no longer compiles against the
#   current public API. Fast; suitable for CI/pre-commit once wired in (B-089).
#
# Optional quick run: ./scripts/verify_perf.sh --quick
#   After the compile gate, executes every bench once in criterion's fast
#   "test" mode (cargo bench -- --test): each benchmark body runs a single
#   iteration and is checked for panics. No timing statistics are produced.
#
# Requires DATABASE_URL pointing at the dev db for sqlx macros; if unset it is
# derived from the repo root (run ./scripts/setup_db.sh first if missing).
# Not part of verify_agent_work.sh by design.
# =============================================================================
set -euo pipefail

if [ ! -f "Cargo.toml" ]; then
    echo "ERROR: Must be run from project root (where Cargo.toml is)"
    exit 1
fi

QUICK=0
for arg in "$@"; do
    case "$arg" in
        --quick) QUICK=1 ;;
        -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $arg"; exit 2 ;;
    esac
done

if [ -z "${DATABASE_URL:-}" ]; then
    export DATABASE_URL="sqlite://$(pwd)/data/db/daytrader.db"
fi

echo "[1/2] Compiling benches (cargo bench --no-run)..."
cargo bench --no-run

if [ "$QUICK" -eq 1 ]; then
    echo "[2/2] Quick run (cargo bench -- --test)..."
    cargo bench -- --test
else
    echo "[2/2] Quick run skipped (pass --quick to enable)"
fi

echo "verify_perf: OK"
