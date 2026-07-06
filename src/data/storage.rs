//! Database storage for market data
//!
//! This module provides persistent storage for candles and quotes using SQLite.
//! Uses sqlx for async database operations with connection pooling.

use crate::brokers::types::Symbol;
use crate::events::types::Candle;
use crate::utils::errors::{AppError, DatabaseError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

/// Database storage for candle data
///
/// Provides async methods for storing and retrieving candle data with
/// efficient batch operations and connection pooling.
///
/// # Examples
///
/// ```no_run
/// use daytrader::data::storage::CandleStorage;
/// use sqlx::SqlitePool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = SqlitePool::connect("sqlite::memory:").await?;
/// let storage = CandleStorage::new(pool);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CandleStorage {
    pool: SqlitePool,
}

impl CandleStorage {
    /// Create a new candle storage instance
    ///
    /// # Arguments
    ///
    /// * `pool` - SQLite connection pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a single candle in the database
    ///
    /// Uses INSERT OR REPLACE to handle duplicate timestamps gracefully.
    ///
    /// # Arguments
    ///
    /// * `candle` - Candle data to store
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::data::storage::CandleStorage;
    /// use daytrader::events::types::Candle;
    /// use daytrader::brokers::types::{Symbol, Price};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    /// use sqlx::SqlitePool;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = SqlitePool::connect("sqlite::memory:").await?;
    /// let storage = CandleStorage::new(pool);
    ///
    /// let candle = Candle {
    ///     symbol: Symbol::new("AAPL"),
    ///     timestamp: Utc::now(),
    ///     open: Price::new(Decimal::new(150, 0)).unwrap(),
    ///     high: Price::new(Decimal::new(155, 0)).unwrap(),
    ///     low: Price::new(Decimal::new(148, 0)).unwrap(),
    ///     close: Price::new(Decimal::new(152, 0)).unwrap(),
    ///     volume: 1_000_000,
    /// };
    ///
    /// storage.store_candle(&candle).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store_candle(&self, candle: &Candle) -> Result<()> {
        let symbol = candle.symbol.as_str();
        let timestamp = candle.timestamp;
        let open = candle.open.value().to_string();
        let high = candle.high.value().to_string();
        let low = candle.low.value().to_string();
        let close = candle.close.value().to_string();
        let volume = candle.volume as i64;
        let broker = "alpaca";

        sqlx::query(
            r"
            INSERT INTO candles (symbol, timestamp, open, high, low, close, volume, broker)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(symbol, timestamp, broker) DO UPDATE SET
                open = excluded.open,
                high = excluded.high,
                low = excluded.low,
                close = excluded.close,
                volume = excluded.volume
            ",
        )
        .bind(symbol)
        .bind(timestamp)
        .bind(open)
        .bind(high)
        .bind(low)
        .bind(close)
        .bind(volume)
        .bind(broker)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    /// Store multiple candles in a single transaction
    ///
    /// More efficient than multiple individual stores for batch operations.
    ///
    /// # Arguments
    ///
    /// * `candles` - Slice of candles to store
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails. Transaction is rolled back on error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::data::storage::CandleStorage;
    /// use daytrader::events::types::Candle;
    /// use daytrader::brokers::types::{Symbol, Price};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    /// use sqlx::SqlitePool;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = SqlitePool::connect("sqlite::memory:").await?;
    /// let storage = CandleStorage::new(pool);
    ///
    /// let candles = vec![
    ///     Candle {
    ///         symbol: Symbol::new("AAPL"),
    ///         timestamp: Utc::now(),
    ///         open: Price::new(Decimal::new(150, 0)).unwrap(),
    ///         high: Price::new(Decimal::new(155, 0)).unwrap(),
    ///         low: Price::new(Decimal::new(148, 0)).unwrap(),
    ///         close: Price::new(Decimal::new(152, 0)).unwrap(),
    ///         volume: 1_000_000,
    ///     },
    /// ];
    ///
    /// storage.store_candles(&candles).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store_candles(&self, candles: &[Candle]) -> Result<()> {
        if candles.is_empty() {
            return Ok(());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DatabaseError::TransactionFailed(e.to_string()))?;

        for candle in candles {
            let symbol = candle.symbol.as_str();
            let timestamp = candle.timestamp;
            let open = candle.open.value().to_string();
            let high = candle.high.value().to_string();
            let low = candle.low.value().to_string();
            let close = candle.close.value().to_string();
            let volume = candle.volume as i64;
            let broker = "alpaca";

            sqlx::query(
                r"
                INSERT INTO candles (symbol, timestamp, open, high, low, close, volume, broker)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(symbol, timestamp, broker) DO UPDATE SET
                    open = excluded.open,
                    high = excluded.high,
                    low = excluded.low,
                    close = excluded.close,
                    volume = excluded.volume
                ",
            )
            .bind(symbol)
            .bind(timestamp)
            .bind(open)
            .bind(high)
            .bind(low)
            .bind(close)
            .bind(volume)
            .bind(broker)
            .execute(&mut *tx)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| DatabaseError::TransactionFailed(e.to_string()))?;

        Ok(())
    }

    /// Get recent candles for a symbol
    ///
    /// Returns candles in reverse chronological order (newest first).
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get candles for
    /// * `count` - Maximum number of candles to return
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::data::storage::CandleStorage;
    /// use daytrader::brokers::types::Symbol;
    /// use sqlx::SqlitePool;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = SqlitePool::connect("sqlite::memory:").await?;
    /// let storage = CandleStorage::new(pool);
    ///
    /// let symbol = Symbol::new("AAPL");
    /// let candles = storage.get_recent_candles(&symbol, 100).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_recent_candles(&self, symbol: &Symbol, count: usize) -> Result<Vec<Candle>> {
        let rows = sqlx::query(
            r"
            SELECT symbol, timestamp, open, high, low, close, volume
            FROM candles
            WHERE symbol = ?
            ORDER BY timestamp DESC
            LIMIT ?
            ",
        )
        .bind(symbol.as_str())
        .bind(count as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut candles = Vec::with_capacity(rows.len());
        for row in rows {
            let candle = row_to_candle(&row)?;
            candles.push(candle);
        }

        Ok(candles)
    }

    /// Get candles for a symbol within a time range
    ///
    /// Returns candles in chronological order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get candles for
    /// * `start` - Start time (inclusive)
    /// * `end` - End time (inclusive)
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails
    pub async fn get_candles_in_range(
        &self,
        symbol: &Symbol,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Candle>> {
        let rows = sqlx::query(
            r"
            SELECT symbol, timestamp, open, high, low, close, volume
            FROM candles
            WHERE symbol = ? AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            ",
        )
        .bind(symbol.as_str())
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut candles = Vec::with_capacity(rows.len());
        for row in rows {
            let candle = row_to_candle(&row)?;
            candles.push(candle);
        }

        Ok(candles)
    }

    /// Get the count of candles for a symbol
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to count candles for
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails
    pub async fn get_candle_count(&self, symbol: &Symbol) -> Result<i64> {
        let row = sqlx::query(
            r"
            SELECT COUNT(*) as count
            FROM candles
            WHERE symbol = ?
            ",
        )
        .bind(symbol.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let count: i64 = row
            .try_get("count")
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(count)
    }

    /// Delete old candles beyond a retention period
    ///
    /// Useful for maintaining database size.
    ///
    /// # Arguments
    ///
    /// * `before` - Delete candles older than this timestamp
    ///
    /// # Returns
    ///
    /// Number of rows deleted
    ///
    /// # Errors
    ///
    /// Returns error if database operation fails
    pub async fn delete_old_candles(&self, before: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query(
            r"
            DELETE FROM candles
            WHERE timestamp < ?
            ",
        )
        .bind(before)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

/// Convert a database row to a Candle struct
fn row_to_candle(row: &sqlx::sqlite::SqliteRow) -> Result<Candle> {
    use crate::brokers::types::Price;

    let symbol_str: String = row
        .try_get("symbol")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    let symbol = Symbol::new(symbol_str).map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let timestamp: DateTime<Utc> = row
        .try_get("timestamp")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    let open_str: String = row
        .try_get("open")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    let open = Price::new(
        Decimal::from_str(&open_str).map_err(|e| DatabaseError::QueryFailed(e.to_string()))?,
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let high_str: String = row
        .try_get("high")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    let high = Price::new(
        Decimal::from_str(&high_str).map_err(|e| DatabaseError::QueryFailed(e.to_string()))?,
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let low_str: String = row
        .try_get("low")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    let low = Price::new(
        Decimal::from_str(&low_str).map_err(|e| DatabaseError::QueryFailed(e.to_string()))?,
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let close_str: String = row
        .try_get("close")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    let close = Price::new(
        Decimal::from_str(&close_str).map_err(|e| DatabaseError::QueryFailed(e.to_string()))?,
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let volume: i64 = row
        .try_get("volume")
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    Ok(Candle {
        symbol,
        timestamp,
        open,
        high,
        low,
        close,
        volume: volume as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::Price;
    use chrono::Duration;

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool");

        // Create candles table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS candles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                open TEXT NOT NULL,
                high TEXT NOT NULL,
                low TEXT NOT NULL,
                close TEXT NOT NULL,
                volume INTEGER NOT NULL,
                broker TEXT NOT NULL,
                UNIQUE(symbol, timestamp, broker)
            )
            ",
        )
        .execute(&pool)
        .await
        .expect("Failed to create table");

        pool
    }

    fn create_test_candle(symbol: Symbol, close: i64) -> Candle {
        Candle {
            symbol,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(close - 1, 0)).unwrap(),
            high: Price::new(Decimal::new(close + 1, 0)).unwrap(),
            low: Price::new(Decimal::new(close - 2, 0)).unwrap(),
            close: Price::new(Decimal::new(close, 0)).unwrap(),
            volume: 1000,
        }
    }

    #[tokio::test]
    async fn test_store_and_retrieve_candle() {
        let pool = create_test_pool().await;
        let storage = CandleStorage::new(pool);
        let symbol = Symbol::new("AAPL").unwrap();

        let candle = create_test_candle(symbol.clone(), 150);
        storage.store_candle(&candle).await.unwrap();

        let retrieved = storage.get_recent_candles(&symbol, 1).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].close, candle.close);
    }

    #[tokio::test]
    async fn test_batch_store() {
        let pool = create_test_pool().await;
        let storage = CandleStorage::new(pool);
        let symbol = Symbol::new("AAPL").unwrap();
        let base_time = Utc::now();

        // Use distinct timestamps (1 minute apart) to avoid ON CONFLICT deduplication
        let candles: Vec<Candle> = (0..10)
            .map(|i| {
                let mut candle = create_test_candle(symbol.clone(), 150 + i);
                candle.timestamp = base_time - Duration::minutes(10 - i);
                candle
            })
            .collect();

        storage.store_candles(&candles).await.unwrap();

        let count = storage.get_candle_count(&symbol).await.unwrap();
        assert_eq!(count, 10);
    }

    #[tokio::test]
    async fn test_get_candles_in_range() {
        let pool = create_test_pool().await;
        let storage = CandleStorage::new(pool);
        let symbol = Symbol::new("AAPL").unwrap();

        let now = Utc::now();
        let mut candles = Vec::new();

        for i in 0..5 {
            let mut candle = create_test_candle(symbol.clone(), 150 + i);
            candle.timestamp = now - Duration::hours(5 - i);
            candles.push(candle);
        }

        storage.store_candles(&candles).await.unwrap();

        let start = now - Duration::hours(3);
        let end = now;
        let retrieved = storage
            .get_candles_in_range(&symbol, start, end)
            .await
            .unwrap();

        assert_eq!(retrieved.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_old_candles() {
        let pool = create_test_pool().await;
        let storage = CandleStorage::new(pool);
        let symbol = Symbol::new("AAPL").unwrap();

        let now = Utc::now();
        let mut candles = Vec::new();

        for i in 0..5 {
            let mut candle = create_test_candle(symbol.clone(), 150 + i);
            candle.timestamp = now - Duration::hours(5 - i);
            candles.push(candle);
        }

        storage.store_candles(&candles).await.unwrap();

        let cutoff = now - Duration::hours(2);
        let deleted = storage.delete_old_candles(cutoff).await.unwrap();

        assert_eq!(deleted, 3); // Should delete 3 old candles

        let remaining = storage.get_candle_count(&symbol).await.unwrap();
        assert_eq!(remaining, 2);
    }
}
