//! Risk management domain types using the newtype pattern for type safety
//!
//! This module provides strongly-typed wrappers for risk-related values
//! to prevent errors like passing a dollar limit where a percentage is expected.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
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
        assert!(Percentage::new(Decimal::new(1500, 2)).is_err()); // 15.00% > 100%

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
        assert_eq!(RiskRuleId::PositionSizeLimit.as_str(), "position_size_limit");
        assert_eq!(RiskRuleId::PdtLimit.as_str(), "pdt_limit");
        assert_eq!(RiskRuleId::PatternSpecificLimit.as_str(), "pattern_specific_limit");
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