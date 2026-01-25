//! Database migrations and schema management
//!
//! This module handles database schema versioning and migrations
//! to ensure smooth upgrades and compatibility.

use sqlx::SqlitePool;

/// Database migration manager
pub struct MigrationManager {
    pool: SqlitePool,
}

impl MigrationManager {
    /// Creates a new migration manager
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Gets current schema version
    pub async fn get_schema_version(&self) -> Result<i32, sqlx::Error> {
        // Check if migrations table exists
        let table_exists = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'"
        )
        .fetch_optional(&self.pool)
        .await?
        .is_some();

        if !table_exists {
            // Create migrations table and set initial version
            sqlx::query!(
                r#"
                CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#
            )
            .execute(&self.pool)
            .await?;

            // Insert version 1 (initial schema)
            sqlx::query!(
                "INSERT INTO schema_migrations (version, description) VALUES (1, 'Initial schema')",
            )
            .execute(&self.pool)
            .await?;

            return Ok(1);
        }

        // Get current version
        let row = sqlx::query!("SELECT MAX(version) as version FROM schema_migrations")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.version.unwrap_or(1))
    }

    /// Runs pending migrations
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        let current_version = self.get_schema_version().await?;

        // Apply migrations in order
        if current_version < 2 {
            self.migrate_to_v2().await?;
        }

        tracing::info!("Database schema is at version {}", self.get_schema_version().await?);
        Ok(())
    }

    /// Migration to version 2: Add trading_signals and indicator_values tables
    async fn migrate_to_v2(&self) -> Result<(), sqlx::Error> {
        tracing::info!("Migrating database schema to version 2...");

        // Create trading_signals table
        sqlx::query!(
            r#"
            CREATE TABLE IF NOT EXISTS trading_signals (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                signal_type TEXT NOT NULL CHECK(signal_type IN ('buy', 'sell')),
                confidence REAL NOT NULL CHECK(confidence >= 0 AND confidence <= 1),
                price_level REAL NOT NULL,
                generated_at TIMESTAMP NOT NULL,
                
                -- Market context fields
                trend_type TEXT,
                trend_strength REAL,
                volatility REAL,
                market_strength REAL,
                
                -- Components and metadata as JSON
                components TEXT NOT NULL,
                metadata TEXT,
                
                -- Outcome tracking
                acted_upon BOOLEAN DEFAULT FALSE,
                outcome TEXT CHECK(outcome IN ('win', 'loss', 'pending', 'ignored')),
                order_id INTEGER,
                FOREIGN KEY (order_id) REFERENCES orders(id)
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for trading_signals
        sqlx::query!(
            "CREATE INDEX IF NOT EXISTS idx_trading_signals_symbol_time ON trading_signals(symbol, generated_at DESC)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query!(
            "CREATE INDEX IF NOT EXISTS idx_trading_signals_confidence ON trading_signals(confidence DESC)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query!(
            "CREATE INDEX IF NOT EXISTS idx_trading_signals_outcome ON trading_signals(outcome)"
        )
        .execute(&self.pool)
        .await?;

        // Create indicator_values table
        sqlx::query!(
            r#"
            CREATE TABLE IF NOT EXISTS indicator_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                indicator_name TEXT NOT NULL,
                value REAL NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                metadata TEXT,
                UNIQUE(symbol, indicator_name, timestamp)
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create index for indicator_values
        sqlx::query!(
            "CREATE INDEX IF NOT EXISTS idx_indicator_values_symbol_time ON indicator_values(symbol, timestamp DESC)"
        )
        .execute(&self.pool)
        .await?;

        // Record migration
        sqlx::query!(
            "INSERT INTO schema_migrations (version, description) VALUES (2, 'Add trading_signals and indicator_values tables')"
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Migration to version 2 completed successfully");
        Ok(())
    }

    /// Validates database schema integrity
    pub async fn validate_schema(&self) -> Result<(), sqlx::Error> {
        // Check that all required tables exist
        let required_tables = vec![
            "accounts",
            "candles",
            "positions",
            "orders",
            "trades",
            "pattern_signals",
            "pattern_stats",
            "pattern_rolling_stats",
            "pattern_trade_history",
            "risk_adjustment_audit",
            "daily_risk_metrics",
            "position_risk_tracking",
            "pdt_tracking",
            "watchlists",
            "watchlist_items",
            "alerts",
            "audit_log",
            "risk_events",
            "app_settings",
            "trading_signals",
            "indicator_values",
        ];

        for table in required_tables {
            let exists = sqlx::query!(
                "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
                table
            )
            .fetch_optional(&self.pool)
            .await?
            .is_some();

            if !exists {
                return Err(sqlx::Error::RowNotFound); // Custom error would be better
            }
        }

        Ok(())
    }
}