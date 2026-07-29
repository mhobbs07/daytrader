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
                CREATE TABLE IF NOT EXISTS schema_migrations (
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

        Ok(row.version.unwrap_or(1) as i32)
    }

    /// Runs pending migrations
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        let current_version = self.get_schema_version().await?;

        // Apply migrations in order
        if current_version < 2 {
            self.migrate_to_v2().await?;
        }
        if current_version < 3 {
            self.migrate_to_v3().await?;
        }
        if current_version < 4 {
            self.migrate_to_v4().await?;
        }

        tracing::info!(
            "Database schema is at version {}",
            self.get_schema_version().await?
        );
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

    /// Migration to version 3: Make candle corporate-action adjustment explicit
    ///
    /// Bars fetched under different [`BarAdjustment`] modes are different
    /// series for the same instrument and instant. The v1 uniqueness key
    /// `(symbol, timestamp, broker)` could not tell them apart, so storing an
    /// adjusted bar over a previously-stored raw one silently overwrote it and
    /// left the table with mixed, unlabelled provenance.
    ///
    /// Adds an `adjustment` discriminator and widens the uniqueness key.
    /// Existing rows are backfilled as `'raw'`, which is historically accurate:
    /// every fetch before this migration hardcoded `adjustment=raw`.
    ///
    /// SQLite cannot alter a UNIQUE constraint in place, so the table is
    /// rebuilt. DDL is issued through the unchecked `sqlx::query` rather than
    /// the `query!` macro: these statements bind no parameters and return no
    /// columns, so the macro would add offline-cache entries without adding
    /// any verification.
    ///
    /// [`BarAdjustment`]: crate::brokers::traits::BarAdjustment
    async fn migrate_to_v3(&self) -> Result<(), sqlx::Error> {
        tracing::info!("Migrating database schema to version 3...");

        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS candles_v3 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume INTEGER NOT NULL,
                broker TEXT NOT NULL,
                adjustment TEXT NOT NULL DEFAULT 'raw'
                    CHECK(adjustment IN ('raw', 'split', 'dividend', 'all')),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(symbol, timestamp, broker, adjustment)
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Pre-v3 rows were all fetched with adjustment=raw.
        sqlx::query(
            r"
            INSERT OR IGNORE INTO candles_v3
                (id, symbol, timestamp, open, high, low, close, volume, broker, adjustment, created_at)
            SELECT id, symbol, timestamp, open, high, low, close, volume, broker, 'raw', created_at
            FROM candles
            ",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("DROP TABLE candles")
            .execute(&self.pool)
            .await?;

        sqlx::query("ALTER TABLE candles_v3 RENAME TO candles")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_candles_symbol_time ON candles(symbol, timestamp DESC)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_candles_adjustment ON candles(symbol, adjustment, timestamp DESC)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query!(
            "INSERT INTO schema_migrations (version, description) VALUES (3, 'Add candle corporate-action adjustment discriminator')"
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Migration to version 3 completed successfully");
        Ok(())
    }

    /// Migration to version 4: Add the corporate-action factor table
    ///
    /// Step 1 (v3) made the corporate-action *basis* of a stored bar
    /// explicit, but adjusted bars are only correct as of the moment they
    /// were fetched — a subsequent split changes what the vendor returns for
    /// the same historical window, so an adjusted series is not
    /// reproducible, and it cannot be spliced with raw live bars without a
    /// discontinuity at the split.
    ///
    /// The fix is to store bars raw and adjust at query time from a
    /// separately-recorded ledger of corporate actions, rather than ever
    /// rewriting stored prices in place. This migration adds that ledger.
    /// `crate::data::corporate_actions` computes the adjustment factors from
    /// its rows; this migration only creates the table.
    ///
    /// A row's `action_type` determines which of the type-specific columns
    /// are populated: `split_from`/`split_to` for `'split'`,
    /// `cash_amount` for `'cash_dividend'`. The CHECK constraint enforces
    /// that exactly the right columns are set for each type, so a
    /// half-populated row (e.g. a `'split'` with no ratio) cannot exist.
    ///
    /// `cash_amount` is `REAL`, matching the pre-existing (and already
    /// imprecise) convention for money elsewhere in this schema — see the
    /// `candles` price columns. The planned `Money<6>` integer-micros
    /// migration (B-087, not yet landed) should convert this column
    /// alongside the rest when it does.
    ///
    /// DDL is issued through the unchecked `sqlx::query` rather than the
    /// `query!` macro, matching `migrate_to_v3`: these statements bind no
    /// parameters and return no columns, so the macro would add
    /// offline-cache entries without adding any verification.
    async fn migrate_to_v4(&self) -> Result<(), sqlx::Error> {
        tracing::info!("Migrating database schema to version 4...");

        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS corporate_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                ex_date DATE NOT NULL,
                action_type TEXT NOT NULL CHECK(action_type IN ('split', 'cash_dividend')),
                split_from INTEGER,
                split_to INTEGER,
                -- REAL pending the B-087 Money<6> integer-micros migration;
                -- convert this column alongside the rest when it lands.
                cash_amount REAL,
                source TEXT NOT NULL,
                ingested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                CHECK (
                    (action_type = 'split'
                        AND split_from IS NOT NULL AND split_from > 0
                        AND split_to IS NOT NULL AND split_to > 0
                        AND cash_amount IS NULL)
                    OR
                    (action_type = 'cash_dividend'
                        AND cash_amount IS NOT NULL AND cash_amount > 0
                        AND split_from IS NULL AND split_to IS NULL)
                ),
                UNIQUE(symbol, ex_date, action_type)
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_corporate_actions_symbol_date ON corporate_actions(symbol, ex_date)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query!(
            "INSERT INTO schema_migrations (version, description) VALUES (4, 'Add corporate_actions factor table for query-time bar adjustment')"
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Migration to version 4 completed successfully");
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
            "corporate_actions",
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// Builds a pre-v3 database: the original candles shape, plus the
    /// migrations bookkeeping table pinned at version 2.
    async fn v2_pool_with_candles() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory pool");

        sqlx::query(
            r"
            CREATE TABLE candles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume INTEGER NOT NULL,
                broker TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(symbol, timestamp, broker)
            )
            ",
        )
        .execute(&pool)
        .await
        .expect("create v1 candles");

        sqlx::query(
            r"
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            ",
        )
        .execute(&pool)
        .await
        .expect("create schema_migrations");

        sqlx::query("INSERT INTO schema_migrations (version, description) VALUES (2, 'test')")
            .execute(&pool)
            .await
            .expect("seed version 2");

        pool
    }

    async fn insert_legacy_candle(pool: &SqlitePool, symbol: &str, close: f64) {
        sqlx::query(
            r"
            INSERT INTO candles (symbol, timestamp, open, high, low, close, volume, broker)
            VALUES (?, '2024-06-10T13:30:00Z', ?, ?, ?, ?, 1000, 'alpaca')
            ",
        )
        .bind(symbol)
        .bind(close)
        .bind(close)
        .bind(close)
        .bind(close)
        .execute(pool)
        .await
        .expect("insert legacy candle");
    }

    #[tokio::test]
    async fn test_migrate_v3_preserves_existing_rows_as_raw() {
        // Pre-v3 fetches hardcoded adjustment=raw, so surviving rows must be
        // labelled raw — anything else would misrepresent their provenance and
        // let unadjusted bars masquerade as analysis-safe.
        let pool = v2_pool_with_candles().await;
        insert_legacy_candle(&pool, "AAPL", 400.0).await;
        insert_legacy_candle(&pool, "MSFT", 300.0).await;

        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v3 migration");

        let rows = sqlx::query("SELECT symbol, adjustment FROM candles ORDER BY symbol")
            .fetch_all(&pool)
            .await
            .expect("read migrated candles");

        assert_eq!(rows.len(), 2, "no rows may be lost by the rebuild");
        for row in &rows {
            let adjustment: String = row.get("adjustment");
            assert_eq!(adjustment, "raw");
        }
    }

    #[tokio::test]
    async fn test_migrate_v3_widens_uniqueness_key() {
        // The point of the migration: the same bar under two adjustment bases
        // must be storable as two rows rather than one silently overwriting
        // the other.
        let pool = v2_pool_with_candles().await;
        insert_legacy_candle(&pool, "AAPL", 400.0).await;

        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v3 migration");

        sqlx::query(
            r"
            INSERT INTO candles (symbol, timestamp, open, high, low, close, volume, broker, adjustment)
            VALUES ('AAPL', '2024-06-10T13:30:00Z', 100, 100, 100, 100, 1000, 'alpaca', 'all')
            ",
        )
        .execute(&pool)
        .await
        .expect("adjusted bar must not collide with the raw bar");

        let count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM candles WHERE symbol = 'AAPL'")
            .fetch_one(&pool)
            .await
            .expect("count")
            .get("n");
        assert_eq!(count, 2, "both price bases must coexist");
    }

    #[tokio::test]
    async fn test_migrate_v3_rejects_unknown_adjustment_label() {
        let pool = v2_pool_with_candles().await;
        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v3 migration");

        let result = sqlx::query(
            r"
            INSERT INTO candles (symbol, timestamp, open, high, low, close, volume, broker, adjustment)
            VALUES ('AAPL', '2024-06-10T13:30:00Z', 100, 100, 100, 100, 1000, 'alpaca', 'bogus')
            ",
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "CHECK constraint must reject an unrecognised adjustment label"
        );
    }

    #[tokio::test]
    async fn test_migrate_v3_is_idempotent() {
        // `run_migrations` cascades through every migration up to the
        // latest (now v4), so a v2 database converges there, not at v3.
        let pool = v2_pool_with_candles().await;
        insert_legacy_candle(&pool, "AAPL", 400.0).await;

        let manager = MigrationManager::new(pool.clone());
        manager.run_migrations().await.expect("first run");
        manager.run_migrations().await.expect("second run");

        assert_eq!(
            manager.get_schema_version().await.expect("version"),
            4,
            "re-running migrations must not advance or duplicate the version"
        );
    }

    #[tokio::test]
    async fn test_migrate_v4_preserves_existing_candles() {
        // Adding the corporate_actions ledger must not touch the candles
        // table it will eventually be read alongside.
        let pool = v2_pool_with_candles().await;
        insert_legacy_candle(&pool, "AAPL", 400.0).await;
        insert_legacy_candle(&pool, "MSFT", 300.0).await;

        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v2 -> v4 migration");

        let count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM candles")
            .fetch_one(&pool)
            .await
            .expect("count")
            .get("n");
        assert_eq!(count, 2, "no candle rows may be lost by the v4 migration");
    }

    #[tokio::test]
    async fn test_migrate_v4_allows_split_and_dividend_for_same_symbol() {
        // The uniqueness key is (symbol, ex_date, action_type), so a split
        // and a dividend on different dates for the same symbol — the
        // common case — must both be storable without colliding.
        let pool = v2_pool_with_candles().await;
        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v2 -> v4 migration");

        sqlx::query(
            r"
            INSERT INTO corporate_actions (symbol, ex_date, action_type, split_from, split_to, source)
            VALUES ('AAPL', '2020-08-31', 'split', 1, 4, 'alpaca')
            ",
        )
        .execute(&pool)
        .await
        .expect("insert split");

        sqlx::query(
            r"
            INSERT INTO corporate_actions (symbol, ex_date, action_type, cash_amount, source)
            VALUES ('AAPL', '2020-05-08', 'cash_dividend', 0.82, 'alpaca')
            ",
        )
        .execute(&pool)
        .await
        .expect("insert dividend must not collide with the split row");

        let count: i64 =
            sqlx::query("SELECT COUNT(*) AS n FROM corporate_actions WHERE symbol = 'AAPL'")
                .fetch_one(&pool)
                .await
                .expect("count")
                .get("n");
        assert_eq!(count, 2, "both action types must coexist");
    }

    #[tokio::test]
    async fn test_migrate_v4_rejects_unknown_action_type() {
        let pool = v2_pool_with_candles().await;
        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v2 -> v4 migration");

        let result = sqlx::query(
            r"
            INSERT INTO corporate_actions (symbol, ex_date, action_type, cash_amount, source)
            VALUES ('AAPL', '2020-05-08', 'bogus', 0.82, 'alpaca')
            ",
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "CHECK constraint must reject an unrecognised action_type"
        );
    }

    #[tokio::test]
    async fn test_migrate_v4_rejects_split_row_missing_ratio() {
        // A 'split' row with no split_from/split_to is a half-populated,
        // meaningless row — the CHECK constraint must not let it in, the
        // same way an unknown action_type is rejected.
        let pool = v2_pool_with_candles().await;
        MigrationManager::new(pool.clone())
            .run_migrations()
            .await
            .expect("v2 -> v4 migration");

        let result = sqlx::query(
            r"
            INSERT INTO corporate_actions (symbol, ex_date, action_type, source)
            VALUES ('AAPL', '2020-08-31', 'split', 'alpaca')
            ",
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "CHECK constraint must reject a split row with no ratio"
        );
    }

    #[tokio::test]
    async fn test_migrate_v4_is_idempotent() {
        let pool = v2_pool_with_candles().await;
        insert_legacy_candle(&pool, "AAPL", 400.0).await;

        let manager = MigrationManager::new(pool.clone());
        manager.run_migrations().await.expect("first run");
        manager.run_migrations().await.expect("second run");

        assert_eq!(
            manager.get_schema_version().await.expect("version"),
            4,
            "re-running migrations must not advance or duplicate the version"
        );
    }
}
