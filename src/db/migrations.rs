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

        // For now, we only have version 1 (initial schema)
        // Future migrations would be added here

        tracing::info!("Database schema is at version {}", current_version);
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