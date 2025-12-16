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

echo "Database setup complete: $DB_FILE"
echo "You can inspect it with: sqlite3 $DB_FILE"