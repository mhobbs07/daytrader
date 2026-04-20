//! Automated Execution Module
//!
//! This module provides automated execution capabilities that connect pattern signals
//! to order placement, implementing the core automation layer for the trading system.
//!
//! # Architecture
//!
//! The execution module consists of several key components:
//!
//! - **Strategies** ([`strategy`]): Execution strategy trait and implementations
//! - **Engine** ([`engine`]): Main execution engine coordinating signal-to-order pipeline
//! - **Rules** ([`rules`]): Entry/exit rule evaluation logic
//!
//! # Examples
//!
//! ```no_run
//! use daytrader::execution::{ExecutionEngine, ExecutionConfig};
//! use daytrader::execution::strategy::{ConservativeStrategy, ConservativeConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create strategy
//! let strategy_config = ConservativeConfig::default();
//! let strategy = Arc::new(ConservativeStrategy::new(strategy_config));
//!
//! // Create execution engine with strategy
//! # let order_engine = todo!();
//! # let position_tracker = todo!();
//! # let risk_engine = todo!();
//! # let event_bus = todo!();
//! # let db_pool = todo!();
//! let config = ExecutionConfig::default();
//! let engine = ExecutionEngine::new(
//!     order_engine,
//!     position_tracker,
//!     risk_engine,
//!     event_bus,
//!     db_pool,
//!     strategy,
//!     config,
//! ).await?;
//!
//! // Start automated execution
//! engine.start().await?;
//! # Ok(())
//! # }
//! ```

pub mod engine;
pub mod rules;
pub mod strategy;

// Re-export primary types
pub use engine::{ExecutionConfig, ExecutionEngine, ExecutionMetrics};
pub use rules::{EntryRule, ExitRule, RuleEngine, RuleEvaluation};
pub use strategy::ExecutionContext;
pub use strategy::{
    AggressiveConfig, AggressiveStrategy, ConservativeConfig, ConservativeStrategy,
    ExecutionStrategy,
};

use crate::brokers::types::{Message, PatternName};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during execution
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// Order placement failed
    #[error("Order placement failed: {0}")]
    OrderPlacementFailed(String),

    /// Risk violation prevents execution
    #[error("Risk violation: {0}")]
    RiskViolation(String),

    /// Strategy rejected the signal
    #[error("Strategy rejected signal: {0}")]
    StrategyRejection(String),

    /// Invalid signal data
    #[error("Invalid signal: {0}")]
    InvalidSignal(String),

    /// Position sizing error
    #[error("Position sizing error: {0}")]
    PositionSizingError(String),

    /// Database operation failed
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    /// Order engine error
    #[error("Order error: {0}")]
    OrderError(#[from] crate::orders::types::OrderError),

    /// Position tracking error
    #[error("Position error: {0}")]
    PositionError(#[from] crate::positions::tracker::PositionError),

    /// Broker error
    #[error("Broker error: {0}")]
    BrokerError(#[from] crate::utils::errors::BrokerError),

    /// Internal execution error
    #[error("Internal error: {0}")]
    Internal(String),
}

// ============================================================================
// Execution Events
// ============================================================================

/// Events published by the execution engine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecutionEvent {
    /// Automation globally enabled
    AutoExecutionEnabled,

    /// Automation globally disabled
    AutoExecutionDisabled,

    /// Pattern automation enabled
    PatternAutoEnabled { pattern_name: PatternName },

    /// Pattern automation disabled
    PatternAutoDisabled { pattern_name: PatternName },

    /// Signal automatically executed
    SignalAutoExecuted {
        signal_id: String,
        order_id: crate::brokers::types::OrderId,
        pattern_name: Option<PatternName>,
        confidence: Decimal,
        timestamp: DateTime<Utc>,
    },

    /// Signal requires manual review
    SignalRequiresManualReview {
        signal_id: String,
        reason: String,
        pattern_name: Option<PatternName>,
        timestamp: DateTime<Utc>,
    },

    /// Execution rejected by strategy or rules
    ExecutionRejected {
        signal_id: String,
        reason: String,
        pattern_name: Option<PatternName>,
        timestamp: DateTime<Utc>,
    },

    /// Strategy evaluation completed
    StrategyEvaluated {
        signal_id: String,
        approved: bool,
        reason: Option<String>,
        evaluation_time_ms: u64,
    },
}

// ============================================================================
// Performance Metrics
// ============================================================================

/// Performance metrics for pattern-based execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternPerformance {
    /// Pattern name
    pub pattern_name: PatternName,

    /// Total signals received
    pub total_signals: u32,

    /// Total trades executed
    pub total_trades: u32,

    /// Winning trades
    pub winning_trades: u32,

    /// Losing trades
    pub losing_trades: u32,

    /// Win rate (0.0 to 1.0)
    pub win_rate: Decimal,

    /// Average profit per trade
    pub avg_profit_per_trade: Decimal,

    /// Total profit/loss
    pub total_profit: Decimal,

    /// Consecutive wins (current streak)
    pub consecutive_wins: u32,

    /// Consecutive losses (current streak)
    pub consecutive_losses: u32,

    /// Best single trade P&L
    pub best_trade: Decimal,

    /// Worst single trade P&L
    pub worst_trade: Decimal,

    /// Average holding period in seconds
    pub avg_holding_period_secs: u64,

    /// Pattern enabled for automation
    pub is_enabled: bool,

    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl PatternPerformance {
    /// Creates new pattern performance tracker
    pub fn new(pattern_name: PatternName) -> Self {
        Self {
            pattern_name,
            total_signals: 0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: Decimal::ZERO,
            avg_profit_per_trade: Decimal::ZERO,
            total_profit: Decimal::ZERO,
            consecutive_wins: 0,
            consecutive_losses: 0,
            best_trade: Decimal::ZERO,
            worst_trade: Decimal::ZERO,
            avg_holding_period_secs: 0,
            is_enabled: true,
            last_updated: Utc::now(),
        }
    }

    /// Records a trade outcome
    pub fn record_trade(&mut self, realized_pl: Decimal, holding_period_secs: u64) {
        self.total_trades += 1;

        // Update win/loss tracking
        if realized_pl > Decimal::ZERO {
            self.winning_trades += 1;
            self.consecutive_wins += 1;
            self.consecutive_losses = 0;
            if realized_pl > self.best_trade {
                self.best_trade = realized_pl;
            }
        } else {
            self.losing_trades += 1;
            self.consecutive_losses += 1;
            self.consecutive_wins = 0;
            if realized_pl < self.worst_trade {
                self.worst_trade = realized_pl;
            }
        }

        // Update P&L
        self.total_profit += realized_pl;
        self.win_rate = if self.total_trades > 0 {
            Decimal::from(self.winning_trades) / Decimal::from(self.total_trades)
        } else {
            Decimal::ZERO
        };
        self.avg_profit_per_trade = if self.total_trades > 0 {
            self.total_profit / Decimal::from(self.total_trades)
        } else {
            Decimal::ZERO
        };

        // Update average holding period
        let total_duration =
            (self.avg_holding_period_secs * (self.total_trades - 1) as u64) + holding_period_secs;
        self.avg_holding_period_secs = total_duration / self.total_trades as u64;

        self.last_updated = Utc::now();
    }

    /// Records a signal (for tracking signal-to-trade ratio)
    pub fn record_signal(&mut self) {
        self.total_signals += 1;
        self.last_updated = Utc::now();
    }

    /// Returns the execution rate (trades / signals)
    pub fn execution_rate(&self) -> f64 {
        if self.total_signals == 0 {
            0.0
        } else {
            self.total_trades as f64 / self.total_signals as f64
        }
    }
}

/// Performance thresholds for automation decisions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Minimum trades before enabling automation
    pub min_trades_for_automation: u32,

    /// Minimum win rate required (0.0 to 1.0)
    pub min_win_rate: Decimal,

    /// Maximum consecutive losses before disabling
    pub max_consecutive_losses: u32,

    /// Minimum average profit per trade
    pub min_avg_profit: Decimal,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            min_trades_for_automation: 20,
            min_win_rate: Decimal::new(60, 2), // 0.60 = 60%
            max_consecutive_losses: 3,
            min_avg_profit: Decimal::new(1000, 2), // $10.00
        }
    }
}

/// Automation decision based on performance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutomationDecision {
    /// Enable automation for this pattern
    Enable { reason: String },

    /// Disable automation for this pattern
    Disable { reason: String },

    /// Maintain current status
    Maintain { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_performance_creation() {
        let pattern_name = PatternName::new("hammer");
        let perf = PatternPerformance::new(pattern_name.clone());

        assert_eq!(perf.pattern_name, pattern_name);
        assert_eq!(perf.total_trades, 0);
        assert_eq!(perf.total_signals, 0);
        assert!(perf.is_enabled);
    }

    #[test]
    fn test_pattern_performance_record_trade() {
        let mut perf = PatternPerformance::new(PatternName::new("hammer"));

        // Record winning trade
        perf.record_trade(Decimal::new(5000, 2), 3600); // $50.00, 1 hour
        assert_eq!(perf.total_trades, 1);
        assert_eq!(perf.winning_trades, 1);
        assert_eq!(perf.consecutive_wins, 1);
        assert_eq!(perf.consecutive_losses, 0);
        assert_eq!(perf.win_rate, Decimal::ONE);

        // Record losing trade
        perf.record_trade(Decimal::new(-2000, 2), 1800); // -$20.00, 30 min
        assert_eq!(perf.total_trades, 2);
        assert_eq!(perf.losing_trades, 1);
        assert_eq!(perf.consecutive_wins, 0);
        assert_eq!(perf.consecutive_losses, 1);
        assert_eq!(perf.win_rate, Decimal::new(50, 2)); // 50%
    }

    #[test]
    fn test_execution_rate() {
        let mut perf = PatternPerformance::new(PatternName::new("hammer"));

        perf.record_signal();
        perf.record_signal();
        perf.record_signal();
        perf.record_trade(Decimal::new(1000, 2), 3600);

        assert_eq!(perf.execution_rate(), 1.0 / 3.0);
    }

    #[test]
    fn test_performance_thresholds_default() {
        let thresholds = PerformanceThresholds::default();

        assert_eq!(thresholds.min_trades_for_automation, 20);
        assert_eq!(thresholds.min_win_rate, Decimal::new(60, 2));
        assert_eq!(thresholds.max_consecutive_losses, 3);
        assert_eq!(thresholds.min_avg_profit, Decimal::new(1000, 2));
    }
}
