//! Domain models
//!
//! These are the core business entities that represent the domain concepts,
//! independent of any infrastructure or persistence concerns.

use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;

/// Conversion implementations between domain models and database models
impl From<DayTradeRecord> for crate::db::models::DayTradeRecord {
    fn from(record: DayTradeRecord) -> Self {
        Self {
            id: record.id.unwrap_or(0),
            account_id: record.account_id,
            trade_id: record.trade_id,
            symbol: record.symbol,
            quantity: record.quantity,
            open_time: record.open_time,
            close_time: record.close_time,
            open_price: record.open_price,
            close_price: record.close_price,
            realized_pl: record.realized_pl,
            commission: record.commission,
            pattern_name: record.pattern_name,
            is_validated: record.is_validated,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<crate::db::models::DayTradeRecord> for DayTradeRecord {
    fn from(record: crate::db::models::DayTradeRecord) -> Self {
        Self {
            id: Some(record.id),
            account_id: record.account_id,
            trade_id: record.trade_id,
            symbol: record.symbol,
            quantity: record.quantity,
            open_time: record.open_time,
            close_time: record.close_time,
            open_price: record.open_price,
            close_price: record.close_price,
            realized_pl: record.realized_pl,
            commission: record.commission,
            pattern_name: record.pattern_name,
            is_validated: record.is_validated,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<PdtAuditEvent> for crate::db::models::PdtAuditEvent {
    fn from(event: PdtAuditEvent) -> Self {
        Self {
            id: event.id.unwrap_or(0),
            account_id: event.account_id,
            event_type: event.event_type,
            event_date: event.event_date,
            day_trade_count: event.day_trade_count,
            equity_value: event.equity_value,
            min_equity_threshold: event.min_equity_threshold,
            max_day_trades_allowed: event.max_day_trades_allowed,
            is_restricted: event.is_restricted,
            violation_details: event.violation_details,
            trade_reference: event.trade_reference,
            message: event.message,
            metadata: event.metadata,
            created_at: event.created_at,
        }
    }
}

impl From<crate::db::models::PdtAuditEvent> for PdtAuditEvent {
    fn from(event: crate::db::models::PdtAuditEvent) -> Self {
        Self {
            id: Some(event.id),
            account_id: event.account_id,
            event_type: event.event_type,
            event_date: event.event_date,
            day_trade_count: event.day_trade_count,
            equity_value: event.equity_value,
            min_equity_threshold: event.min_equity_threshold,
            max_day_trades_allowed: event.max_day_trades_allowed,
            is_restricted: event.is_restricted,
            violation_details: event.violation_details,
            trade_reference: event.trade_reference,
            message: event.message,
            metadata: event.metadata,
            created_at: event.created_at,
        }
    }
}

impl From<PdtTracking> for crate::db::models::PdtTracking {
    fn from(tracking: PdtTracking) -> Self {
        Self {
            id: tracking.id.unwrap_or(0),
            account_id: tracking.account_id,
            trading_date: tracking.trading_date,
            day_trades_count: tracking.day_trades_count,
            rolling_window_start: tracking.rolling_window_start,
            rolling_window_end: tracking.rolling_window_end,
            total_day_trades_window: tracking.total_day_trades_window,
            equity_value: tracking.equity_value,
            min_equity_threshold: tracking.min_equity_threshold,
            max_day_trades_allowed: tracking.max_day_trades_allowed,
            is_restricted: tracking.is_restricted,
            created_at: tracking.created_at,
            updated_at: tracking.updated_at,
        }
    }
}

impl From<crate::db::models::PdtTracking> for PdtTracking {
    fn from(tracking: crate::db::models::PdtTracking) -> Self {
        Self {
            id: Some(tracking.id),
            account_id: tracking.account_id,
            trading_date: tracking.trading_date,
            day_trades_count: tracking.day_trades_count,
            rolling_window_start: tracking.rolling_window_start,
            rolling_window_end: tracking.rolling_window_end,
            total_day_trades_window: tracking.total_day_trades_window,
            equity_value: tracking.equity_value,
            min_equity_threshold: tracking.min_equity_threshold,
            max_day_trades_allowed: tracking.max_day_trades_allowed,
            is_restricted: tracking.is_restricted,
            created_at: tracking.created_at,
            updated_at: tracking.updated_at,
        }
    }
}

impl From<DayTradeSummary> for crate::db::traits::DayTradeSummary {
    fn from(summary: DayTradeSummary) -> Self {
        Self {
            account_id: summary.account_id,
            date_range_days: summary.date_range_days,
            total_trades: summary.total_trades,
            total_profit_loss: summary.total_profit_loss,
            winning_trades: summary.winning_trades,
            losing_trades: summary.losing_trades,
            avg_profit_loss: summary.avg_profit_loss,
            max_profit: summary.max_profit,
            max_loss: summary.max_loss,
            unique_symbols: summary.unique_symbols,
            trading_days: summary.trading_days,
            win_rate: summary.win_rate,
        }
    }
}

impl From<crate::db::traits::DayTradeSummary> for DayTradeSummary {
    fn from(summary: crate::db::traits::DayTradeSummary) -> Self {
        Self {
            account_id: summary.account_id,
            date_range_days: summary.date_range_days,
            total_trades: summary.total_trades,
            total_profit_loss: summary.total_profit_loss,
            winning_trades: summary.winning_trades,
            losing_trades: summary.losing_trades,
            avg_profit_loss: summary.avg_profit_loss,
            max_profit: summary.max_profit,
            max_loss: summary.max_loss,
            unique_symbols: summary.unique_symbols,
            trading_days: summary.trading_days,
            win_rate: summary.win_rate,
        }
    }
}

/// Pattern statistics for performance tracking
#[derive(Debug, Clone)]
pub struct PatternStats {
    pub pattern_name: String,
    pub total_signals: i32,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_profit_loss: f64,
    pub avg_profit_loss: f64,
    pub max_profit: f64,
    pub max_loss: f64,
    pub win_rate: f64,
    pub is_enabled: bool,
    pub last_updated: DateTime<Utc>,
}

/// Trading signal generated by pattern analysis
#[derive(Debug, Clone)]
pub struct PatternSignal {
    pub id: Option<i64>,
    pub symbol: String,
    pub pattern_name: String,
    pub signal_type: String,
    pub confidence: f64,
    pub detected_at: DateTime<Utc>,
    pub order_id: Option<i64>,
    pub acted_upon: bool,
    pub outcome: Option<String>,
}

/// Trade record for performance analysis
#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub id: Option<i64>,
    pub pattern_name: String,
    pub trade_timestamp: DateTime<Utc>,
    pub confidence: f64,
    pub profit: f64,
    pub was_profitable: bool,
    pub created_at: DateTime<Utc>,
}

/// Risk adjustment audit entry
#[derive(Debug, Clone)]
pub struct RiskAdjustmentAudit {
    pub id: Option<i64>,
    pub pattern_name: String,
    pub adjustment_type: String,
    pub old_risk_multiplier: f64,
    pub new_risk_multiplier: f64,
    pub timestamp: DateTime<Utc>,
    pub reason: String,
}

/// Risk event for compliance tracking
#[derive(Debug, Clone)]
pub struct RiskEvent {
    pub id: Option<i64>,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub account_id: i64,
    pub pattern_name: Option<String>,
    pub violation_type: String,
    pub severity: String,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditLog {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub severity: String,
    pub user_id: Option<String>,
    pub account_id: Option<i64>,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
}

/// System performance summary
#[derive(Debug, Clone)]
pub struct SystemPerformance {
    pub total_patterns: i32,
    pub active_patterns: i32,
    pub total_signals_today: i32,
    pub total_trades_today: i32,
    pub win_rate_today: f64,
    pub total_profit_loss_today: f64,
    pub total_signals_week: i32,
    pub total_trades_week: i32,
    pub win_rate_week: f64,
    pub total_profit_loss_week: f64,
    pub total_signals_month: i32,
    pub total_trades_month: i32,
    pub win_rate_month: f64,
    pub total_profit_loss_month: f64,
    pub last_updated: DateTime<Utc>,
}

/// Day trade record for PDT compliance
#[derive(Debug, Clone)]
pub struct DayTradeRecord {
    pub id: Option<i64>,
    pub account_id: i64,
    pub trade_id: String,
    pub symbol: String,
    pub quantity: i32,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open_price: f64,
    pub close_price: f64,
    pub realized_pl: f64,
    pub commission: f64,
    pub pattern_name: Option<String>,
    pub is_validated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PDT audit event for compliance tracking
#[derive(Debug, Clone)]
pub struct PdtAuditEvent {
    pub id: Option<i64>,
    pub account_id: i64,
    pub event_type: String,
    pub event_date: NaiveDate,
    pub day_trade_count: Option<i32>,
    pub equity_value: f64,
    pub min_equity_threshold: Option<f64>,
    pub max_day_trades_allowed: Option<i32>,
    pub is_restricted: bool,
    pub violation_details: Option<String>,
    pub trade_reference: Option<String>,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// PDT tracking record for daily compliance monitoring
#[derive(Debug, Clone)]
pub struct PdtTracking {
    pub id: Option<i64>,
    pub account_id: i64,
    pub trading_date: NaiveDate,
    pub day_trades_count: i32,
    pub rolling_window_start: NaiveDate,
    pub rolling_window_end: NaiveDate,
    pub total_day_trades_window: i32,
    pub equity_value: f64,
    pub min_equity_threshold: Option<f64>,
    pub max_day_trades_allowed: Option<i32>,
    pub is_restricted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Day trade summary for compliance reporting
#[derive(Debug, Clone)]
pub struct DayTradeSummary {
    pub account_id: i64,
    pub date_range_days: i32,
    pub total_trades: i32,
    pub total_profit_loss: f64,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub avg_profit_loss: f64,
    pub max_profit: f64,
    pub max_loss: f64,
    pub unique_symbols: i32,
    pub trading_days: i32,
    pub win_rate: f64,
}