//! Mock implementations for testing
//!
//! These provide test doubles that don't require database setup,
//! allowing for fast, isolated unit tests.

use async_trait::async_trait;
use crate::domain::models::*;
use crate::domain::ports::repositories::*;

/// Mock implementation of PatternRepository for testing
#[derive(Clone)]
pub struct MockPatternRepository;

#[async_trait]
impl PatternRepository for MockPatternRepository {
    async fn store_pattern_stats(&self, _stats: &PatternStats) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn get_pattern_stats(&self, _pattern_name: &str) -> Result<Option<PatternStats>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(None)
    }

    async fn get_all_pattern_stats(&self) -> Result<Vec<PatternStats>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn update_pattern_status(&self, _pattern_name: &str, _is_enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// Mock implementation of SignalRepository for testing
#[derive(Clone)]
pub struct MockSignalRepository;

#[async_trait]
impl SignalRepository for MockSignalRepository {
    async fn store_signal(&self, _signal: &PatternSignal) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn update_signal_with_order(&self, _signal_id: i64, _order_id: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn update_signal_outcome(&self, _signal_id: i64, _outcome: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn get_signals_for_symbol(&self, _symbol: &str, _since: chrono::DateTime<chrono::Utc>) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn get_recent_signals(&self, _limit: i32) -> Result<Vec<PatternSignal>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
}

/// Mock implementation of PerformanceRepository for testing
#[derive(Clone)]
pub struct MockPerformanceRepository;

#[async_trait]
impl PerformanceRepository for MockPerformanceRepository {
    async fn store_trade_record(&self, _record: &TradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn store_risk_adjustment(&self, _audit: &RiskAdjustmentAudit) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn store_risk_event(&self, _event: &RiskEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn store_audit_log(&self, _audit: &AuditLog) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn get_trade_history(&self, _pattern_name: &str, _since: chrono::DateTime<chrono::Utc>) -> Result<Vec<TradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn get_performance_summary(&self) -> Result<SystemPerformance, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SystemPerformance {
            total_patterns: 0,
            active_patterns: 0,
            total_signals_today: 0,
            total_trades_today: 0,
            win_rate_today: 0.0,
            total_profit_loss_today: 0.0,
            total_signals_week: 0,
            total_trades_week: 0,
            win_rate_week: 0.0,
            total_profit_loss_week: 0.0,
            total_signals_month: 0,
            total_trades_month: 0,
            win_rate_month: 0.0,
            total_profit_loss_month: 0.0,
            last_updated: chrono::Utc::now(),
        })
    }
}

/// Mock implementation of PdtRepository for testing
#[derive(Clone)]
pub struct MockPdtRepository;

#[async_trait]
impl PdtRepository for MockPdtRepository {
    async fn record_day_trade(&self, _day_trade: &DayTradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn record_audit_event(&self, _event: &PdtAuditEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(1)
    }

    async fn upsert_pdt_tracking(&self, _pdt_tracking: &PdtTracking) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn get_day_trades_in_range(
        &self,
        _account_id: i64,
        _start_date: chrono::NaiveDate,
        _end_date: chrono::NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn count_day_trades_in_window(
        &self,
        _account_id: i64,
        _window_start: chrono::NaiveDate,
        _window_end: chrono::NaiveDate,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(0)
    }

    async fn get_current_pdt_tracking(&self, _account_id: i64) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(None)
    }

    async fn get_pdt_tracking_history(
        &self,
        _account_id: i64,
        _start_date: chrono::NaiveDate,
        _end_date: chrono::NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn get_audit_events(
        &self,
        _account_id: i64,
        _limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    async fn get_day_trade_summary(
        &self,
        _account_id: i64,
        _date_range_days: i32,
    ) -> Result<DayTradeSummary, Box<dyn std::error::Error + Send + Sync>> {
        Ok(DayTradeSummary {
            account_id: _account_id,
            date_range_days: _date_range_days,
            total_trades: 0,
            total_profit_loss: 0.0,
            winning_trades: 0,
            losing_trades: 0,
            avg_profit_loss: 0.0,
            max_profit: 0.0,
            max_loss: 0.0,
            unique_symbols: 0,
            trading_days: 0,
            win_rate: 0.0,
        })
    }

    async fn cleanup_old_records(&self, _retention_years: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}