#!/bin/bash
# Development database setup.
#
# The build does NOT need this: sqlx macros compile against the committed
# offline cache in .sqlx/ (SQLX_OFFLINE=true). This database is only needed to
# regenerate or check that cache:
#
#   cargo sqlx prepare -- --lib --tests          # after changing a query
#   cargo sqlx prepare --check -- --lib --tests  # what verify_agent_work.sh runs
#
# Modes:
#   ./scripts/setup_db.sh               interactive: asks before recreating
#   ./scripts/setup_db.sh --force       recreate without asking
#   ./scripts/setup_db.sh --if-missing  create only if absent (non-interactive)
#
# Uses the sqlite3 CLI when installed, otherwise python3's built-in sqlite3
# module. Fails loudly if neither is available.

set -euo pipefail

DB_DIR="data/db"
DB_FILE="$DB_DIR/daytrader.db"
SCHEMA_FILE="src/db/schema.sql"
MODE="${1:-interactive}"

# schema.sql omits schema_migrations because MigrationManager creates it at
# runtime, but the sqlx macros in src/db/migrations.rs query it, so the
# database used to prepare the offline cache must have it.
MIGRATIONS_DDL="CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, description TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT (datetime('now')));"

if [ ! -f "$SCHEMA_FILE" ]; then
    echo "ERROR: run from the repository root ($SCHEMA_FILE not found)" >&2
    exit 1
fi

apply_schema() {
    if command -v sqlite3 >/dev/null 2>&1; then
        sqlite3 "$DB_FILE" < "$SCHEMA_FILE"
        sqlite3 "$DB_FILE" "$MIGRATIONS_DDL"
    elif command -v python3 >/dev/null 2>&1; then
        python3 -I - "$DB_FILE" "$SCHEMA_FILE" "$MIGRATIONS_DDL" <<'PY'
import sqlite3
import sys

db_file, schema_file, migrations_ddl = sys.argv[1:4]
with open(schema_file, encoding="utf-8") as f:
    schema = f.read()
conn = sqlite3.connect(db_file)
try:
    conn.executescript(schema)
    conn.executescript(migrations_ddl)
    conn.commit()
finally:
    conn.close()
PY
    else
        echo "ERROR: need either the sqlite3 CLI or python3 to apply the schema" >&2
        exit 1
    fi
}

mkdir -p "$DB_DIR"

if [ -f "$DB_FILE" ]; then
    case "$MODE" in
        --if-missing)
            echo "Database already exists at $DB_FILE; leaving it as is"
            exit 0
            ;;
        --force)
            rm -f "$DB_FILE"
            echo "Removed existing database (--force)"
            ;;
        interactive)
            read -p "Database already exists. Recreate? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                rm -f "$DB_FILE"
                echo "Removed existing database"
            else
                echo "Keeping existing database"
                exit 0
            fi
            ;;
        *)
            echo "ERROR: unknown option '$MODE' (use --force or --if-missing)" >&2
            exit 1
            ;;
    esac
fi

echo "Creating database and applying schema..."
apply_schema

echo "Database setup complete: $DB_FILE"
echo ""
echo "To regenerate or check the sqlx offline cache, export:"
echo "  export DATABASE_URL=\"sqlite://$PWD/$DB_FILE\""
