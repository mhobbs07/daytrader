//! Rule evaluation for entry and exit conditions
//!
//! This module provides composable rules for evaluating when to enter or exit trades
//! based on signal characteristics, market conditions, and risk parameters.

use crate::brokers::types::{PatternName, Price, Symbol};
use crate::execution::ExecutionError;
use crate::patterns::signals::TradingSignal;
use crate::positions::tracker::Position;
use crate::risk::types::DailyRiskMetrics;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Rule Evaluation Types
// ============================================================================

/// Result of a rule evaluation
#[derive(Clone, Debug, PartialEq)]
pub enum RuleEvaluation {
    /// Rule passed
    Pass,
    /// Rule failed with reason
    Fail(String),
}

impl RuleEvaluation {
    /// Returns true if the rule passed
    pub fn passed(&self) -> bool {
        matches!(self, RuleEvaluation::Pass)
    }

    /// Returns true if the rule failed
    pub fn failed(&self) -> bool {
        !self.passed()
    }

    /// Gets the failure reason if rule failed
    pub fn reason(&self) -> Option<&str> {
        match self {
            RuleEvaluation::Fail(reason) => Some(reason),
            RuleEvaluation::Pass => None,
        }
    }
}

// ============================================================================
// Entry Rules
// ============================================================================

/// Entry rule trait for evaluating whether to enter a trade
pub trait EntryRule: Send + Sync {
    /// Evaluates the rule against a trading signal
    fn evaluate(&self, signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation;

    /// Returns the rule name for debugging
    fn name(&self) -> &str;
}

/// Context for entry rule evaluation
#[derive(Clone, Debug)]
pub struct EntryContext {
    /// Current account balance
    pub account_balance: Decimal,

    /// Current positions
    pub current_positions: Vec<Position>,

    /// Daily risk metrics
    pub daily_metrics: DailyRiskMetrics,

    /// Pattern-specific statistics
    pub pattern_stats: HashMap<PatternName, PatternStats>,

    /// Current time
    pub current_time: DateTime<Utc>,
}

/// Pattern statistics for rule evaluation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternStats {
    pub win_rate: f64,
    pub total_trades: u32,
    pub consecutive_losses: u32,
}

/// Minimum confidence entry rule
pub struct MinConfidenceRule {
    min_confidence: Decimal,
}

impl MinConfidenceRule {
    pub fn new(min_confidence: Decimal) -> Self {
        Self { min_confidence }
    }
}

impl EntryRule for MinConfidenceRule {
    fn evaluate(&self, signal: &TradingSignal, _context: &EntryContext) -> RuleEvaluation {
        if Decimal::from_f64_retain(signal.confidence.value()).unwrap_or(Decimal::ZERO)
            >= self.min_confidence
        {
            RuleEvaluation::Pass
        } else {
            RuleEvaluation::Fail(format!(
                "Confidence {} below minimum {}",
                signal.confidence.value(),
                self.min_confidence
            ))
        }
    }

    fn name(&self) -> &str {
        "MinConfidence"
    }
}

/// Maximum positions entry rule
pub struct MaxPositionsRule {
    max_positions: usize,
}

impl MaxPositionsRule {
    pub fn new(max_positions: usize) -> Self {
        Self { max_positions }
    }
}

impl EntryRule for MaxPositionsRule {
    fn evaluate(&self, _signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation {
        if context.current_positions.len() < self.max_positions {
            RuleEvaluation::Pass
        } else {
            RuleEvaluation::Fail(format!(
                "Already at maximum positions ({}/{})",
                context.current_positions.len(),
                self.max_positions
            ))
        }
    }

    fn name(&self) -> &str {
        "MaxPositions"
    }
}

/// No duplicate position rule (prevents opening position in same symbol)
pub struct NoDuplicatePositionRule;

impl EntryRule for NoDuplicatePositionRule {
    fn evaluate(&self, signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation {
        let has_position = context
            .current_positions
            .iter()
            .any(|p| p.symbol == signal.symbol);

        if has_position {
            RuleEvaluation::Fail(format!(
                "Already have position in {}",
                signal.symbol.as_str()
            ))
        } else {
            RuleEvaluation::Pass
        }
    }

    fn name(&self) -> &str {
        "NoDuplicatePosition"
    }
}

/// Market hours rule (only trade during specified hours)
pub struct MarketHoursRule {
    start_hour: u32,
    end_hour: u32,
}

impl MarketHoursRule {
    pub fn new(start_hour: u32, end_hour: u32) -> Self {
        Self {
            start_hour,
            end_hour,
        }
    }

    /// Creates rule for standard market hours (9:30 AM - 4:00 PM EST)
    pub fn standard_hours() -> Self {
        Self::new(9, 16) // Simplified to hours only
    }
}

impl EntryRule for MarketHoursRule {
    fn evaluate(&self, _signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation {
        let hour = context.current_time.hour();
        if hour >= self.start_hour && hour < self.end_hour {
            RuleEvaluation::Pass
        } else {
            RuleEvaluation::Fail(format!(
                "Outside market hours (current: {}:00, allowed: {}:00-{}:00)",
                hour, self.start_hour, self.end_hour
            ))
        }
    }

    fn name(&self) -> &str {
        "MarketHours"
    }
}

/// Daily loss limit rule
pub struct DailyLossLimitRule;

impl EntryRule for DailyLossLimitRule {
    fn evaluate(&self, _signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation {
        if context.daily_metrics.trading_halted {
            RuleEvaluation::Fail("Trading halted due to daily loss limit".to_string())
        } else {
            RuleEvaluation::Pass
        }
    }

    fn name(&self) -> &str {
        "DailyLossLimit"
    }
}

/// Pattern performance rule (checks pattern win rate and trade count)
pub struct PatternPerformanceRule {
    min_win_rate: f64,
    min_trades: u32,
}

impl PatternPerformanceRule {
    pub fn new(min_win_rate: f64, min_trades: u32) -> Self {
        Self {
            min_win_rate,
            min_trades,
        }
    }
}

impl EntryRule for PatternPerformanceRule {
    fn evaluate(&self, signal: &TradingSignal, context: &EntryContext) -> RuleEvaluation {
        // Extract pattern name from signal components
        let pattern_name = signal.components.first().and_then(|comp| {
            if matches!(
                comp.component_type,
                crate::patterns::signals::SignalComponentType::Pattern
            ) {
                Some(PatternName::new(&comp.name))
            } else {
                None
            }
        });
        let pattern_name = match &pattern_name {
            Some(name) => name,
            None => return RuleEvaluation::Pass, // No pattern, skip this rule
        };

        if let Some(stats) = context.pattern_stats.get(pattern_name) {
            // Check if we have enough trades to evaluate
            if stats.total_trades < self.min_trades {
                return RuleEvaluation::Pass; // Not enough data yet
            }

            // Check win rate
            if stats.win_rate < self.min_win_rate {
                return RuleEvaluation::Fail(format!(
                    "Pattern {} win rate {:.1}% below minimum {:.1}%",
                    pattern_name.as_str(),
                    stats.win_rate * 100.0,
                    self.min_win_rate * 100.0
                ));
            }

            // Check consecutive losses
            if stats.consecutive_losses >= 3 {
                return RuleEvaluation::Fail(format!(
                    "Pattern {} has {} consecutive losses",
                    pattern_name.as_str(),
                    stats.consecutive_losses
                ));
            }
        }

        RuleEvaluation::Pass
    }

    fn name(&self) -> &str {
        "PatternPerformance"
    }
}

// ============================================================================
// Exit Rules
// ============================================================================

/// Exit rule trait for evaluating whether to exit a position
pub trait ExitRule: Send + Sync {
    /// Evaluates the rule against a position
    fn evaluate(&self, position: &Position, context: &ExitContext) -> RuleEvaluation;

    /// Returns the rule name for debugging
    fn name(&self) -> &str;
}

/// Context for exit rule evaluation
#[derive(Clone, Debug)]
pub struct ExitContext {
    /// Current market price
    pub current_price: Price,

    /// Daily risk metrics
    pub daily_metrics: DailyRiskMetrics,

    /// Current time
    pub current_time: DateTime<Utc>,
}

/// Stop loss exit rule
pub struct StopLossRule {
    stop_loss_percent: Decimal,
}

impl StopLossRule {
    pub fn new(stop_loss_percent: Decimal) -> Self {
        Self { stop_loss_percent }
    }
}

impl ExitRule for StopLossRule {
    fn evaluate(&self, position: &Position, context: &ExitContext) -> RuleEvaluation {
        let loss_percent = if position.avg_entry_price.value() > Decimal::ZERO {
            ((context.current_price.value() - position.avg_entry_price.value())
                / position.avg_entry_price.value())
                * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        if loss_percent < -self.stop_loss_percent {
            RuleEvaluation::Fail(format!("Stop loss triggered: {:.2}% loss", loss_percent))
        } else {
            RuleEvaluation::Pass
        }
    }

    fn name(&self) -> &str {
        "StopLoss"
    }
}

/// Take profit exit rule
pub struct TakeProfitRule {
    take_profit_percent: Decimal,
}

impl TakeProfitRule {
    pub fn new(take_profit_percent: Decimal) -> Self {
        Self {
            take_profit_percent,
        }
    }
}

impl ExitRule for TakeProfitRule {
    fn evaluate(&self, position: &Position, context: &ExitContext) -> RuleEvaluation {
        let profit_percent = if position.avg_entry_price.value() > Decimal::ZERO {
            ((context.current_price.value() - position.avg_entry_price.value())
                / position.avg_entry_price.value())
                * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        if profit_percent >= self.take_profit_percent {
            RuleEvaluation::Fail(format!(
                "Take profit triggered: {:.2}% profit",
                profit_percent
            ))
        } else {
            RuleEvaluation::Pass
        }
    }

    fn name(&self) -> &str {
        "TakeProfit"
    }
}

/// Time-based exit rule (close position after specified duration)
pub struct TimeBasedExitRule {
    max_holding_period_secs: i64,
}

impl TimeBasedExitRule {
    pub fn new(max_holding_period_secs: i64) -> Self {
        Self {
            max_holding_period_secs,
        }
    }

    /// Creates rule for end-of-day exit (close 15 minutes before market close)
    pub fn end_of_day() -> Self {
        Self::new(6 * 3600) // 6 hours (9:30 AM - 3:45 PM)
    }
}

impl ExitRule for TimeBasedExitRule {
    fn evaluate(&self, position: &Position, context: &ExitContext) -> RuleEvaluation {
        let holding_duration = (context.current_time - position.opened_at).num_seconds();

        if holding_duration >= self.max_holding_period_secs {
            RuleEvaluation::Fail(format!(
                "Max holding period exceeded: {} seconds",
                holding_duration
            ))
        } else {
            RuleEvaluation::Pass
        }
    }

    fn name(&self) -> &str {
        "TimeBasedExit"
    }
}

// ============================================================================
// Rule Engine
// ============================================================================

/// Rule engine for evaluating multiple rules
pub struct RuleEngine {
    entry_rules: Vec<Box<dyn EntryRule>>,
    exit_rules: Vec<Box<dyn ExitRule>>,
}

impl RuleEngine {
    /// Creates a new rule engine
    pub fn new() -> Self {
        Self {
            entry_rules: Vec::new(),
            exit_rules: Vec::new(),
        }
    }

    /// Adds an entry rule
    pub fn add_entry_rule(mut self, rule: Box<dyn EntryRule>) -> Self {
        self.entry_rules.push(rule);
        self
    }

    /// Adds an exit rule
    pub fn add_exit_rule(mut self, rule: Box<dyn ExitRule>) -> Self {
        self.exit_rules.push(rule);
        self
    }

    /// Evaluates all entry rules
    pub fn evaluate_entry(
        &self,
        signal: &TradingSignal,
        context: &EntryContext,
    ) -> Result<(), ExecutionError> {
        for rule in &self.entry_rules {
            let evaluation = rule.evaluate(signal, context);
            if evaluation.failed() {
                return Err(ExecutionError::StrategyRejection(format!(
                    "{}: {}",
                    rule.name(),
                    evaluation.reason().unwrap_or("unknown")
                )));
            }
        }
        Ok(())
    }

    /// Evaluates all exit rules (returns true if any rule triggers exit)
    pub fn evaluate_exit(&self, position: &Position, context: &ExitContext) -> bool {
        for rule in &self.exit_rules {
            let evaluation = rule.evaluate(position, context);
            if evaluation.failed() {
                tracing::info!(
                    "Exit rule triggered: {} - {}",
                    rule.name(),
                    evaluation.reason().unwrap_or("unknown")
                );
                return true;
            }
        }
        false
    }

    /// Creates a default conservative rule engine
    pub fn conservative() -> Self {
        Self::new()
            .add_entry_rule(Box::new(MinConfidenceRule::new(Decimal::new(75, 2)))) // 0.75
            .add_entry_rule(Box::new(MaxPositionsRule::new(5)))
            .add_entry_rule(Box::new(NoDuplicatePositionRule))
            .add_entry_rule(Box::new(MarketHoursRule::standard_hours()))
            .add_entry_rule(Box::new(DailyLossLimitRule))
            .add_exit_rule(Box::new(StopLossRule::new(Decimal::new(200, 2)))) // 2%
            .add_exit_rule(Box::new(TakeProfitRule::new(Decimal::new(500, 2)))) // 5%
            .add_exit_rule(Box::new(TimeBasedExitRule::end_of_day()))
    }

    /// Creates a default aggressive rule engine
    pub fn aggressive() -> Self {
        Self::new()
            .add_entry_rule(Box::new(MinConfidenceRule::new(Decimal::new(70, 2)))) // 0.70
            .add_entry_rule(Box::new(MaxPositionsRule::new(10)))
            .add_entry_rule(Box::new(NoDuplicatePositionRule))
            .add_entry_rule(Box::new(MarketHoursRule::standard_hours()))
            .add_entry_rule(Box::new(DailyLossLimitRule))
            .add_exit_rule(Box::new(StopLossRule::new(Decimal::new(300, 2)))) // 3%
            .add_exit_rule(Box::new(TakeProfitRule::new(Decimal::new(800, 2)))) // 8%
            .add_exit_rule(Box::new(TimeBasedExitRule::end_of_day()))
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Confidence, Quantity};

    fn create_test_signal() -> TradingSignal {
        TradingSignal::new(
            Symbol::new("AAPL").unwrap(),
            crate::events::types::SignalType::Buy,
            Confidence::new(0.80).unwrap(),
            Price::new(Decimal::new(15000, 2)).unwrap(),
            crate::patterns::signals::MarketContext::new(
                crate::patterns::types::TrendContext::Sideways { volatility: 0.5 },
            ),
        )
    }

    fn create_test_context() -> EntryContext {
        EntryContext {
            account_balance: Decimal::new(10000000, 2), // $100,000
            current_positions: Vec::new(),
            daily_metrics: DailyRiskMetrics {
                realized_pl: Decimal::ZERO,
                unrealized_pl: Decimal::ZERO,
                total_pl: Decimal::ZERO,
                day_trade_count: 0,
                trading_halted: false,
            },
            pattern_stats: HashMap::new(),
            // Fixed time inside market hours (14:30 UTC = 9:30 ET) so tests pass 24/7
            current_time: chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 1, 2, 14, 30, 0).unwrap(),
        }
    }

    #[test]
    fn test_min_confidence_rule() {
        let rule = MinConfidenceRule::new(Decimal::new(75, 2));
        let signal = create_test_signal(); // 0.80 confidence
        let context = create_test_context();

        let result = rule.evaluate(&signal, &context);
        assert!(result.passed());
    }

    #[test]
    fn test_max_positions_rule() {
        let rule = MaxPositionsRule::new(2);
        let signal = create_test_signal();
        let mut context = create_test_context();

        // Add 2 positions
        context.current_positions.push(Position::new(
            Symbol::new("MSFT").unwrap(),
            crate::positions::tracker::PositionSide::Long,
            Quantity::new(100).unwrap(),
            Price::new(Decimal::new(30000, 2)).unwrap(),
            crate::brokers::types::OrderId::new("order1"),
            None,
            None,
        ));
        context.current_positions.push(Position::new(
            Symbol::new("GOOGL").unwrap(),
            crate::positions::tracker::PositionSide::Long,
            Quantity::new(50).unwrap(),
            Price::new(Decimal::new(140000, 2)).unwrap(),
            crate::brokers::types::OrderId::new("order2"),
            None,
            None,
        ));

        let result = rule.evaluate(&signal, &context);
        assert!(result.failed());
    }

    #[test]
    fn test_no_duplicate_position_rule() {
        let rule = NoDuplicatePositionRule;
        let signal = create_test_signal(); // AAPL
        let mut context = create_test_context();

        // Add existing AAPL position
        context.current_positions.push(Position::new(
            Symbol::new("AAPL").unwrap(),
            crate::positions::tracker::PositionSide::Long,
            Quantity::new(100).unwrap(),
            Price::new(Decimal::new(15000, 2)).unwrap(),
            crate::brokers::types::OrderId::new("order1"),
            None,
            None,
        ));

        let result = rule.evaluate(&signal, &context);
        assert!(result.failed());
    }

    #[test]
    fn test_rule_engine() {
        let engine = RuleEngine::conservative();
        let signal = create_test_signal();
        let context = create_test_context();

        let result = engine.evaluate_entry(&signal, &context);
        assert!(result.is_ok());
    }
}
