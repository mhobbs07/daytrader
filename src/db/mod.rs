//! Database operations and schema management
//!
//! This module provides comprehensive database integration for the trading platform:
//! - Database connection management with connection pooling
//! - Schema migrations and initialization
//! - Repository patterns for data access
//! - Pattern storage, signal history, and performance analytics
//! - Optimized queries with proper indexing
//!
//! # Architecture
//!
//! The database layer follows a repository pattern with:
//! - **Connection Manager**: Handles database connections and pooling
//! - **Repositories**: Domain-specific data access objects
//! - **Migrations**: Schema versioning and updates
//! - **Models**: Database entity representations
//!
//! # Performance Features
//!
//! - Connection pooling with sqlx
//! - Prepared statements for query optimization
//! - Batch operations for bulk inserts
//! - Proper indexing on frequently queried columns
//! - WAL mode for concurrent reads/writes
//!
//! # Examples
//!
//! ```
//! use daytrader::db::{Database, PatternRepository, SignalRepository};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let db = Database::new("trading.db").await?;
//!
//! // Store pattern signal
//! let signal_repo = SignalRepository::new(db.pool.clone());
//! signal_repo.store_signal(&trading_signal).await?;
//!
//! // Query pattern performance
//! let pattern_repo = PatternRepository::new(db.pool.clone());
//! let stats = pattern_repo.get_pattern_stats("hammer").await?;
//!
//! # Ok(())
//! # }
//! ```

pub mod connection;
pub mod models;
pub mod repositories;
pub mod migrations;

// Re-export commonly used types
pub use connection::{Database, DatabaseConfig};
pub use repositories::{PatternRepository, SignalRepository, PerformanceRepository};

// Re-export models for convenience
pub use models::{
    PatternSignal, PatternStats, RollingStats, TradeRecord,
    RiskAdjustmentAudit, SystemPerformance,
};