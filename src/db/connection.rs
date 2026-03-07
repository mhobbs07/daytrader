//! Database connection management and configuration
//!
//! This module provides database connection pooling, initialization, and
//! configuration management for the trading platform's data persistence layer.

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;
use std::time::Duration;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Database file path
    pub database_url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Idle timeout in seconds
    pub idle_timeout: u64,
    /// Maximum lifetime of a connection in seconds
    pub max_lifetime: u64,
    /// Enable WAL mode for better concurrent access
    pub enable_wal: bool,
    /// Enable foreign key constraints
    pub enable_foreign_keys: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "trading.db".to_string(),
            max_connections: 10,
            min_connections: 2,
            connect_timeout: 30,
            idle_timeout: 600,
            max_lifetime: 1800,
            enable_wal: true,
            enable_foreign_keys: true,
        }
    }
}

impl DatabaseConfig {
    /// Creates a new database config with custom database path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            database_url: path.as_ref().to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    /// Creates a config for in-memory database (for testing)
    pub fn in_memory() -> Self {
        Self {
            database_url: ":memory:".to_string(),
            ..Default::default()
        }
    }
}

/// Main database connection manager
pub struct Database {
    /// Connection pool
    pub pool: SqlitePool,
    /// Configuration
    config: DatabaseConfig,
}

impl Database {
    /// Creates a new database connection with the given path
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, sqlx::Error> {
        Self::with_config(DatabaseConfig::with_path(path)).await
    }

    /// Creates a new database connection with custom configuration
    pub async fn with_config(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .max_lifetime(Duration::from_secs(config.max_lifetime))
            .connect(&config.database_url)
            .await?;

        let db = Self { pool, config };

        // Initialize database schema and settings
        db.initialize().await?;

        Ok(db)
    }

    /// Creates an in-memory database (primarily for testing)
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        Self::with_config(DatabaseConfig::in_memory()).await
    }

    /// Initializes the database with schema and optimizations
    async fn initialize(&self) -> Result<(), sqlx::Error> {
        // Enable WAL mode for better concurrent access
        if self.config.enable_wal {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&self.pool)
                .await?;
        }

        // Enable foreign key constraints
        if self.config.enable_foreign_keys {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&self.pool)
                .await?;
        }

        // Performance optimizations
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&self.pool)
            .await?;

        sqlx::query("PRAGMA cache_size = 1000000")
            .execute(&self.pool)
            .await?;

        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&self.pool)
            .await?;

        sqlx::query("PRAGMA mmap_size = 268435456")
            .execute(&self.pool)
            .await?;

        // Create schema
        self.create_schema().await?;

        Ok(())
    }

    /// Creates the database schema
    async fn create_schema(&self) -> Result<(), sqlx::Error> {
        let schema = include_str!("../db/schema.sql");
        sqlx::query(schema).execute(&self.pool).await?;
        Ok(())
    }

    /// Gets database statistics
    pub async fn get_stats(&self) -> Result<DatabaseStats, sqlx::Error> {
        let stats = sqlx::query_as!(
            DatabaseStats,
            r#"
            SELECT
                (SELECT COUNT(*) FROM pattern_signals) as pattern_signals_count,
                (SELECT COUNT(*) FROM pattern_stats) as pattern_stats_count,
                (SELECT COUNT(*) FROM pattern_trade_history) as trade_history_count,
                (SELECT COUNT(*) FROM risk_events) as risk_events_count,
                (SELECT COUNT(*) FROM daily_risk_metrics) as daily_metrics_count
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Performs database maintenance (cleanup old data, optimize)
    pub async fn maintenance(&self) -> Result<(), sqlx::Error> {
        // Vacuum database to reclaim space
        sqlx::query("VACUUM").execute(&self.pool).await?;

        // Analyze tables for query optimization
        sqlx::query("ANALYZE").execute(&self.pool).await?;

        Ok(())
    }

    /// Closes the database connection pool
    pub async fn close(&self) -> Result<(), sqlx::Error> {
        self.pool.close().await;
        Ok(())
    }
}

/// Database statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DatabaseStats {
    pub pattern_signals_count: i64,
    pub pattern_stats_count: i64,
    pub trade_history_count: i64,
    pub risk_events_count: i64,
    pub daily_metrics_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let db = Database::new(db_path).await.unwrap();
        let stats = db.get_stats().await.unwrap();

        assert_eq!(stats.pattern_signals_count, 0);
        assert_eq!(stats.pattern_stats_count, 0);

        db.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_database() {
        let db = Database::in_memory().await.unwrap();
        let stats = db.get_stats().await.unwrap();

        assert_eq!(stats.pattern_signals_count, 0);

        db.close().await.unwrap();
    }
}