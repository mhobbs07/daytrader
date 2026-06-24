#!/bin/bash
# Database initialization script.
#
# Modes:
#   ./scripts/setup_db.sh           interactive — prompts before recreating
#   ./scripts/setup_db.sh --force   non-interactive — always recreates
#   ./scripts/setup_db.sh --if-missing  non-interactive — only creates if absent
#
# Falls back to python3's sqlite3 module if the sqlite3 CLI is not installed.

set -e

DB_DIR="data/db"
DB_FILE="$DB_DIR/daytrader.db"
SCHEMA_FILE="src/db/schema.sql"
MODE="${1:-interactive}"

apply_schema() {
    if command -v sqlite3 >/dev/null 2>&1; then
        sqlite3 "$DB_FILE" < "$SCHEMA_FILE"
    elif command -v python3 >/dev/null 2>&1; then
        python3 - "$DB_FILE" "$SCHEMA_FILE" <<'PY'
import sqlite3, sys
db_file, schema_file = sys.argv[1], sys.argv[2]
with open(schema_file) as f:
    schema = f.read()
conn = sqlite3.connect(db_file)
conn.executescript(schema)
conn.commit()
conn.close()
PY
    else
        echo "ERROR: neither sqlite3 CLI nor python3 is available to apply schema" >&2
        exit 1
    fi
}

echo "Setting up database..."
mkdir -p "$DB_DIR"

if [ -f "$DB_FILE" ]; then
    case "$MODE" in
        --if-missing)
            echo "Database already exists at $DB_FILE — leaving as-is"
            exit 0
            ;;
        --force)
            rm "$DB_FILE"
            echo "Removed existing database (--force)"
            ;;
        *)
            read -p "Database already exists. Recreate? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                rm "$DB_FILE"
                echo "Removed existing database"
            else
                echo "Keeping existing database"
                exit 0
            fi
            ;;
    esac
fi

echo "Creating database and applying schema..."
apply_schema

echo "Database setup complete: $DB_FILE"