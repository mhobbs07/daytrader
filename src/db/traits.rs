//! Traits for database repositories to enable mocking and testing
//!
//! This module defines traits that abstract database operations,
//! allowing for easy mocking in tests and alternative implementations.

use crate::db::models::*;
use async_trait::async_trait;

/// Trait for PDT (Pattern Day Trading) repository operations
#[async_trait]
pub trait PdtRepositoryTrait: Send + Sync {
    /// Records a new day trade for audit trail compliance
    async fn record_day_trade(&self, day_trade: &DayTradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Records a PDT audit event
    async fn record_audit_event(&self, event: &PdtAuditEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Updates or inserts PDT tracking record for a specific day
    async fn upsert_pdt_tracking(&self, pdt_tracking: &PdtTracking) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Gets day trades for an account within a specific date range
    async fn get_day_trades_in_range(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>>;

    /// Counts day trades for an account within a rolling window
    async fn count_day_trades_in_window(
        &self,
        account_id: i64,
        window_start: chrono::NaiveDate,
        window_end: chrono::NaiveDate,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>>;

    /// Gets the current PDT tracking record for an account
    async fn get_current_pdt_tracking(&self, account_id: i64) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>>;

    /// Gets PDT tracking records for an account within a date range
    async fn get_pdt_tracking_history(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>>;

    /// Gets PDT audit events for an account
    async fn get_audit_events(
        &self,
        account_id: i64,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>>;

    /// Gets audit events by type for an account
    async fn get_audit_events_by_type(
        &self,
        account_id: i64,
        event_type: &str,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>>;

    /// Gets day trade summary statistics for compliance reporting
    async fn get_day_trade_summary(
        &self,
        account_id: i64,
        date_range_days: i32,
    ) -> Result<DayTradeSummary, Box<dyn std::error::Error + Send + Sync>>;

    /// Cleans up old PDT records based on regulatory retention requirements
    async fn cleanup_old_records(&self, retention_years: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}