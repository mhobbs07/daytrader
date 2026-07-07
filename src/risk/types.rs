//! Risk management domain types using the newtype pattern for type safety
//!
//! This module provides strongly-typed wrappers for risk-related values
//! to prevent errors like passing a dollar limit where a percentage is expected.

use crate::brokers::types::PatternName;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Newtype for dollar amounts (risk limits, balances)
///
/// Ensures dollar amounts are non-negative and uses Decimal for precision.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::DollarAmount;
/// use rust_decimal::Decimal;
///
/// let limit = DollarAmount::new(Decimal::new(50000, 2)).unwrap(); // $500.00
/// assert_eq!(limit.value(), Decimal::new(50000, 2));
///
/// // Negative amounts are rejected
/// assert!(DollarAmount::new(Decimal::new(-100, 0)).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DollarAmount(Decimal);

impl DollarAmount {
    /// Creates a new dollar amount
    ///
    /// # Errors
    ///
    /// Returns an error if the value is negative
    #[inline]
    pub fn new(value: Decimal) -> Result<Self, &'static str> {
        if value < Decimal::ZERO {
            return Err("Dollar amount cannot be negative");
        }
        Ok(Self(value))
    }

    /// Returns the zero dollar amount. Infallible: zero is always
    /// non-negative, so this never fails the way `new` can.
    #[inline]
    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }

    /// Returns the dollar amount value
    #[inline]
    pub fn value(&self) -> Decimal {
        self.0
    }

    /// Returns the amount as f64 (use sparingly, prefer Decimal)
    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0.to_string().parse().unwrap_or(0.0)
    }
}

impl fmt::Display for DollarAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// Newtype for percentage values (0.0 to 100.0)
///
/// Ensures percentages are within valid range for risk calculations.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::Percentage;
///
/// let pct = Percentage::new(Decimal::new(250, 2)).unwrap(); // 2.50%
/// assert_eq!(pct.value(), Decimal::new(250, 2));
///
/// // Out of range values are rejected
/// assert!(Percentage::new(Decimal::new(-10, 0)).is_err());
/// assert!(Percentage::new(Decimal::new(1500, 2)).is_err()); // 15.00% > 100%
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Percentage(Decimal);

impl Percentage {
    /// Creates a new percentage
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not between 0.0 and 100.0 (inclusive)
    #[inline]
    pub fn new(value: Decimal) -> Result<Self, &'static str> {
        if value < Decimal::ZERO || value > Decimal::new(1000, 1) {
            return Err("Percentage must be between 0.0 and 100.0");
        }
        Ok(Self(value))
    }

    /// Returns the zero percentage. Infallible: zero is always within the
    /// valid 0.0..=100.0 range, so this never fails the way `new` can.
    #[inline]
    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }

    /// Returns the percentage value
    #[inline]
    pub fn value(&self) -> Decimal {
        self.0
    }

    /// Returns the percentage as a decimal fraction (0.0 to 1.0)
    #[inline]
    pub fn as_fraction(&self) -> Decimal {
        self.0 / Decimal::new(1000, 1)
    }

    /// Returns the percentage as f64 (use sparingly)
    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0.to_string().parse().unwrap_or(0.0)
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

/// Newtype for count limits (trades, positions, consecutive losses)
///
/// Ensures count values are non-negative integers.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::Count;
///
/// let limit = Count::new(5).unwrap();
/// assert_eq!(limit.value(), 5);
///
/// // Negative counts are rejected
/// assert!(Count::new(-1).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Count(u32);

impl Count {
    /// Creates a new count
    ///
    /// # Errors
    ///
    /// Returns an error if the value is negative (though u32 can't be negative)
    #[inline]
    pub fn new(value: u32) -> Result<Self, &'static str> {
        Ok(Self(value))
    }

    /// Returns the count value
    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for account IDs
///
/// Wraps account identifiers for type safety.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::AccountId;
///
/// let account = AccountId::new("account-123");
/// assert_eq!(account.as_str(), "account-123");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    /// Creates a new account ID
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the account ID as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AccountId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Risk rule identifiers
///
/// Strongly-typed identifiers for different risk rules.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::RiskRuleId;
///
/// let rule_id = RiskRuleId::DailyLossLimit;
/// assert_eq!(rule_id.as_str(), "daily_loss_limit");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskRuleId {
    DailyLossLimit,
    PositionSizeLimit,
    PdtLimit,
    PatternSpecificLimit,
    AccountSafeguard,
}

impl RiskRuleId {
    /// Returns the rule ID as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DailyLossLimit => "daily_loss_limit",
            Self::PositionSizeLimit => "position_size_limit",
            Self::PdtLimit => "pdt_limit",
            Self::PatternSpecificLimit => "pattern_specific_limit",
            Self::AccountSafeguard => "account_safeguard",
        }
    }
}

impl fmt::Display for RiskRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Risk violation severity levels
///
/// Indicates the seriousness of a risk violation.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::ViolationSeverity;
///
/// let severity = ViolationSeverity::Critical;
/// assert!(severity.is_blocking());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ViolationSeverity {
    /// Warning - non-blocking, informational
    Warning,
    /// Moderate violation - may block some actions
    Moderate,
    /// Critical violation - blocks trading, may halt system
    Critical,
}

impl ViolationSeverity {
    /// Returns true if this severity level blocks trading
    #[inline]
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Returns true if this severity level halts trading
    #[inline]
    pub fn halts_trading(&self) -> bool {
        matches!(self, Self::Critical)
    }
}

impl fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Warning => "Warning",
            Self::Moderate => "Moderate",
            Self::Critical => "Critical",
        };
        write!(f, "{}", s)
    }
}

/// Risk evaluation result
///
/// The outcome of evaluating a risk rule against current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskEvaluation {
    /// Rule passed - no violation
    Passed,
    /// Rule violated - includes severity and suggested action
    Violated {
        severity: ViolationSeverity,
        suggested_action: Option<String>,
    },
}

impl RiskEvaluation {
    /// Returns true if the evaluation passed
    #[inline]
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns true if the evaluation resulted in a violation
    #[inline]
    pub fn is_violated(&self) -> bool {
        matches!(self, Self::Violated { .. })
    }

    /// Returns the severity if violated, None if passed
    #[inline]
    pub fn severity(&self) -> Option<&ViolationSeverity> {
        match self {
            Self::Passed => None,
            Self::Violated { severity, .. } => Some(severity),
        }
    }
}

/// Pattern risk profile configuration
///
/// Defines risk parameters specific to each candlestick pattern type.
/// Used to customize risk limits based on pattern characteristics and performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRiskProfile {
    /// Maximum trades allowed per day for this pattern
    pub max_trades_per_day: Count,
    /// Multiplier for position size limits (0.5 = 50% of base limit)
    pub max_position_size_multiplier: f64,
    /// Minimum confidence score required for automated trading
    pub min_confidence_threshold: f64,
    /// Overall risk multiplier for this pattern (0.6 = 60% of base risk limits)
    pub risk_multiplier: f64,
    /// Stop pattern after this many consecutive losses
    pub stop_after_consecutive_losses: Count,
}

impl PatternRiskProfile {
    /// Creates a conservative pattern risk profile
    pub fn conservative() -> Self {
        Self {
            max_trades_per_day: Count::new(2).unwrap(),
            max_position_size_multiplier: 0.5,
            min_confidence_threshold: 0.80,
            risk_multiplier: 0.4,
            stop_after_consecutive_losses: Count::new(2).unwrap(),
        }
    }

    /// Creates a moderate pattern risk profile
    pub fn moderate() -> Self {
        Self {
            max_trades_per_day: Count::new(4).unwrap(),
            max_position_size_multiplier: 0.8,
            min_confidence_threshold: 0.70,
            risk_multiplier: 0.8,
            stop_after_consecutive_losses: Count::new(3).unwrap(),
        }
    }

    /// Creates an aggressive pattern risk profile
    pub fn aggressive() -> Self {
        Self {
            max_trades_per_day: Count::new(5).unwrap(),
            max_position_size_multiplier: 1.0,
            min_confidence_threshold: 0.65,
            risk_multiplier: 1.0,
            stop_after_consecutive_losses: Count::new(3).unwrap(),
        }
    }
}

/// Confidence multiplier for risk adjustments
///
/// Maps confidence score ranges to risk multipliers for dynamic risk adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceMultiplier {
    /// Minimum confidence score for this range (inclusive)
    pub min_confidence: f64,
    /// Maximum confidence score for this range (exclusive)
    pub max_confidence: f64,
    /// Position size multiplier (1.0 = no change)
    pub position_size_multiplier: f64,
    /// Trade limit multiplier (1.0 = no change)
    pub trade_limit_multiplier: f64,
    /// Risk limit multiplier (1.0 = no change)
    pub risk_limit_multiplier: f64,
}

impl ConfidenceMultiplier {
    /// Returns the appropriate multiplier for a given confidence score
    pub fn for_confidence(confidence: f64, multipliers: &[Self]) -> Option<&Self> {
        multipliers
            .iter()
            .find(|m| confidence >= m.min_confidence && confidence < m.max_confidence)
    }
}

/// Pattern performance thresholds
///
/// Defines thresholds for pattern performance monitoring and enable/disable decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternPerformanceThresholds {
    /// Minimum win rate for short-term performance (7-day)
    pub min_win_rate_short: Percentage,
    /// Minimum win rate for medium-term performance (30-day)
    pub min_win_rate_medium: Percentage,
    /// Minimum win rate for long-term performance (90-day)
    pub min_win_rate_long: Percentage,
    /// Maximum consecutive losses before disabling pattern
    pub max_consecutive_losses: Count,
    /// Minimum sample size required for performance evaluation
    pub min_sample_size: Count,
    /// Auto-disable threshold (win rate below this disables pattern)
    pub auto_disable_threshold: Percentage,
}

impl Default for PatternPerformanceThresholds {
    fn default() -> Self {
        Self {
            min_win_rate_short: Percentage::new(Decimal::new(500, 1)).unwrap(), // 50%
            min_win_rate_medium: Percentage::new(Decimal::new(550, 1)).unwrap(), // 55%
            min_win_rate_long: Percentage::new(Decimal::new(600, 1)).unwrap(),  // 60%
            max_consecutive_losses: Count::new(3).unwrap(),
            min_sample_size: Count::new(20).unwrap(),
            auto_disable_threshold: Percentage::new(Decimal::new(400, 1)).unwrap(), // 40%
        }
    }
}

/// Pattern-specific risk violation types
///
/// Extends the general risk violation types with pattern-specific violations.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternRiskViolationType {
    /// Pattern is disabled due to poor performance
    PatternDisabled,
    /// Pattern confidence score is below minimum threshold
    ConfidenceTooLow,
    /// Daily trade limit for this pattern exceeded
    DailyTradeLimitExceeded,
    /// Position size exceeds pattern-specific limits
    PositionSizeExceeded,
    /// Consecutive losses exceeded threshold
    ConsecutiveLossesExceeded,
    /// Pattern performance below required thresholds
    PerformanceThresholdBreached,
}

impl fmt::Display for PatternRiskViolationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::PatternDisabled => "Pattern Disabled",
            Self::ConfidenceTooLow => "Confidence Too Low",
            Self::DailyTradeLimitExceeded => "Daily Trade Limit Exceeded",
            Self::PositionSizeExceeded => "Position Size Exceeded",
            Self::ConsecutiveLossesExceeded => "Consecutive Losses Exceeded",
            Self::PerformanceThresholdBreached => "Performance Threshold Breached",
        };
        write!(f, "{}", s)
    }
}

/// Lightweight daily risk metrics for execution context
///
/// This is the domain-layer representation of daily risk metrics,
/// containing only the fields needed by the execution and strategy layers.
/// For full database-backed metrics, see `db::models::DailyRiskMetrics`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyRiskMetrics {
    /// Realized profit/loss for the day
    pub realized_pl: Decimal,
    /// Unrealized profit/loss for the day
    pub unrealized_pl: Decimal,
    /// Total profit/loss for the day
    pub total_pl: Decimal,
    /// Number of day trades today
    pub day_trade_count: u32,
    /// Whether trading is halted
    pub trading_halted: bool,
}

/// Pattern risk violation information
///
/// Contains details about pattern-specific risk rule violations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternRiskViolation {
    /// Type of pattern risk violation
    pub violation_type: PatternRiskViolationType,
    /// Name of the pattern that caused the violation
    pub pattern_name: PatternName,
    /// Confidence score if applicable
    pub confidence: Option<f64>,
    /// Human-readable message describing the violation
    pub message: String,
    /// Timestamp when the violation occurred
    pub timestamp: DateTime<Utc>,
    /// Severity of the violation
    pub severity: ViolationSeverity,
    /// Suggested action to resolve the violation
    pub suggested_action: Option<String>,
}

/// Direction of a tracked position for day-trade detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    /// Net long (bought to open)
    Long,
    /// Net short (sold to open)
    Short,
    /// No open quantity
    Flat,
}

/// Errors from position lifecycle tracking
#[derive(Debug, thiserror::Error)]
pub enum PositionTrackingError {
    /// Fill quantity must be greater than zero
    #[error("invalid quantity: {quantity} (must be > 0)")]
    InvalidQuantity {
        /// The rejected quantity
        quantity: u64,
    },
    /// Fill price must be positive
    #[error("invalid price: {price} (must be > 0)")]
    InvalidPrice {
        /// The rejected price
        price: Decimal,
    },
}

/// One FIFO lot of an open position, recording when it was opened
///
/// Day-trade detection needs to know whether the quantity being closed was
/// opened on the same trading day, so each opening fill is kept as its own
/// component until it is consumed by closing fills (FIFO).
#[derive(Debug, Clone, PartialEq)]
pub struct PositionComponent {
    /// Remaining open quantity in this lot
    pub quantity: u64,
    /// Fill price the lot was opened at
    pub price: Decimal,
    /// Exact time the lot was opened
    pub opened_at: DateTime<Utc>,
    /// Trading date the lot was opened (UTC date of `opened_at`)
    pub opened_date: chrono::NaiveDate,
}

/// Position lifecycle state for a single symbol, tracked for day-trade detection
///
/// Maintains FIFO components so that closing fills can be matched against the
/// specific lots they close. A close counts as a day trade when any consumed
/// lot was opened on the same UTC date as the closing fill.
///
/// # Examples
///
/// ```
/// use daytrader::risk::types::TrackedPosition;
/// use daytrader::brokers::types::Symbol;
/// use daytrader::events::types::OrderSide;
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let mut position = TrackedPosition::new(Symbol::new("AAPL").unwrap());
/// let now = Utc::now();
/// assert!(!position.update_with_fill(OrderSide::Buy, 100, Decimal::new(15000, 2), now));
/// assert!(position.update_with_fill(OrderSide::Sell, 100, Decimal::new(15100, 2), now));
/// assert!(position.is_flat());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedPosition {
    /// Symbol this position is for
    pub symbol: crate::brokers::types::Symbol,
    /// Current direction of the position
    pub side: PositionSide,
    /// Signed net quantity (positive = long, negative = short, zero = flat)
    pub net_quantity: i64,
    /// Open FIFO lots, oldest first
    pub components: Vec<PositionComponent>,
}

impl TrackedPosition {
    /// Creates an empty (flat) tracked position for `symbol`
    pub fn new(symbol: crate::brokers::types::Symbol) -> Self {
        Self {
            symbol,
            side: PositionSide::Flat,
            net_quantity: 0,
            components: Vec::new(),
        }
    }

    /// Whether the position has no open quantity
    pub fn is_flat(&self) -> bool {
        self.net_quantity == 0
    }

    /// Whether any open lot was opened on `date`
    pub fn has_components_opened_on_date(&self, date: chrono::NaiveDate) -> bool {
        self.components.iter().any(|c| c.opened_date == date)
    }

    /// Applies a fill to the position and reports whether it completed a day trade
    ///
    /// A fill in the position's current direction (or into a flat position)
    /// opens a new FIFO lot and is never a day trade. A fill in the opposite
    /// direction consumes lots oldest-first; it is a day trade when any
    /// consumed lot was opened on the same UTC date as `fill_time`. Quantity
    /// beyond the open lots reverses the position into a new lot on the other
    /// side.
    pub fn update_with_fill(
        &mut self,
        side: crate::events::types::OrderSide,
        quantity: u64,
        price: Decimal,
        fill_time: DateTime<Utc>,
    ) -> bool {
        use crate::events::types::OrderSide;

        let fill_date = fill_time.date_naive();
        let opening_side = match side {
            OrderSide::Buy => PositionSide::Long,
            OrderSide::Sell => PositionSide::Short,
        };

        // Opening or adding: same direction as current position, or flat.
        if self.side == PositionSide::Flat || self.side == opening_side {
            self.components.push(PositionComponent {
                quantity,
                price,
                opened_at: fill_time,
                opened_date: fill_date,
            });
            self.side = opening_side;
            self.net_quantity += match side {
                OrderSide::Buy => quantity as i64,
                OrderSide::Sell => -(quantity as i64),
            };
            return false;
        }

        // Closing (possibly reversing): consume FIFO lots.
        let mut remaining = quantity;
        let mut is_day_trade = false;
        while remaining > 0 {
            let Some(front) = self.components.first_mut() else {
                break;
            };
            if front.opened_date == fill_date {
                is_day_trade = true;
            }
            if front.quantity > remaining {
                front.quantity -= remaining;
                remaining = 0;
            } else {
                remaining -= front.quantity;
                self.components.remove(0);
            }
        }

        self.net_quantity += match side {
            OrderSide::Buy => quantity as i64,
            OrderSide::Sell => -(quantity as i64),
        };

        if self.net_quantity == 0 {
            self.side = PositionSide::Flat;
            self.components.clear();
        } else if remaining > 0 {
            // Over-close reversed the position: the excess opens on the other side.
            self.side = opening_side;
            self.components.push(PositionComponent {
                quantity: remaining,
                price,
                opened_at: fill_time,
                opened_date: fill_date,
            });
        }

        is_day_trade
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_dollar_amount_validation() {
        assert!(DollarAmount::new(Decimal::ZERO).is_ok());
        assert!(DollarAmount::new(Decimal::new(50000, 2)).is_ok());
        assert!(DollarAmount::new(Decimal::new(-100, 0)).is_err());

        let amount = DollarAmount::new(Decimal::new(50000, 2)).unwrap();
        assert_eq!(amount.value(), Decimal::new(50000, 2));
        assert_eq!(format!("{}", amount), "$500.00");
    }

    #[test]
    fn test_percentage_validation() {
        assert!(Percentage::new(Decimal::ZERO).is_ok());
        assert!(Percentage::new(Decimal::new(250, 2)).is_ok()); // 2.50%
        assert!(Percentage::new(Decimal::new(1000, 1)).is_ok()); // 100.0%
        assert!(Percentage::new(Decimal::new(-10, 0)).is_err());
        assert!(Percentage::new(Decimal::new(10001, 2)).is_err()); // 100.01% > 100%

        let pct = Percentage::new(Decimal::new(250, 2)).unwrap();
        assert_eq!(pct.value(), Decimal::new(250, 2));
        assert_eq!(pct.as_fraction(), Decimal::new(25, 3)); // 0.025
        assert_eq!(format!("{}", pct), "2.50%");
    }

    #[test]
    fn test_count_validation() {
        assert!(Count::new(0).is_ok());
        assert!(Count::new(5).is_ok());
        assert!(Count::new(1000).is_ok());

        let count = Count::new(5).unwrap();
        assert_eq!(count.value(), 5);
        assert_eq!(format!("{}", count), "5");
    }

    #[test]
    fn test_account_id() {
        let account = AccountId::new("account-123");
        assert_eq!(account.as_str(), "account-123");
        assert_eq!(format!("{}", account), "account-123");
    }

    #[test]
    fn test_risk_rule_id() {
        assert_eq!(RiskRuleId::DailyLossLimit.as_str(), "daily_loss_limit");
        assert_eq!(
            RiskRuleId::PositionSizeLimit.as_str(),
            "position_size_limit"
        );
        assert_eq!(RiskRuleId::PdtLimit.as_str(), "pdt_limit");
        assert_eq!(
            RiskRuleId::PatternSpecificLimit.as_str(),
            "pattern_specific_limit"
        );
        assert_eq!(RiskRuleId::AccountSafeguard.as_str(), "account_safeguard");
    }

    #[test]
    fn test_violation_severity() {
        assert!(!ViolationSeverity::Warning.is_blocking());
        assert!(!ViolationSeverity::Warning.halts_trading());

        assert!(!ViolationSeverity::Moderate.is_blocking());
        assert!(!ViolationSeverity::Moderate.halts_trading());

        assert!(ViolationSeverity::Critical.is_blocking());
        assert!(ViolationSeverity::Critical.halts_trading());
    }

    #[test]
    fn test_risk_evaluation() {
        let passed = RiskEvaluation::Passed;
        assert!(passed.is_passed());
        assert!(!passed.is_violated());
        assert!(passed.severity().is_none());

        let violated = RiskEvaluation::Violated {
            severity: ViolationSeverity::Critical,
            suggested_action: Some("Halt trading".to_string()),
        };
        assert!(!violated.is_passed());
        assert!(violated.is_violated());
        assert_eq!(violated.severity(), Some(&ViolationSeverity::Critical));
    }
}
