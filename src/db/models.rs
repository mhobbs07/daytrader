//! Database models and entity representations
//!
//! This module defines the data structures that map to database tables,
//! providing type-safe interfaces for database operations.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Pattern signal stored in database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PatternSignal {
    pub id: i64,
    pub symbol: String,
    pub pattern_name: String,
    pub signal_type: String,
    pub confidence: f64,
    pub detected_at: DateTime<Utc>,
    pub candle_timestamp: DateTime<Utc>,
    pub price_at_detection: f64,
    pub acted_upon: bool,
    pub order_id: Option<i64>,
    pub outcome: Option<String>,
}

/// Pattern statistics stored in database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PatternStats {
    pub id: i64,
    pub pattern_name: String,
    pub total_signals: i32,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_profit: f64,
    pub win_rate: f64,
    pub avg_profit_per_trade: f64,
    pub consecutive_losses: i32,
    pub is_enabled: bool,
    pub avg_confidence: f64,
    pub confidence_weighted_win_rate: f64,
    pub last_updated: DateTime<Utc>,
}

/// Rolling window statistics for pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RollingStats {
    pub id: i64,
    pub pattern_name: String,
    pub window_days: i32,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_profit: f64,
    pub win_rate: f64,
    pub avg_profit_per_trade: f64,
    pub consecutive_losses: i32,
    pub performance_score: f64,
    pub risk_multiplier: f64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// Individual trade record for rolling window analysis
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TradeRecord {
    pub id: i64,
    pub pattern_name: String,
    pub trade_timestamp: DateTime<Utc>,
    pub confidence: f64,
    pub profit: f64,
    pub was_profitable: bool,
    pub order_id: Option<String>,
    pub symbol: Option<String>,
    pub quantity: Option<i32>,
    pub price: Option<f64>,
    pub commission: Option<f64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Risk adjustment audit log entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RiskAdjustmentAudit {
    pub id: i64,
    pub pattern_name: String,
    pub adjustment_type: String,
    pub old_risk_multiplier: Option<f64>,
    pub new_risk_multiplier: f64,
    pub performance_score: Option<f64>,
    pub trigger_reason: String,
    pub window_days: Option<i32>,
    pub trade_count: Option<i32>,
    pub win_rate: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

/// System-wide performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemPerformance {
    pub total_patterns: i32,
    pub enabled_patterns: i32,
    pub total_trades: i32,
    pub system_win_rate: f64,
    pub total_profit: f64,
}

/// Risk event for audit and compliance
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RiskEvent {
    pub id: i64,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub account_id: Option<i64>,
    pub pattern_name: Option<String>,
    pub violation_type: Option<String>,
    pub severity: Option<String>,
    pub message: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Daily risk metrics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyRiskMetrics {
    pub id: i64,
    pub account_id: i64,
    pub trading_date: chrono::NaiveDate,
    pub starting_balance: f64,
    pub current_balance: f64,
    pub realized_pl: f64,
    pub unrealized_pl: f64,
    pub total_pl: f64,
    pub day_trades_count: i32,
    pub trades_count: i32,
    pub largest_loss: f64,
    pub is_trading_halted: bool,
    pub halt_reason: Option<String>,
    pub max_loss_limit_dollars: Option<f64>,
    pub max_loss_limit_percent: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Position risk tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionRiskTracking {
    pub id: i64,
    pub account_id: i64,
    pub symbol: String,
    pub position_size: f64,
    pub market_value: f64,
    pub portfolio_allocation_percent: f64,
    pub max_position_limit_dollars: Option<f64>,
    pub max_portfolio_limit_percent: Option<f64>,
    pub is_violation: bool,
    pub violation_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PDT tracking for pattern day trading rules
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PdtTracking {
    pub id: i64,
    pub account_id: i64,
    pub trading_date: chrono::NaiveDate,
    pub day_trades_count: i32,
    pub rolling_window_start: chrono::NaiveDate,
    pub rolling_window_end: chrono::NaiveDate,
    pub total_day_trades_window: i32,
    pub equity_value: f64,
    pub min_equity_threshold: Option<f64>,
    pub max_day_trades_allowed: Option<i32>,
    pub is_restricted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Conversion implementations for domain types to database types

impl From<crate::patterns::signals::TradingSignal> for PatternSignal {
    fn from(signal: crate::patterns::signals::TradingSignal) -> Self {
        Self {
            id: 0, // Will be set by database
            symbol: signal.symbol.as_str().to_string(),
            pattern_name: signal.components.first()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            signal_type: signal.signal_type.as_str().to_string(),
            confidence: signal.confidence.value(),
            detected_at: signal.generated_at,
            candle_timestamp: signal.generated_at, // Approximate
            price_at_detection: signal.price_level.value().to_f64().unwrap_or(0.0),
            acted_upon: false,
            order_id: None,
            outcome: None,
        }
    }
}

impl From<crate::risk::pattern_stats::PatternStats> for PatternStats {
    fn from(stats: crate::risk::pattern_stats::PatternStats) -> Self {
        Self {
            id: 0, // Will be set by database
            pattern_name: stats.pattern_name,
            total_signals: stats.total_signals as i32,
            total_trades: stats.total_trades as i32,
            winning_trades: stats.winning_trades as i32,
            losing_trades: stats.losing_trades as i32,
            total_profit: stats.total_profit.to_f64().unwrap_or(0.0),
            win_rate: stats.win_rate,
            avg_profit_per_trade: stats.avg_profit_per_trade.to_f64().unwrap_or(0.0),
            consecutive_losses: stats.consecutive_losses as i32,
            is_enabled: stats.is_enabled,
            avg_confidence: stats.avg_confidence,
            confidence_weighted_win_rate: stats.confidence_weighted_win_rate,
            last_updated: stats.last_updated,
        }
    }
}

impl From<crate::risk::pattern_stats::TradeRecord> for TradeRecord {
    fn from(record: crate::risk::pattern_stats::TradeRecord) -> Self {
        Self {
            id: 0, // Will be set by database
            pattern_name: record.pattern_name,
            trade_timestamp: record.timestamp,
            confidence: record.confidence,
            profit: record.profit.to_f64().unwrap_or(0.0),
            was_profitable: record.was_profitable,
            order_id: None,
            symbol: None,
            quantity: None,
            price: None,
            commission: None,
            notes: None,
            created_at: Utc::now(),
        }
    }
}

impl From<crate::events::types::RiskViolation> for RiskEvent {
    fn from(violation: crate::events::types::RiskViolation) -> Self {
        Self {
            id: 0, // Will be set by database
            event_type: "RiskViolation".to_string(),
            timestamp: violation.timestamp,
            account_id: None, // Could be added later
            pattern_name: violation.pattern_name.map(|p| p.as_str().to_string()),
            violation_type: Some(violation.violation_type.as_str().to_string()),
            severity: Some("moderate".to_string()), // Could be enhanced
            message: violation.message.as_str().to_string(),
            details: None,
            created_at: Utc::now(),
        }
    }
}