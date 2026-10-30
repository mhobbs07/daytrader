//! Database connection management and configuration
//!
//! This module provides database connection pooling, initialization, and
//! configuration management for the trading platform's data persistence layer.

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

/// Page cache per connection, in KiB (64 MiB).
///
/// SQLite's `cache_size` pragma counts pages when positive and KiB when
/// negative, so this is passed as `-CACHE_SIZE_KIB`.
const CACHE_SIZE_KIB: u32 = 64_000;

/// How long a connection waits on a locked database before returning `SQLITE_BUSY`.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Memory-mapped I/O window per connection (256 MiB).
const MMAP_SIZE_BYTES: u64 = 268_435_456;

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

    /// Returns true when `database_url` names an in-memory database.
    fn is_in_memory(&self) -> bool {
        self.database_url.contains(":memory:") || self.database_url.contains("mode=memory")
    }

    /// Builds the per-connection options.
    ///
    /// Pragmas are set here rather than with one-off `PRAGMA` queries after
    /// the pool opens, because a query on the pool reaches only the single
    /// connection that happens to run it. Options apply to every connection
    /// the pool creates.
    fn connect_options(&self) -> Result<SqliteConnectOptions, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(&self.database_url)?
            .create_if_missing(true)
            .foreign_keys(self.enable_foreign_keys)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .pragma("cache_size", format!("-{CACHE_SIZE_KIB}"))
            .pragma("temp_store", "MEMORY")
            .pragma("mmap_size", MMAP_SIZE_BYTES.to_string());

        // WAL does not apply to in-memory databases.
        if self.enable_wal && !self.is_in_memory() {
            Ok(options.journal_mode(SqliteJournalMode::Wal))
        } else {
            Ok(options)
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
    ///
    /// The database file is created if it does not exist. Its parent
    /// directory must already exist.
    pub async fn with_config(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout))
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .max_lifetime(Duration::from_secs(config.max_lifetime))
            .connect_with(config.connect_options()?)
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

    /// Initializes the database schema
    ///
    /// Connection settings (WAL, foreign keys, synchronous, cache size, and
    /// so on) are applied per connection by [`DatabaseConfig::connect_options`].
    async fn initialize(&self) -> Result<(), sqlx::Error> {
        self.create_schema().await
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

    #[tokio::test]
    async fn test_new_missing_file_creates_database() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("fresh.db");
        assert!(!db_path.exists());

        let db = Database::new(&db_path).await.unwrap();

        assert!(db_path.exists(), "Database::new must create a missing file");
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.pattern_stats_count, 0);

        db.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_connections_all_apply_pragmas() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("pragmas.db")).await.unwrap();

        // Hold several connections at once so each is a distinct connection,
        // not the same one handed back by the pool.
        let mut connections = Vec::new();
        for _ in 0..3 {
            connections.push(db.pool.acquire().await.unwrap());
        }

        for conn in &mut connections {
            let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
                .fetch_one(&mut **conn)
                .await
                .unwrap();
            let cache_size: i64 = sqlx::query_scalar("PRAGMA cache_size")
                .fetch_one(&mut **conn)
                .await
                .unwrap();
            let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&mut **conn)
                .await
                .unwrap();
            let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&mut **conn)
                .await
                .unwrap();

            assert_eq!(
                synchronous, 1,
                "synchronous must be NORMAL on every connection"
            );
            assert_eq!(cache_size, -i64::from(CACHE_SIZE_KIB));
            assert_eq!(foreign_keys, 1);
            assert_eq!(journal_mode, "wal");
        }

        drop(connections);
        db.close().await.unwrap();
    }
}
