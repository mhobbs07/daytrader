//! Repository interfaces for domain layer
//!
//! These traits define the contracts that repositories must implement,
//! allowing business logic to be independent of data access implementations.

use async_trait::async_trait;
use crate::domain::models::*;

/// Repository for pattern-related data operations
#[async_trait]
pub trait PatternRepository: Send + Sync {
    /// Store pattern statistics
    async fn store_pattern_stats(&self, stats: &PatternStats) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get pattern statistics by name
    async fn get_pattern_stats(&self, pattern_name: &str) -> Result<Option<PatternStats>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get all pattern statistics
    async fn get_all_pattern_stats(&self) -> Result<Vec<PatternStats>, Box<dyn std::error::Error + Send + Sync>>;

    /// Update pattern enable/disable status
    async fn update_pattern_status(&self, pattern_name: &str, is_enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Repository for signal-related data operations
#[async_trait]
pub trait SignalRepository: Send + Sync {
    /// Store a trading signal
    async fn store_signal(&self, signal: &PatternSignal) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Update signal with order information
    async fn update_signal_with_order(&self, signal_id: i64, order_id: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Update signal with trade outcome
    async fn update_signal_outcome(&self, signal_id: i64, outcome: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get signals for a symbol within time range
    async fn get_signals_for_symbol(&self, symbol: &str, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get recent signals across all patterns
    async fn get_recent_signals(&self, limit: i32) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Repository for performance analytics data operations
#[async_trait]
pub trait PerformanceRepository: Send + Sync {
    /// Store trade history for pattern analysis
    async fn store_trade_record(&self, record: &TradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Store risk adjustment audit entry
    async fn store_risk_adjustment(&self, audit: &RiskAdjustmentAudit) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Store risk event
    async fn store_risk_event(&self, event: &RiskEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Store audit log entry
    async fn store_audit_log(&self, audit: &AuditLog) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Get trade history for a pattern within date range
    async fn get_trade_history(&self, pattern_name: &str, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<TradeRecord>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get performance summary for all patterns
    async fn get_performance_summary(&self) -> Result<SystemPerformance, Box<dyn std::error::Error + Send + Sync>>;
}

/// Repository for PDT (Pattern Day Trading) compliance operations
#[async_trait]
pub trait PdtRepository: Send + Sync {
    /// Record a new day trade for audit trail compliance
    async fn record_day_trade(&self, day_trade: &DayTradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Record a PDT audit event
    async fn record_audit_event(&self, event: &PdtAuditEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    /// Update or insert PDT tracking record for a specific day
    async fn upsert_pdt_tracking(&self, pdt_tracking: &PdtTracking) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get day trades for an account within a specific date range
    async fn get_day_trades_in_range(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>>;

    /// Count day trades for an account within a rolling window
    async fn count_day_trades_in_window(
        &self,
        account_id: i64,
        window_start: chrono::NaiveDate,
        window_end: chrono::NaiveDate,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>>;

    /// Get the current PDT tracking record for an account
    async fn get_current_pdt_tracking(&self, account_id: i64) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get PDT tracking records for an account within a date range
    async fn get_pdt_tracking_history(
        &self,
        account_id: i64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get PDT audit events for an account
    async fn get_audit_events(
        &self,
        account_id: i64,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get day trade summary statistics for compliance reporting
    async fn get_day_trade_summary(
        &self,
        account_id: i64,
        date_range_days: i32,
    ) -> Result<DayTradeSummary, Box<dyn std::error::Error + Send + Sync>>;

    /// Clean up old PDT records based on regulatory retention requirements
    async fn cleanup_old_records(&self, retention_years: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}