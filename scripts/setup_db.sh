#!/bin/bash
# Database initialization script

set -e

DB_DIR="data/db"
DB_FILE="$DB_DIR/daytrader.db"
SCHEMA_FILE="src/db/schema.sql"

echo "Setting up database..."

# Create directory if it doesn't exist
mkdir -p "$DB_DIR"

# Check if database already exists
if [ -f "$DB_FILE" ]; then
    read -p "Database already exists. Recreate? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm "$DB_FILE"
        echo "Removed existing database"
    else
        echo "Keeping existing database"
        exit 0
    fi
fi

# Create database and apply schema
echo "Creating database and applying schema..."
sqlite3 "$DB_FILE" < "$SCHEMA_FILE"

# schema.sql omits schema_migrations because MigrationManager creates it at
# runtime — but sqlx's compile-time query validation needs it to exist already.
sqlite3 "$DB_FILE" "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, description TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT (datetime('now')));"

echo "Database setup complete: $DB_FILE"
echo "You can inspect it with: sqlite3 $DB_FILE"
echo ""
echo "For sqlx compile-time macros, export:"
echo "  export DATABASE_URL=\"sqlite://$PWD/$DB_FILE\""