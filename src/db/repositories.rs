//! Repository implementations for database operations
//!
//! This module provides repository patterns for data access, implementing
//! the domain-driven design principles with clean separation of concerns.

use crate::db::models::*;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Repository for pattern-related database operations
#[derive(Clone, Debug)]
pub struct PatternRepository {
    pool: SqlitePool,
}

impl PatternRepository {
    /// Creates a new pattern repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Stores pattern statistics
    pub async fn store_pattern_stats(
        &self,
        stats: &crate::risk::pattern_stats::PatternStats,
    ) -> Result<(), sqlx::Error> {
        let db_stats = PatternStats::from(stats.clone());

        sqlx::query!(
            r#"
            INSERT INTO pattern_stats (
                pattern_name, total_signals, total_trades, winning_trades, losing_trades,
                total_profit, win_rate, avg_profit_per_trade, consecutive_losses,
                is_enabled, avg_confidence, confidence_weighted_win_rate, last_updated
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pattern_name) DO UPDATE SET
                total_signals = excluded.total_signals,
                total_trades = excluded.total_trades,
                winning_trades = excluded.winning_trades,
                losing_trades = excluded.losing_trades,
                total_profit = excluded.total_profit,
                win_rate = excluded.win_rate,
                avg_profit_per_trade = excluded.avg_profit_per_trade,
                consecutive_losses = excluded.consecutive_losses,
                is_enabled = excluded.is_enabled,
                avg_confidence = excluded.avg_confidence,
                confidence_weighted_win_rate = excluded.confidence_weighted_win_rate,
                last_updated = excluded.last_updated
            "#,
            db_stats.pattern_name,
            db_stats.total_signals,
            db_stats.total_trades,
            db_stats.winning_trades,
            db_stats.losing_trades,
            db_stats.total_profit,
            db_stats.win_rate,
            db_stats.avg_profit_per_trade,
            db_stats.consecutive_losses,
            db_stats.is_enabled,
            db_stats.avg_confidence,
            db_stats.confidence_weighted_win_rate,
            db_stats.last_updated
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves pattern statistics by name
    pub async fn get_pattern_stats(
        &self,
        pattern_name: &str,
    ) -> Result<Option<PatternStats>, sqlx::Error> {
        let stats =
            sqlx::query_as::<_, PatternStats>("SELECT * FROM pattern_stats WHERE pattern_name = ?")
                .bind(pattern_name)
                .fetch_optional(&self.pool)
                .await?;

        Ok(stats)
    }

    /// Retrieves all pattern statistics
    pub async fn get_all_pattern_stats(&self) -> Result<Vec<PatternStats>, sqlx::Error> {
        let stats =
            sqlx::query_as::<_, PatternStats>("SELECT * FROM pattern_stats ORDER BY pattern_name")
                .fetch_all(&self.pool)
                .await?;

        Ok(stats)
    }

    /// Updates pattern enable/disable status
    pub async fn update_pattern_status(
        &self,
        pattern_name: &str,
        is_enabled: bool,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        sqlx::query!(
            "UPDATE pattern_stats SET is_enabled = ?, last_updated = ? WHERE pattern_name = ?",
            is_enabled,
            now,
            pattern_name
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Stores rolling window statistics
    pub async fn store_rolling_stats(
        &self,
        stats: &crate::risk::pattern_stats::RollingStats,
    ) -> Result<(), sqlx::Error> {
        let db_stats = RollingStats {
            id: 0,
            pattern_name: stats.stats.pattern_name.clone(),
            window_days: stats.window_days as i32,
            total_trades: stats.stats.total_trades as i32,
            winning_trades: stats.stats.winning_trades as i32,
            losing_trades: stats.stats.losing_trades as i32,
            total_profit: stats.stats.total_profit.to_f64().unwrap_or(0.0),
            win_rate: stats.stats.win_rate,
            avg_profit_per_trade: stats.stats.avg_profit_per_trade.to_f64().unwrap_or(0.0),
            consecutive_losses: stats.stats.consecutive_losses as i32,
            performance_score: stats.performance_score,
            risk_multiplier: stats.risk_multiplier,
            window_start: stats.last_recalculated - Duration::days(stats.window_days as i64),
            window_end: stats.last_recalculated,
            last_updated: stats.last_recalculated,
        };

        sqlx::query!(
            r#"
            INSERT INTO pattern_rolling_stats (
                pattern_name, window_days, total_trades, winning_trades, losing_trades,
                total_profit, win_rate, avg_profit_per_trade, consecutive_losses,
                performance_score, risk_multiplier, window_start, window_end, last_updated
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pattern_name, window_days, window_end) DO UPDATE SET
                total_trades = excluded.total_trades,
                winning_trades = excluded.winning_trades,
                losing_trades = excluded.losing_trades,
                total_profit = excluded.total_profit,
                win_rate = excluded.win_rate,
                avg_profit_per_trade = excluded.avg_profit_per_trade,
                consecutive_losses = excluded.consecutive_losses,
                performance_score = excluded.performance_score,
                risk_multiplier = excluded.risk_multiplier,
                last_updated = excluded.last_updated
            "#,
            db_stats.pattern_name,
            db_stats.window_days,
            db_stats.total_trades,
            db_stats.winning_trades,
            db_stats.losing_trades,
            db_stats.total_profit,
            db_stats.win_rate,
            db_stats.avg_profit_per_trade,
            db_stats.consecutive_losses,
            db_stats.performance_score,
            db_stats.risk_multiplier,
            db_stats.window_start,
            db_stats.window_end,
            db_stats.last_updated
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets rolling statistics for a pattern and window
    pub async fn get_rolling_stats(
        &self,
        pattern_name: &str,
        window_days: i32,
    ) -> Result<Option<RollingStats>, sqlx::Error> {
        let stats = sqlx::query_as::<_, RollingStats>(
            "SELECT * FROM pattern_rolling_stats WHERE pattern_name = ? AND window_days = ? ORDER BY window_end DESC LIMIT 1"
        )
        .bind(pattern_name)
        .bind(window_days)
        .fetch_optional(&self.pool)
        .await?;

        Ok(stats)
    }
}

/// Repository for signal-related database operations
#[derive(Clone, Debug)]
pub struct SignalRepository {
    pool: SqlitePool,
}

impl SignalRepository {
    /// Creates a new signal repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Stores a trading signal
    pub async fn store_signal(
        &self,
        signal: &crate::patterns::signals::TradingSignal,
    ) -> Result<i64, sqlx::Error> {
        let db_signal = PatternSignal::from(signal.clone());

        let result = sqlx::query!(
            r#"
            INSERT INTO pattern_signals (
                symbol, pattern_name, signal_type, confidence, detected_at,
                candle_timestamp, price_at_detection, acted_upon
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            db_signal.symbol,
            db_signal.pattern_name,
            db_signal.signal_type,
            db_signal.confidence,
            db_signal.detected_at,
            db_signal.candle_timestamp,
            db_signal.price_at_detection,
            db_signal.acted_upon
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Stores a detected pattern signal from its raw event fields
    ///
    /// Unlike [`Self::store_signal`], this does not require constructing a
    /// full `patterns::signals::TradingSignal` (which carries pattern-engine
    /// internals like signal components and market context that a detection
    /// event does not have). Callers that only observe a
    /// `events::types::PatternSignal` — e.g. the risk engine recording
    /// signal detections for statistics — can persist exactly the fields
    /// they actually have, with `candle_timestamp` approximated as
    /// `detected_at` since candle data isn't available at that layer.
    pub async fn store_signal_detection(
        &self,
        symbol: &str,
        pattern_name: &str,
        signal_type: &str,
        confidence: f64,
        detected_at: DateTime<Utc>,
        price_at_detection: f64,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO pattern_signals (
                symbol, pattern_name, signal_type, confidence, detected_at,
                candle_timestamp, price_at_detection, acted_upon
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            symbol,
            pattern_name,
            signal_type,
            confidence,
            detected_at,
            detected_at,
            price_at_detection,
            false,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Updates signal with order information
    pub async fn update_signal_with_order(
        &self,
        signal_id: i64,
        order_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE pattern_signals SET order_id = ?, acted_upon = TRUE WHERE id = ?",
            order_id,
            signal_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Updates signal with trade outcome
    pub async fn update_signal_outcome(
        &self,
        signal_id: i64,
        outcome: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE pattern_signals SET outcome = ? WHERE id = ?",
            outcome,
            signal_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets signals for a symbol within a time range
    pub async fn get_signals_for_symbol(
        &self,
        symbol: &str,
        since: DateTime<Utc>,
    ) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as::<_, PatternSignal>(
            "SELECT * FROM pattern_signals WHERE symbol = ? AND detected_at >= ? ORDER BY detected_at DESC"
        )
        .bind(symbol)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    /// Gets signals for a pattern within a time range
    pub async fn get_signals_for_pattern(
        &self,
        pattern_name: &str,
        since: DateTime<Utc>,
    ) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as::<_, PatternSignal>(
            "SELECT * FROM pattern_signals WHERE pattern_name = ? AND detected_at >= ? ORDER BY detected_at DESC"
        )
        .bind(pattern_name)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    /// Gets recent signals across all patterns
    pub async fn get_recent_signals(&self, limit: i32) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as::<_, PatternSignal>(
            "SELECT * FROM pattern_signals ORDER BY detected_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    // ========================================
    // TradingSignal Methods (Phase 3D-2)
    // ========================================

    /// Saves a trading signal to the database
    pub async fn save_trading_signal(
        &self,
        signal: &crate::patterns::signals::TradingSignal,
    ) -> Result<i64, sqlx::Error> {
        use crate::db::models::TradingSignalRow;

        let row = TradingSignalRow::from(signal);

        sqlx::query!(
            r#"
            INSERT INTO trading_signals (
                id, symbol, signal_type, confidence, price_level, generated_at,
                trend_type, trend_strength, volatility, market_strength,
                components, metadata, acted_upon
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            row.id,
            row.symbol,
            row.signal_type,
            row.confidence,
            row.price_level,
            row.generated_at,
            row.trend_type,
            row.trend_strength,
            row.volatility,
            row.market_strength,
            row.components,
            row.metadata,
            row.acted_upon
        )
        .execute(&self.pool)
        .await?;

        Ok(1) // SQLite doesn't return row ID for TEXT primary keys
    }

    /// Saves a PatternSignal (simple pattern detection) to the database
    pub async fn save_pattern_signal(
        &self,
        signal: &crate::events::types::PatternSignal,
    ) -> Result<i64, sqlx::Error> {
        let symbol_str = signal.symbol.as_str();
        let pattern_str = signal.pattern_name.as_str();
        let signal_type_str = signal.signal_type.as_str();
        let confidence_val = signal.confidence.value();
        let price_val = signal.price.value().to_f64().unwrap_or(0.0);
        let result = sqlx::query!(
            r#"
            INSERT INTO pattern_signals (
                symbol, pattern_name, signal_type, confidence, detected_at,
                candle_timestamp, price_at_detection, acted_upon
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            symbol_str,
            pattern_str,
            signal_type_str,
            confidence_val,
            signal.detected_at,
            signal.detected_at,
            price_val,
            false
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Gets trading signals for a symbol with optional limit
    pub async fn get_trading_signals_for_symbol(
        &self,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<crate::patterns::signals::TradingSignal>, sqlx::Error> {
        use crate::db::models::TradingSignalRow;

        let rows = sqlx::query_as::<_, TradingSignalRow>(
            "SELECT * FROM trading_signals WHERE symbol = ? ORDER BY generated_at DESC LIMIT ?",
        )
        .bind(symbol)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        let signals = rows
            .into_iter()
            .filter_map(|row| row.to_trading_signal().ok())
            .collect();

        Ok(signals)
    }

    /// Updates the outcome of a trading signal
    pub async fn update_trading_signal_outcome(
        &self,
        signal_id: &str,
        outcome: &str,
        order_id: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE trading_signals SET outcome = ?, acted_upon = TRUE, order_id = ? WHERE id = ?",
            outcome,
            order_id,
            signal_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets signal statistics for a date range
    pub async fn get_signal_statistics(
        &self,
        from_date: DateTime<Utc>,
    ) -> Result<crate::db::models::SignalStatistics, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) as "total_signals!: i32",
                COALESCE(SUM(CASE WHEN signal_type = 'buy' THEN 1 ELSE 0 END), 0) as "buy_signals!: i32",
                COALESCE(SUM(CASE WHEN signal_type = 'sell' THEN 1 ELSE 0 END), 0) as "sell_signals!: i32",
                COALESCE(AVG(confidence), 0.0) as "avg_confidence!: f64",
                COALESCE(SUM(CASE WHEN confidence >= 0.8 THEN 1 ELSE 0 END), 0) as "high_confidence_signals!: i32",
                COALESCE(SUM(CASE WHEN acted_upon = TRUE THEN 1 ELSE 0 END), 0) as "signals_acted_upon!: i32",
                COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as "win_count!: i32",
                COALESCE(SUM(CASE WHEN outcome = 'loss' THEN 1 ELSE 0 END), 0) as "loss_count!: i32",
                COALESCE(SUM(CASE WHEN outcome = 'pending' THEN 1 ELSE 0 END), 0) as "pending_count!: i32"
            FROM trading_signals
            WHERE generated_at >= ?
            "#,
            from_date
        )
        .fetch_one(&self.pool)
        .await?;

        let total_decided = row.win_count + row.loss_count;

        let win_rate = if total_decided > 0 {
            row.win_count as f64 / total_decided as f64
        } else {
            0.0
        };

        Ok(crate::db::models::SignalStatistics {
            total_signals: row.total_signals,
            buy_signals: row.buy_signals,
            sell_signals: row.sell_signals,
            avg_confidence: row.avg_confidence,
            high_confidence_signals: row.high_confidence_signals,
            signals_acted_upon: row.signals_acted_upon,
            win_count: row.win_count,
            loss_count: row.loss_count,
            pending_count: row.pending_count,
            win_rate,
        })
    }

    /// Batch inserts trading signals for improved performance
    pub async fn batch_save_trading_signals(
        &self,
        signals: &[crate::patterns::signals::TradingSignal],
    ) -> Result<Vec<String>, sqlx::Error> {
        use crate::db::models::TradingSignalRow;

        let mut inserted_ids = Vec::new();

        // SQLite doesn't support batch inserts in the same way as other databases
        // We'll use a transaction for better performance
        let mut tx = self.pool.begin().await?;

        for signal in signals {
            let row = TradingSignalRow::from(signal);

            sqlx::query!(
                r#"
                INSERT INTO trading_signals (
                    id, symbol, signal_type, confidence, price_level, generated_at,
                    trend_type, trend_strength, volatility, market_strength,
                    components, metadata, acted_upon
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                row.id,
                row.symbol,
                row.signal_type,
                row.confidence,
                row.price_level,
                row.generated_at,
                row.trend_type,
                row.trend_strength,
                row.volatility,
                row.market_strength,
                row.components,
                row.metadata,
                row.acted_upon
            )
            .execute(&mut *tx)
            .await?;

            inserted_ids.push(row.id.clone());
        }

        tx.commit().await?;

        Ok(inserted_ids)
    }

    /// Stores an indicator value
    pub async fn save_indicator_value(
        &self,
        symbol: &str,
        indicator_name: &str,
        value: f64,
        timestamp: DateTime<Utc>,
        metadata: Option<String>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO indicator_values (symbol, indicator_name, value, timestamp, metadata)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(symbol, indicator_name, timestamp) DO UPDATE SET
                value = excluded.value,
                metadata = excluded.metadata
            "#,
            symbol,
            indicator_name,
            value,
            timestamp,
            metadata
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Gets indicator values for a symbol and indicator
    pub async fn get_indicator_values(
        &self,
        symbol: &str,
        indicator_name: &str,
        since: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<crate::db::models::IndicatorValueRow>, sqlx::Error> {
        use crate::db::models::IndicatorValueRow;

        let rows = sqlx::query_as::<_, IndicatorValueRow>(
            r#"
            SELECT * FROM indicator_values
            WHERE symbol = ? AND indicator_name = ? AND timestamp >= ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(symbol)
        .bind(indicator_name)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

/// Repository for performance analytics database operations
#[derive(Clone, Debug)]
pub struct PerformanceRepository {
    pool: SqlitePool,
}

impl PerformanceRepository {
    /// Creates a new performance repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Stores trade history for pattern analysis
    pub async fn store_trade_record(
        &self,
        record: &crate::risk::pattern_stats::TradeRecord,
    ) -> Result<i64, sqlx::Error> {
        let db_record = TradeRecord::from(record.clone());

        let result = sqlx::query!(
            r#"
            INSERT INTO pattern_trade_history (
                pattern_name, trade_timestamp, confidence, profit, was_profitable,
                order_id, symbol, quantity, price, commission, notes, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            db_record.pattern_name,
            db_record.trade_timestamp,
            db_record.confidence,
            db_record.profit,
            db_record.was_profitable,
            db_record.order_id,
            db_record.symbol,
            db_record.quantity,
            db_record.price,
            db_record.commission,
            db_record.notes,
            db_record.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Stores risk adjustment audit entry
    pub async fn store_risk_adjustment(
        &self,
        audit: &RiskAdjustmentAudit,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO risk_adjustment_audit (
                pattern_name, adjustment_type, old_risk_multiplier, new_risk_multiplier,
                performance_score, trigger_reason, window_days, trade_count, win_rate, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            audit.pattern_name,
            audit.adjustment_type,
            audit.old_risk_multiplier,
            audit.new_risk_multiplier,
            audit.performance_score,
            audit.trigger_reason,
            audit.window_days,
            audit.trade_count,
            audit.win_rate,
            audit.timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Stores risk event
    pub async fn store_risk_event(
        &self,
        event: &crate::events::types::RiskViolation,
    ) -> Result<i64, sqlx::Error> {
        let db_event = RiskEvent::from(event.clone());

        let result = sqlx::query!(
            r#"
            INSERT INTO risk_events (
                event_type, timestamp, account_id, pattern_name, violation_type,
                severity, message, details, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            db_event.event_type,
            db_event.timestamp,
            db_event.account_id,
            db_event.pattern_name,
            db_event.violation_type,
            db_event.severity,
            db_event.message,
            db_event.details,
            db_event.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Gets trade history for a pattern within date range
    pub async fn get_trade_history(
        &self,
        pattern_name: &str,
        since: DateTime<Utc>,
    ) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, TradeRecord>(
            "SELECT * FROM pattern_trade_history WHERE pattern_name = ? AND trade_timestamp >= ? ORDER BY trade_timestamp DESC"
        )
        .bind(pattern_name)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Gets risk adjustment history for a pattern
    pub async fn get_risk_adjustments(
        &self,
        pattern_name: &str,
        limit: i32,
    ) -> Result<Vec<RiskAdjustmentAudit>, sqlx::Error> {
        let adjustments = sqlx::query_as::<_, RiskAdjustmentAudit>(
            "SELECT * FROM risk_adjustment_audit WHERE pattern_name = ? ORDER BY timestamp DESC LIMIT ?"
        )
        .bind(pattern_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(adjustments)
    }

    /// Gets recent risk events
    pub async fn get_recent_risk_events(&self, limit: i32) -> Result<Vec<RiskEvent>, sqlx::Error> {
        let events = sqlx::query_as::<_, RiskEvent>(
            "SELECT * FROM risk_events ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Gets the most recent risk violation events for an account, newest first.
    ///
    /// Filters `risk_events` to rows with `event_type = 'RiskViolation'` for the
    /// given `account_id`. Used to power the risk dashboard's violation feed.
    pub async fn get_recent_risk_violations(
        &self,
        account_id: i64,
        limit: i32,
    ) -> Result<Vec<RiskEvent>, sqlx::Error> {
        let events = sqlx::query_as::<_, RiskEvent>(
            "SELECT * FROM risk_events \
             WHERE account_id = ? AND event_type = 'RiskViolation' \
             ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(account_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Gets performance summary for all patterns
    pub async fn get_performance_summary(&self) -> Result<SystemPerformance, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(DISTINCT ps.pattern_name) as "total_patterns!: i32",
                COUNT(DISTINCT CASE WHEN ps.is_enabled THEN ps.pattern_name END) as "enabled_patterns!: i32",
                COALESCE(SUM(ps.total_trades), 0) as "total_trades!: i32",
                COALESCE(AVG(ps.win_rate), 0.0) as "avg_win_rate!: f64",
                COALESCE(SUM(ps.total_profit), 0.0) as "total_profit!: f64"
            FROM pattern_stats ps
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(SystemPerformance {
            total_patterns: row.total_patterns,
            enabled_patterns: row.enabled_patterns,
            total_trades: row.total_trades,
            system_win_rate: row.avg_win_rate,
            total_profit: row.total_profit,
        })
    }

    /// Cleans up old data based on retention policies
    pub async fn cleanup_old_data(&self, retention_days: i32) -> Result<(), sqlx::Error> {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        // Clean up old trade history
        sqlx::query!(
            "DELETE FROM pattern_trade_history WHERE created_at < ?",
            cutoff
        )
        .execute(&self.pool)
        .await?;

        // Clean up old risk events
        sqlx::query!("DELETE FROM risk_events WHERE created_at < ?", cutoff)
            .execute(&self.pool)
            .await?;

        // Clean up old rolling stats (keep only recent windows)
        let window_cutoff = Utc::now() - Duration::days(90);
        sqlx::query!(
            "DELETE FROM pattern_rolling_stats WHERE window_end < ?",
            window_cutoff
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ============================================================================
// Order Repository (Phase 4A)
// ============================================================================

/// Repository for order-related database operations
#[derive(Clone, Debug)]
pub struct OrderRepository {
    pool: SqlitePool,
}

impl OrderRepository {
    /// Creates a new order repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Saves a new order to the database
    pub async fn save_order(
        &self,
        order: &crate::orders::types::ManagedOrder,
        account_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let symbol_str = order.symbol.as_str().to_string();
        let side_str = format!("{:?}", order.side).to_lowercase();
        let order_type_str = format!("{:?}", order.order_type).to_lowercase();
        let quantity_val = order.quantity.value();
        let limit_price_val = order.limit_price.map(|p| p.as_f64());
        let stop_price_val = order.stop_price.map(|p| p.as_f64());
        let trailing_val = order.trailing_amount.and_then(|d| d.to_f64());
        let status_str = order_status_to_db(&order.status);
        let tif_str = time_in_force_to_db(&order.time_in_force);
        let filled_avg_val = order.filled_avg_price.map(|p| p.as_f64());
        let commission_val = order.commission.and_then(|c| c.to_f64());
        let pattern_name_str = order.pattern_name.as_ref().map(|p| p.as_str().to_string());
        let result = sqlx::query!(
            r#"
            INSERT INTO orders (
                account_id, broker_order_id, symbol, side, order_type, quantity,
                filled_quantity, limit_price, stop_price, trailing_percent, status,
                time_in_force, submitted_at, filled_at, filled_avg_price, commission,
                error_message, is_pattern_trade, pattern_name
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            account_id,
            order.broker_order_id,
            symbol_str,
            side_str,
            order_type_str,
            quantity_val,
            order.filled_quantity,
            limit_price_val,
            stop_price_val,
            trailing_val,
            status_str,
            tif_str,
            order.submitted_at,
            order.filled_at,
            filled_avg_val,
            commission_val,
            order.error_message,
            order.is_pattern_trade,
            pattern_name_str,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Updates an existing order
    pub async fn update_order(
        &self,
        order: &crate::orders::types::ManagedOrder,
    ) -> Result<(), sqlx::Error> {
        let status_str = order_status_to_db(&order.status);
        let filled_avg_val = order.filled_avg_price.map(|p| p.as_f64());
        let commission_val = order.commission.and_then(|c| c.to_f64());
        sqlx::query!(
            r#"
            UPDATE orders SET
                broker_order_id = ?,
                filled_quantity = ?,
                status = ?,
                submitted_at = ?,
                filled_at = ?,
                filled_avg_price = ?,
                commission = ?,
                error_message = ?
            WHERE broker_order_id = ? OR id = (
                SELECT id FROM orders WHERE broker_order_id IS NULL
                ORDER BY id DESC LIMIT 1
            )
            "#,
            order.broker_order_id,
            order.filled_quantity,
            status_str,
            order.submitted_at,
            order.filled_at,
            filled_avg_val,
            commission_val,
            order.error_message,
            order.broker_order_id,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets an order by broker order ID
    pub async fn get_order_by_broker_id(
        &self,
        broker_order_id: &str,
    ) -> Result<Option<OrderRow>, sqlx::Error> {
        let order = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE broker_order_id = ?")
            .bind(broker_order_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(order)
    }

    /// Gets orders for a specific symbol
    pub async fn get_orders_for_symbol(
        &self,
        symbol: &str,
        account_id: i64,
        limit: i32,
    ) -> Result<Vec<OrderRow>, sqlx::Error> {
        let orders = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders WHERE account_id = ? AND symbol = ? ORDER BY submitted_at DESC LIMIT ?"
        )
        .bind(account_id)
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Gets all open orders for an account
    pub async fn get_open_orders(&self, account_id: i64) -> Result<Vec<OrderRow>, sqlx::Error> {
        let orders = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders WHERE account_id = ? AND status IN ('pending', 'partial') ORDER BY submitted_at DESC"
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Gets order history for an account within a time range
    pub async fn get_order_history(
        &self,
        account_id: i64,
        since: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<OrderRow>, sqlx::Error> {
        let orders = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders WHERE account_id = ? AND submitted_at >= ? ORDER BY submitted_at DESC LIMIT ?"
        )
        .bind(account_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Gets orders for a specific pattern
    pub async fn get_pattern_orders(
        &self,
        pattern_name: &str,
        account_id: i64,
        limit: i32,
    ) -> Result<Vec<OrderRow>, sqlx::Error> {
        let orders = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders WHERE account_id = ? AND pattern_name = ? ORDER BY submitted_at DESC LIMIT ?"
        )
        .bind(account_id)
        .bind(pattern_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Counts pattern trades for today
    pub async fn count_pattern_trades_today(
        &self,
        pattern_name: &str,
        account_id: i64,
    ) -> Result<i32, sqlx::Error> {
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();

        let result = sqlx::query!(
            "SELECT COUNT(*) as count FROM orders WHERE account_id = ? AND pattern_name = ? AND submitted_at >= ?",
            account_id,
            pattern_name,
            today_start
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(result.count as i32)
    }
}

/// Database row for orders table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRow {
    pub id: i64,
    pub account_id: i64,
    pub broker_order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: i64,
    pub filled_quantity: Option<i64>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub trailing_percent: Option<f64>,
    pub status: String,
    pub time_in_force: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub filled_avg_price: Option<f64>,
    pub commission: Option<f64>,
    pub error_message: Option<String>,
    pub is_pattern_trade: Option<bool>,
    pub pattern_name: Option<String>,
}

// Helper functions for converting between domain types and database strings
fn order_status_to_db(status: &crate::brokers::traits::OrderStatus) -> &'static str {
    match status {
        crate::brokers::traits::OrderStatus::Pending => "pending",
        crate::brokers::traits::OrderStatus::Accepted => "pending",
        crate::brokers::traits::OrderStatus::PartiallyFilled => "partial",
        crate::brokers::traits::OrderStatus::Filled => "filled",
        crate::brokers::traits::OrderStatus::Cancelled => "cancelled",
        crate::brokers::traits::OrderStatus::Rejected => "rejected",
        crate::brokers::traits::OrderStatus::Expired => "cancelled",
    }
}

// ============================================================================
// Position Repository (Phase 4B)
// ============================================================================

/// Repository for position-related database operations
#[derive(Clone, Debug)]
pub struct PositionRepository {
    pool: SqlitePool,
}

impl PositionRepository {
    /// Creates a new position repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Saves or updates a position in the database
    ///
    /// This method performs an upsert operation - if a position exists for the
    /// given account and symbol, it will be updated; otherwise, a new record is inserted.
    ///
    /// # Arguments
    /// * `position` - The position to save
    /// * `account_id` - The account ID associated with this position
    ///
    /// # Returns
    /// The database row ID of the saved position
    pub async fn save_position(
        &self,
        position: &crate::positions::tracker::Position,
        account_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let side = match position.side {
            crate::positions::tracker::PositionSide::Long => "long",
            crate::positions::tracker::PositionSide::Short => "short",
        };

        // Serialize order IDs as JSON
        let entry_orders =
            serde_json::to_string(&position.entry_orders).unwrap_or_else(|_| "[]".to_string());
        let exit_orders =
            serde_json::to_string(&position.exit_orders).unwrap_or_else(|_| "[]".to_string());

        let symbol_str = position.symbol.as_str().to_string();
        let quantity_val = position.quantity.value();
        let avg_entry = position.avg_entry_price.as_f64();
        let current = position.current_price.as_f64();
        let market_val = position.market_value.to_f64().unwrap_or(0.0);
        let cost_basis_val = position.cost_basis.to_f64().unwrap_or(0.0);
        let unrealized = position.unrealized_pl.to_f64().unwrap_or(0.0);
        let unrealized_pct = position.unrealized_pl_percent.to_f64().unwrap_or(0.0);
        let realized = position.realized_pl.to_f64().unwrap_or(0.0);
        let pattern_name_str = position
            .pattern_name
            .as_ref()
            .map(|p| p.as_str().to_string());
        let signal_conf = position.signal_confidence.and_then(|c| c.to_f64());
        let result = sqlx::query!(
            r#"
            INSERT INTO positions (
                account_id, symbol, quantity, side, avg_entry_price, current_price,
                market_value, cost_basis, unrealized_pl, unrealized_pl_percent, realized_pl,
                opened_at, updated_at, pattern_name, signal_confidence, entry_orders, exit_orders
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(account_id, symbol) DO UPDATE SET
                quantity = excluded.quantity,
                side = excluded.side,
                avg_entry_price = excluded.avg_entry_price,
                current_price = excluded.current_price,
                market_value = excluded.market_value,
                cost_basis = excluded.cost_basis,
                unrealized_pl = excluded.unrealized_pl,
                unrealized_pl_percent = excluded.unrealized_pl_percent,
                realized_pl = excluded.realized_pl,
                updated_at = excluded.updated_at,
                pattern_name = excluded.pattern_name,
                signal_confidence = excluded.signal_confidence,
                entry_orders = excluded.entry_orders,
                exit_orders = excluded.exit_orders
            "#,
            account_id,
            symbol_str,
            quantity_val,
            side,
            avg_entry,
            current,
            market_val,
            cost_basis_val,
            unrealized,
            unrealized_pct,
            realized,
            position.opened_at,
            position.updated_at,
            pattern_name_str,
            signal_conf,
            entry_orders,
            exit_orders,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Gets a position by symbol for a specific account
    ///
    /// # Arguments
    /// * `symbol` - The symbol to look up
    /// * `account_id` - The account ID
    ///
    /// # Returns
    /// The position if found, or None
    pub async fn get_position(
        &self,
        symbol: &str,
        account_id: i64,
    ) -> Result<Option<PositionRow>, sqlx::Error> {
        let position = sqlx::query_as::<_, PositionRow>(
            "SELECT * FROM positions WHERE account_id = ? AND symbol = ?",
        )
        .bind(account_id)
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(position)
    }

    /// Gets all positions for a specific symbol across all accounts
    ///
    /// Useful for system-wide analysis or multi-account management.
    ///
    /// # Arguments
    /// * `symbol` - The symbol to look up
    ///
    /// # Returns
    /// Vector of all positions for the symbol
    pub async fn get_positions_by_symbol(
        &self,
        symbol: &str,
    ) -> Result<Vec<PositionRow>, sqlx::Error> {
        let positions = sqlx::query_as::<_, PositionRow>(
            "SELECT * FROM positions WHERE symbol = ? ORDER BY updated_at DESC",
        )
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;

        Ok(positions)
    }

    /// Gets all active (open) positions for an account
    ///
    /// # Arguments
    /// * `account_id` - The account ID
    ///
    /// # Returns
    /// Vector of all active positions
    pub async fn get_active_positions(
        &self,
        account_id: i64,
    ) -> Result<Vec<PositionRow>, sqlx::Error> {
        let positions = sqlx::query_as::<_, PositionRow>(
            "SELECT * FROM positions WHERE account_id = ? ORDER BY updated_at DESC",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(positions)
    }

    /// Updates an existing position
    ///
    /// This is similar to [`save_position`](PositionRepository::save_position) but requires
    /// the position to already exist.
    ///
    /// # Arguments
    /// * `position` - The position with updated values
    /// * `account_id` - The account ID
    ///
    /// # Returns
    /// Number of rows affected (should be 1)
    pub async fn update_position(
        &self,
        position: &crate::positions::tracker::Position,
        account_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let side = match position.side {
            crate::positions::tracker::PositionSide::Long => "long",
            crate::positions::tracker::PositionSide::Short => "short",
        };

        let entry_orders =
            serde_json::to_string(&position.entry_orders).unwrap_or_else(|_| "[]".to_string());
        let exit_orders =
            serde_json::to_string(&position.exit_orders).unwrap_or_else(|_| "[]".to_string());

        let quantity_val = position.quantity.value();
        let avg_entry = position.avg_entry_price.as_f64();
        let current = position.current_price.as_f64();
        let market_val = position.market_value.to_f64().unwrap_or(0.0);
        let cost_basis_val = position.cost_basis.to_f64().unwrap_or(0.0);
        let unrealized = position.unrealized_pl.to_f64().unwrap_or(0.0);
        let unrealized_pct = position.unrealized_pl_percent.to_f64().unwrap_or(0.0);
        let realized = position.realized_pl.to_f64().unwrap_or(0.0);
        let pattern_name_str = position
            .pattern_name
            .as_ref()
            .map(|p| p.as_str().to_string());
        let signal_conf = position.signal_confidence.and_then(|c| c.to_f64());
        let symbol_str = position.symbol.as_str().to_string();
        let result = sqlx::query!(
            r#"
            UPDATE positions SET
                quantity = ?,
                side = ?,
                avg_entry_price = ?,
                current_price = ?,
                market_value = ?,
                cost_basis = ?,
                unrealized_pl = ?,
                unrealized_pl_percent = ?,
                realized_pl = ?,
                updated_at = ?,
                pattern_name = ?,
                signal_confidence = ?,
                entry_orders = ?,
                exit_orders = ?
            WHERE account_id = ? AND symbol = ?
            "#,
            quantity_val,
            side,
            avg_entry,
            current,
            market_val,
            cost_basis_val,
            unrealized,
            unrealized_pct,
            realized,
            position.updated_at,
            pattern_name_str,
            signal_conf,
            entry_orders,
            exit_orders,
            account_id,
            symbol_str,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Closes a position by moving it to position_history and removing from active positions
    ///
    /// This is a two-step operation:
    /// 1. Archive the position to the `position_history` table
    /// 2. Delete from the `positions` table
    ///
    /// # Arguments
    /// * `position` - The position to close
    /// * `account_id` - The account ID
    /// * `close_reason` - Reason for closing the position
    /// * `exit_price` - Final exit price
    ///
    /// # Returns
    /// The history record ID
    pub async fn close_position(
        &self,
        position: &crate::positions::tracker::Position,
        account_id: i64,
        close_reason: &crate::positions::tracker::CloseReason,
        exit_price: crate::brokers::types::Price,
    ) -> Result<i64, sqlx::Error> {
        let side = match position.side {
            crate::positions::tracker::PositionSide::Long => "long",
            crate::positions::tracker::PositionSide::Short => "short",
        };

        let close_reason_str = match close_reason {
            crate::positions::tracker::CloseReason::Manual => "manual",
            crate::positions::tracker::CloseReason::StopLoss => "stop_loss",
            crate::positions::tracker::CloseReason::TakeProfit => "take_profit",
            crate::positions::tracker::CloseReason::RiskViolation => "risk_violation",
            crate::positions::tracker::CloseReason::PatternExit => "pattern_exit",
            crate::positions::tracker::CloseReason::EndOfDay => "end_of_day",
        };

        let entry_orders =
            serde_json::to_string(&position.entry_orders).unwrap_or_else(|_| "[]".to_string());
        let exit_orders =
            serde_json::to_string(&position.exit_orders).unwrap_or_else(|_| "[]".to_string());

        let closed_at = chrono::Utc::now();
        let holding_period_minutes = (closed_at - position.opened_at).num_minutes() as i64;
        let total_pl = position.realized_pl;

        // Start a transaction to ensure both operations succeed or fail together
        let mut tx = self.pool.begin().await?;

        // Pre-compute values for position_history insert
        let symbol_str = position.symbol.as_str().to_string();
        let quantity_val = position.quantity.value();
        let avg_entry = position.avg_entry_price.as_f64();
        let exit_price_val = exit_price.as_f64();
        let cost_basis_val = position.cost_basis.to_f64().unwrap_or(0.0);
        let unrealized = position.unrealized_pl.to_f64().unwrap_or(0.0);
        let realized = position.realized_pl.to_f64().unwrap_or(0.0);
        let total_pl_val = total_pl.to_f64().unwrap_or(0.0);
        let pattern_name_str = position
            .pattern_name
            .as_ref()
            .map(|p| p.as_str().to_string());
        let signal_conf = position.signal_confidence.and_then(|c| c.to_f64());

        // Insert into position_history
        let result = sqlx::query!(
            r#"
            INSERT INTO position_history (
                account_id, symbol, quantity, side, avg_entry_price, exit_price,
                cost_basis, unrealized_pl, realized_pl, total_pl, opened_at, closed_at,
                holding_period_minutes, close_reason, pattern_name, signal_confidence,
                entry_orders, exit_orders
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            account_id,
            symbol_str,
            quantity_val,
            side,
            avg_entry,
            exit_price_val,
            cost_basis_val,
            unrealized,
            realized,
            total_pl_val,
            position.opened_at,
            closed_at,
            holding_period_minutes,
            close_reason_str,
            pattern_name_str,
            signal_conf,
            entry_orders,
            exit_orders,
        )
        .execute(&mut *tx)
        .await?;

        let history_id = result.last_insert_rowid();

        // Delete from active positions
        sqlx::query!(
            "DELETE FROM positions WHERE account_id = ? AND symbol = ?",
            account_id,
            symbol_str
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(history_id)
    }

    /// Gets position history for an account
    ///
    /// # Arguments
    /// * `account_id` - The account ID
    /// * `since` - Only return positions closed after this time
    /// * `limit` - Maximum number of records to return
    ///
    /// # Returns
    /// Vector of closed positions
    pub async fn get_position_history(
        &self,
        account_id: i64,
        since: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<PositionHistoryRow>, sqlx::Error> {
        let history = sqlx::query_as::<_, PositionHistoryRow>(
            "SELECT * FROM position_history WHERE account_id = ? AND closed_at >= ? ORDER BY closed_at DESC LIMIT ?"
        )
        .bind(account_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Gets position history for a specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The symbol to look up
    /// * `account_id` - The account ID
    /// * `limit` - Maximum number of records to return
    ///
    /// # Returns
    /// Vector of closed positions for the symbol
    pub async fn get_position_history_by_symbol(
        &self,
        symbol: &str,
        account_id: i64,
        limit: i32,
    ) -> Result<Vec<PositionHistoryRow>, sqlx::Error> {
        let history = sqlx::query_as::<_, PositionHistoryRow>(
            "SELECT * FROM position_history WHERE account_id = ? AND symbol = ? ORDER BY closed_at DESC LIMIT ?"
        )
        .bind(account_id)
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Gets profitable closed positions count and total P&L
    ///
    /// Useful for performance analytics.
    ///
    /// # Arguments
    /// * `account_id` - The account ID
    /// * `since` - Only count positions closed after this time
    ///
    /// # Returns
    /// Tuple of (winning_count, losing_count, total_pl, avg_pl)
    pub async fn get_position_performance(
        &self,
        account_id: i64,
        since: DateTime<Utc>,
    ) -> Result<(i32, i32, f64, f64), sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(CASE WHEN realized_pl > 0 THEN 1 END) as "winning_count!: i32",
                COUNT(CASE WHEN realized_pl <= 0 THEN 1 END) as "losing_count!: i32",
                COALESCE(SUM(realized_pl), 0.0) as "total_pl!: f64",
                COALESCE(AVG(realized_pl), 0.0) as "avg_pl!: f64"
            FROM position_history
            WHERE account_id = ? AND closed_at >= ?
            "#,
            account_id,
            since
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((
            row.winning_count,
            row.losing_count,
            row.total_pl,
            row.avg_pl,
        ))
    }

    /// Gets positions by pattern name
    ///
    /// Returns both active and historical positions for a specific pattern.
    ///
    /// # Arguments
    /// * `pattern_name` - The pattern name to filter by
    /// * `account_id` - The account ID
    ///
    /// # Returns
    /// Tuple of (active_positions, historical_positions)
    pub async fn get_positions_by_pattern(
        &self,
        pattern_name: &str,
        account_id: i64,
    ) -> Result<(Vec<PositionRow>, Vec<PositionHistoryRow>), sqlx::Error> {
        let active = sqlx::query_as::<_, PositionRow>(
            "SELECT * FROM positions WHERE account_id = ? AND pattern_name = ? ORDER BY updated_at DESC"
        )
        .bind(account_id)
        .bind(pattern_name)
        .fetch_all(&self.pool)
        .await?;

        let historical = sqlx::query_as::<_, PositionHistoryRow>(
            "SELECT * FROM position_history WHERE account_id = ? AND pattern_name = ? ORDER BY closed_at DESC LIMIT 100"
        )
        .bind(account_id)
        .bind(pattern_name)
        .fetch_all(&self.pool)
        .await?;

        Ok((active, historical))
    }

    /// Deletes a position without archiving
    ///
    /// **Warning**: This permanently deletes the position without creating a history record.
    /// Use [`close_position`](PositionRepository::close_position) instead for normal position closure.
    ///
    /// # Arguments
    /// * `symbol` - The symbol to delete
    /// * `account_id` - The account ID
    ///
    /// # Returns
    /// Number of rows affected
    pub async fn delete_position(&self, symbol: &str, account_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM positions WHERE account_id = ? AND symbol = ?",
            account_id,
            symbol
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

/// Database row for positions table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PositionRow {
    pub id: i64,
    pub account_id: i64,
    pub symbol: String,
    pub quantity: i64,
    pub side: String,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub market_value: f64,
    pub cost_basis: f64,
    pub unrealized_pl: f64,
    pub unrealized_pl_percent: f64,
    pub realized_pl: f64,
    pub opened_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub pattern_name: Option<String>,
    pub signal_confidence: Option<f64>,
    pub entry_orders: Option<String>, // JSON
    pub exit_orders: Option<String>,  // JSON
}

/// Database row for position_history table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PositionHistoryRow {
    pub id: i64,
    pub account_id: i64,
    pub symbol: String,
    pub quantity: i64,
    pub side: String,
    pub avg_entry_price: f64,
    pub exit_price: f64,
    pub cost_basis: f64,
    pub unrealized_pl: Option<f64>,
    pub realized_pl: f64,
    pub total_pl: f64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub holding_period_minutes: Option<i64>,
    pub close_reason: Option<String>,
    pub pattern_name: Option<String>,
    pub signal_confidence: Option<f64>,
    pub entry_orders: Option<String>, // JSON
    pub exit_orders: Option<String>,  // JSON
    pub notes: Option<String>,
}

fn time_in_force_to_db(tif: &crate::brokers::traits::TimeInForce) -> &'static str {
    match tif {
        crate::brokers::traits::TimeInForce::Day => "day",
        crate::brokers::traits::TimeInForce::GTC => "gtc",
        crate::brokers::traits::TimeInForce::IOC => "ioc",
        crate::brokers::traits::TimeInForce::FOK => "fok",
    }
}
