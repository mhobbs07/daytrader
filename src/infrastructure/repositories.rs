//! Infrastructure repository implementations
//!
//! These are the concrete implementations of domain repository traits,
//! providing actual data access using the existing database code.

use async_trait::async_trait;
use std::sync::Arc;
use crate::db::models::*;
use crate::domain::models::*;
use crate::domain::ports::repositories::*;
use crate::db::repositories as db_repos;

/// SQLite implementation of PatternRepository
pub struct SqlitePatternRepository {
    inner: db_repos::PatternRepository,
}

impl SqlitePatternRepository {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            inner: db_repos::PatternRepository::new(pool),
        }
    }
}

#[async_trait]
impl PatternRepository for SqlitePatternRepository {
    async fn store_pattern_stats(&self, stats: &PatternStats) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_pattern_stats(&crate::risk::pattern_stats::PatternStats::from(stats.clone()))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_pattern_stats(&self, pattern_name: &str) -> Result<Option<PatternStats>, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_pattern_stats(pattern_name)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_all_pattern_stats(&self) -> Result<Vec<PatternStats>, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_all_pattern_stats()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn update_pattern_status(&self, pattern_name: &str, is_enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.update_pattern_status(pattern_name, is_enabled)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

/// SQLite implementation of SignalRepository
pub struct SqliteSignalRepository {
    inner: db_repos::SignalRepository,
}

impl SqliteSignalRepository {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            inner: db_repos::SignalRepository::new(pool),
        }
    }
}

#[async_trait]
impl SignalRepository for SqliteSignalRepository {
    async fn store_signal(&self, signal: &PatternSignal) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_signal(&crate::patterns::signals::TradingSignal::from(signal.clone()))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn update_signal_with_order(&self, signal_id: i64, order_id: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.update_signal_with_order(signal_id, order_id)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn update_signal_outcome(&self, signal_id: i64, outcome: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.update_signal_outcome(signal_id, outcome)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_signals_for_symbol(&self, symbol: &str, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_signals_for_symbol(symbol, since)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_recent_signals(&self, limit: i32) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_recent_signals(limit)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

/// SQLite implementation of PerformanceRepository
pub struct SqlitePerformanceRepository {
    inner: db_repos::PerformanceRepository,
}

impl SqlitePerformanceRepository {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            inner: db_repos::PerformanceRepository::new(pool),
        }
    }
}

#[async_trait]
impl PerformanceRepository for SqlitePerformanceRepository {
    async fn store_trade_record(&self, record: &TradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_trade_record(&crate::risk::pattern_stats::TradeRecord::from(record.clone()))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn store_risk_adjustment(&self, audit: &RiskAdjustmentAudit) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_risk_adjustment(audit)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn store_risk_event(&self, event: &RiskEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_risk_event(&crate::events::types::RiskViolation::from(event.clone()))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn store_audit_log(&self, audit: &AuditLog) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.store_audit_log(audit)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_trade_history(&self, pattern_name: &str, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<TradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_trade_history(pattern_name, since)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_performance_summary(&self) -> Result<SystemPerformance, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.get_performance_summary()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

/// SQLite implementation of PdtRepository
pub struct SqlitePdtRepository {
    inner: db_repos::PdtRepository,
}

impl SqlitePdtRepository {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            inner: db_repos::PdtRepository::new(pool),
        }
    }
}

#[async_trait]
impl PdtRepository for SqlitePdtRepository {
    async fn record_day_trade(&self, day_trade: &DayTradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let db_record = crate::db::models::DayTradeRecord::from(day_trade.clone());
        self.inner.record_day_trade(&db_record)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn record_audit_event(&self, event: &PdtAuditEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let db_event = crate::db::models::PdtAuditEvent::from(event.clone());
        self.inner.record_audit_event(&db_event)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn upsert_pdt_tracking(&self, pdt_tracking: &PdtTracking) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db_tracking = crate::db::models::PdtTracking::from(pdt_tracking.clone());
        self.inner.upsert_pdt_tracking(&db_tracking)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_day_trades_in_range(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let db_records = self.inner.get_day_trades_in_range(account_id, start_date, end_date)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(db_records.into_iter().map(DayTradeRecord::from).collect())
    }

    async fn count_day_trades_in_window(
        &self,
        account_id: i64,
        window_start: chrono::NaiveDate,
        window_end: chrono::NaiveDate,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        self.inner.count_day_trades_in_window(account_id, window_start, window_end)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_current_pdt_tracking(&self, account_id: i64) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let db_tracking = self.inner.get_current_pdt_tracking(account_id)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(db_tracking.map(PdtTracking::from))
    }

    async fn get_pdt_tracking_history(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let db_history = self.inner.get_pdt_tracking_history(account_id, start_date, end_date)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(db_history.into_iter().map(PdtTracking::from).collect())
    }

    async fn get_audit_events(
        &self,
        account_id: i64,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let db_events = self.inner.get_audit_events(account_id, limit)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(db_events.into_iter().map(PdtAuditEvent::from).collect())
    }

    async fn get_day_trade_summary(
        &self,
        account_id: i64,
        date_range_days: i32,
    ) -> Result<DayTradeSummary, Box<dyn std::error::Error + Send + Sync>> {
        let db_summary = self.inner.get_day_trade_summary(account_id, date_range_days)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(DayTradeSummary::from(db_summary))
    }

    async fn cleanup_old_records(&self, retention_years: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.cleanup_old_records(retention_years)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}