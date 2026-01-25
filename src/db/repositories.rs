//! Repository implementations for database operations
//!
//! This module provides repository patterns for data access, implementing
//! the domain-driven design principles with clean separation of concerns.

use crate::db::models::*;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use rust_decimal::prelude::ToPrimitive;

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
    pub async fn store_pattern_stats(&self, stats: &crate::risk::pattern_stats::PatternStats) -> Result<(), sqlx::Error> {
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
    pub async fn get_pattern_stats(&self, pattern_name: &str) -> Result<Option<PatternStats>, sqlx::Error> {
        let stats = sqlx::query_as!(
            PatternStats,
            "SELECT * FROM pattern_stats WHERE pattern_name = ?",
            pattern_name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Retrieves all pattern statistics
    pub async fn get_all_pattern_stats(&self) -> Result<Vec<PatternStats>, sqlx::Error> {
        let stats = sqlx::query_as!(PatternStats, "SELECT * FROM pattern_stats ORDER BY pattern_name")
            .fetch_all(&self.pool)
            .await?;

        Ok(stats)
    }

    /// Updates pattern enable/disable status
    pub async fn update_pattern_status(&self, pattern_name: &str, is_enabled: bool) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE pattern_stats SET is_enabled = ?, last_updated = ? WHERE pattern_name = ?",
            is_enabled,
            Utc::now(),
            pattern_name
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Stores rolling window statistics
    pub async fn store_rolling_stats(&self, stats: &crate::risk::pattern_stats::RollingStats) -> Result<(), sqlx::Error> {
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
    pub async fn get_rolling_stats(&self, pattern_name: &str, window_days: i32) -> Result<Option<RollingStats>, sqlx::Error> {
        let stats = sqlx::query_as!(
            RollingStats,
            "SELECT * FROM pattern_rolling_stats WHERE pattern_name = ? AND window_days = ? ORDER BY window_end DESC LIMIT 1",
            pattern_name,
            window_days
        )
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
    pub async fn store_signal(&self, signal: &crate::patterns::signals::TradingSignal) -> Result<i64, sqlx::Error> {
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

    /// Updates signal with order information
    pub async fn update_signal_with_order(&self, signal_id: i64, order_id: i64) -> Result<(), sqlx::Error> {
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
    pub async fn update_signal_outcome(&self, signal_id: i64, outcome: &str) -> Result<(), sqlx::Error> {
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
    pub async fn get_signals_for_symbol(&self, symbol: &str, since: DateTime<Utc>) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as!(
            PatternSignal,
            "SELECT * FROM pattern_signals WHERE symbol = ? AND detected_at >= ? ORDER BY detected_at DESC",
            symbol,
            since
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    /// Gets signals for a pattern within a time range
    pub async fn get_signals_for_pattern(&self, pattern_name: &str, since: DateTime<Utc>) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as!(
            PatternSignal,
            "SELECT * FROM pattern_signals WHERE pattern_name = ? AND detected_at >= ? ORDER BY detected_at DESC",
            pattern_name,
            since
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    /// Gets recent signals across all patterns
    pub async fn get_recent_signals(&self, limit: i32) -> Result<Vec<PatternSignal>, sqlx::Error> {
        let signals = sqlx::query_as!(
            PatternSignal,
            "SELECT * FROM pattern_signals ORDER BY detected_at DESC LIMIT ?",
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    // ========================================
    // TradingSignal Methods (Phase 3D-2)
    // ========================================

    /// Saves a trading signal to the database
    pub async fn save_trading_signal(&self, signal: &crate::patterns::signals::TradingSignal) -> Result<i64, sqlx::Error> {
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
    pub async fn save_pattern_signal(&self, signal: &crate::events::types::PatternSignal) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO pattern_signals (
                symbol, pattern_name, signal_type, confidence, detected_at,
                candle_timestamp, price_at_detection, acted_upon
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            signal.symbol.as_str(),
            signal.pattern_name.as_str(),
            signal.signal_type.as_str(),
            signal.confidence.value(),
            signal.detected_at,
            signal.detected_at,
            signal.price.value().to_f64().unwrap_or(0.0),
            false
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Gets trading signals for a symbol with optional limit
    pub async fn get_signals_for_symbol(
        &self,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<crate::patterns::signals::TradingSignal>, sqlx::Error> {
        use crate::db::models::TradingSignalRow;
        
        let rows = sqlx::query_as!(
            TradingSignalRow,
            "SELECT * FROM trading_signals WHERE symbol = ? ORDER BY generated_at DESC LIMIT ?",
            symbol,
            limit as i32
        )
        .fetch_all(&self.pool)
        .await?;

        let signals = rows
            .into_iter()
            .filter_map(|row| row.to_trading_signal().ok())
            .collect();

        Ok(signals)
    }

    /// Updates the outcome of a trading signal
    pub async fn update_signal_outcome(
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
                COUNT(*) as total_signals,
                SUM(CASE WHEN signal_type = 'buy' THEN 1 ELSE 0 END) as buy_signals,
                SUM(CASE WHEN signal_type = 'sell' THEN 1 ELSE 0 END) as sell_signals,
                AVG(confidence) as avg_confidence,
                SUM(CASE WHEN confidence >= 0.8 THEN 1 ELSE 0 END) as high_confidence_signals,
                SUM(CASE WHEN acted_upon = TRUE THEN 1 ELSE 0 END) as signals_acted_upon,
                SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END) as win_count,
                SUM(CASE WHEN outcome = 'loss' THEN 1 ELSE 0 END) as loss_count,
                SUM(CASE WHEN outcome = 'pending' THEN 1 ELSE 0 END) as pending_count
            FROM trading_signals
            WHERE generated_at >= ?
            "#,
            from_date
        )
        .fetch_one(&self.pool)
        .await?;

        let total_signals = row.total_signals.unwrap_or(0) as i32;
        let win_count = row.win_count.unwrap_or(0) as i32;
        let loss_count = row.loss_count.unwrap_or(0) as i32;
        let total_decided = win_count + loss_count;
        
        let win_rate = if total_decided > 0 {
            win_count as f64 / total_decided as f64
        } else {
            0.0
        };

        Ok(crate::db::models::SignalStatistics {
            total_signals,
            buy_signals: row.buy_signals.unwrap_or(0) as i32,
            sell_signals: row.sell_signals.unwrap_or(0) as i32,
            avg_confidence: row.avg_confidence.unwrap_or(0.0),
            high_confidence_signals: row.high_confidence_signals.unwrap_or(0) as i32,
            signals_acted_upon: row.signals_acted_upon.unwrap_or(0) as i32,
            win_count,
            loss_count,
            pending_count: row.pending_count.unwrap_or(0) as i32,
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
        
        let rows = sqlx::query_as!(
            IndicatorValueRow,
            r#"
            SELECT * FROM indicator_values
            WHERE symbol = ? AND indicator_name = ? AND timestamp >= ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
            symbol,
            indicator_name,
            since,
            limit
        )
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
    pub async fn store_trade_record(&self, record: &crate::risk::pattern_stats::TradeRecord) -> Result<i64, sqlx::Error> {
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
    pub async fn store_risk_adjustment(&self, audit: &RiskAdjustmentAudit) -> Result<i64, sqlx::Error> {
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
    pub async fn store_risk_event(&self, event: &crate::events::types::RiskViolation) -> Result<i64, sqlx::Error> {
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
    pub async fn get_trade_history(&self, pattern_name: &str, since: DateTime<Utc>) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let records = sqlx::query_as!(
            TradeRecord,
            "SELECT * FROM pattern_trade_history WHERE pattern_name = ? AND trade_timestamp >= ? ORDER BY trade_timestamp DESC",
            pattern_name,
            since
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Gets risk adjustment history for a pattern
    pub async fn get_risk_adjustments(&self, pattern_name: &str, limit: i32) -> Result<Vec<RiskAdjustmentAudit>, sqlx::Error> {
        let adjustments = sqlx::query_as!(
            RiskAdjustmentAudit,
            "SELECT * FROM risk_adjustment_audit WHERE pattern_name = ? ORDER BY timestamp DESC LIMIT ?",
            pattern_name,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(adjustments)
    }

    /// Gets recent risk events
    pub async fn get_recent_risk_events(&self, limit: i32) -> Result<Vec<RiskEvent>, sqlx::Error> {
        let events = sqlx::query_as!(
            RiskEvent,
            "SELECT * FROM risk_events ORDER BY timestamp DESC LIMIT ?",
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Gets performance summary for all patterns
    pub async fn get_performance_summary(&self) -> Result<SystemPerformance, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(DISTINCT ps.pattern_name) as total_patterns,
                COUNT(DISTINCT CASE WHEN ps.is_enabled THEN ps.pattern_name END) as enabled_patterns,
                SUM(ps.total_trades) as total_trades,
                AVG(ps.win_rate) as avg_win_rate,
                SUM(ps.total_profit) as total_profit
            FROM pattern_stats ps
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        let system_win_rate = row.avg_win_rate.unwrap_or(0.0);

        Ok(SystemPerformance {
            total_patterns: row.total_patterns.unwrap_or(0) as i32,
            enabled_patterns: row.enabled_patterns.unwrap_or(0) as i32,
            total_trades: row.total_trades.unwrap_or(0) as i32,
            system_win_rate,
            total_profit: row.total_profit.unwrap_or(0.0),
        })
    }

    /// Cleans up old data based on retention policies
    pub async fn cleanup_old_data(&self, retention_days: i32) -> Result<(), sqlx::Error> {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        // Clean up old trade history
        sqlx::query!("DELETE FROM pattern_trade_history WHERE created_at < ?", cutoff)
            .execute(&self.pool)
            .await?;

        // Clean up old risk events
        sqlx::query!("DELETE FROM risk_events WHERE created_at < ?", cutoff)
            .execute(&self.pool)
            .await?;

        // Clean up old rolling stats (keep only recent windows)
        let window_cutoff = Utc::now() - Duration::days(90);
        sqlx::query!("DELETE FROM pattern_rolling_stats WHERE window_end < ?", window_cutoff)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}