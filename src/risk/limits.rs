//! Daily Loss Limits and Tracking
//!
//! This module implements daily P&L monitoring and loss limits to prevent
//! catastrophic losses. It tracks both realized and unrealized P&L and enforces
//! configurable dollar and percentage limits.
//!
//! # Key Features
//!
//! - **Dual Limits**: Both dollar amount and percentage-based loss limits
//! - **Real-time Tracking**: Updates from order fills and position changes
//! - **Automatic Halt**: Trading suspension when limits are exceeded
//! - **Market Open Reset**: Daily reset logic at 9:30 AM ET
//! - **Type Safety**: Uses newtype wrappers for precision and safety
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::limits::{DailyLossTracker, DailyLossLimits};
//! use daytrader::risk::types::{DollarAmount, Percentage};
//! use rust_decimal::Decimal;
//!
//! let limits = DailyLossLimits {
//!     max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(), // $500.00
//!     max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),     // 2.00%
//! };
//!
//! let mut tracker = DailyLossTracker::new(limits);
//!
//! // Check if a $100 loss would violate limits
//! let can_trade = tracker.check_daily_loss_limit(Decimal::new(-100, 0)).unwrap();
//! ```

use crate::config::RiskConfig;
use crate::risk::types::{DollarAmount, Percentage, RiskEvaluation, ViolationSeverity};
use crate::utils::market_hours;
use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::Decimal;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::error;

/// Errors constructing [`DailyLossLimits`] from a [`RiskConfig`].
///
/// `RiskConfig` values are deserialized from user-editable config files as
/// `f64`, so they can carry `NaN`, infinities, or out-of-range values that
/// have no valid `Decimal` or domain-newtype representation. This error
/// makes those cases explicit instead of panicking at startup.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DailyLossLimitsError {
    /// `max_daily_loss_dollars` was NaN, infinite, or otherwise not
    /// representable as a `Decimal`.
    #[error("max_daily_loss_dollars ({0}) is not a finite, representable decimal value")]
    InvalidMaxLossDollars(String),

    /// `max_daily_loss_dollars` converted to a `Decimal` but failed newtype
    /// validation (e.g. negative).
    #[error("max_daily_loss_dollars is invalid: {0}")]
    MaxLossDollarsOutOfRange(&'static str),

    /// `max_daily_loss_percent` was NaN, infinite, or otherwise not
    /// representable as a `Decimal`.
    #[error("max_daily_loss_percent ({0}) is not a finite, representable decimal value")]
    InvalidMaxLossPercent(String),

    /// `max_daily_loss_percent` converted to a `Decimal` but failed newtype
    /// validation (e.g. negative or over 100%).
    #[error("max_daily_loss_percent is invalid: {0}")]
    MaxLossPercentOutOfRange(&'static str),
}

/// Daily loss limits configuration
///
/// Defines the maximum allowable losses for a trading day.
/// Both dollar and percentage limits are checked independently.
#[derive(Debug, Clone)]
pub struct DailyLossLimits {
    /// Maximum loss in dollar amount
    pub max_loss_dollars: DollarAmount,
    /// Maximum loss as percentage of starting balance
    pub max_loss_percent: Percentage,
}

impl DailyLossLimits {
    /// Creates limits from `RiskConfig`, validating the config-supplied
    /// `f64` values rather than panicking on `NaN`/infinite/negative input.
    ///
    /// # Errors
    ///
    /// Returns [`DailyLossLimitsError`] if either `max_daily_loss_dollars`
    /// or `max_daily_loss_percent` is NaN, infinite, negative, or (for the
    /// percentage) greater than 100.
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::config::RiskConfig;
    /// use daytrader::risk::limits::DailyLossLimits;
    ///
    /// let mut config = RiskConfig::default();
    /// config.max_daily_loss_dollars = 500.0;
    /// config.max_daily_loss_percent = 2.0;
    /// let limits = DailyLossLimits::try_from_config(&config).unwrap();
    /// assert_eq!(limits.max_loss_dollars.value().to_string(), "500");
    ///
    /// // NaN is rejected instead of panicking.
    /// config.max_daily_loss_dollars = f64::NAN;
    /// assert!(DailyLossLimits::try_from_config(&config).is_err());
    /// ```
    pub fn try_from_config(config: &RiskConfig) -> Result<Self, DailyLossLimitsError> {
        let max_loss_dollars_decimal =
            Decimal::try_from(config.max_daily_loss_dollars).map_err(|_| {
                DailyLossLimitsError::InvalidMaxLossDollars(
                    config.max_daily_loss_dollars.to_string(),
                )
            })?;
        let max_loss_dollars = DollarAmount::new(max_loss_dollars_decimal)
            .map_err(DailyLossLimitsError::MaxLossDollarsOutOfRange)?;

        let max_loss_percent_decimal =
            Decimal::try_from(config.max_daily_loss_percent).map_err(|_| {
                DailyLossLimitsError::InvalidMaxLossPercent(
                    config.max_daily_loss_percent.to_string(),
                )
            })?;
        let max_loss_percent = Percentage::new(max_loss_percent_decimal)
            .map_err(DailyLossLimitsError::MaxLossPercentOutOfRange)?;

        Ok(Self {
            max_loss_dollars,
            max_loss_percent,
        })
    }

    /// Creates limits from RiskConfig.
    ///
    /// This is a non-panicking, infallible wrapper around
    /// [`Self::try_from_config`] kept for callers that cannot propagate a
    /// `Result` (constructors that must return `Self`). If the config is
    /// invalid, this logs the error and falls back to maximally
    /// conservative limits (zero — which callers must treat as "halt
    /// immediately", not "unlimited"); pair with
    /// [`DailyLossTracker::from_config`], which does exactly that by
    /// starting the tracker in a halted state on invalid config.
    ///
    /// Prefer [`Self::try_from_config`] in new code so invalid config is a
    /// startup error rather than a silent fallback.
    pub fn from_config(config: &RiskConfig, _starting_balance: Decimal) -> Self {
        match Self::try_from_config(config) {
            Ok(limits) => limits,
            Err(err) => {
                error!(
                    error = %err,
                    "invalid daily loss limits config; falling back to zero limits \
                     (caller must halt trading rather than treat this as unlimited)"
                );
                Self {
                    max_loss_dollars: DollarAmount::zero(),
                    max_loss_percent: Percentage::zero(),
                }
            }
        }
    }

    /// Calculates the dollar limit based on starting balance and percentage
    pub fn dollar_limit_from_percent(&self, starting_balance: Decimal) -> Decimal {
        starting_balance * self.max_loss_percent.as_fraction()
    }
}

/// Daily P&L tracking state
///
/// Maintains current P&L position and trading date for limit enforcement.
#[derive(Debug, Clone)]
pub struct DailyPnlState {
    /// Current trading date
    pub trading_date: chrono::NaiveDate,
    /// Starting account balance for the day
    pub starting_balance: Decimal,
    /// Current realized P&L
    pub realized_pl: Decimal,
    /// Current unrealized P&L
    pub unrealized_pl: Decimal,
    /// Whether trading is halted due to loss limits
    pub is_halted: bool,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl DailyPnlState {
    /// Creates a new daily P&L state
    pub fn new(starting_balance: Decimal) -> Self {
        Self {
            trading_date: Utc::now().date_naive(),
            starting_balance,
            realized_pl: Decimal::ZERO,
            unrealized_pl: Decimal::ZERO,
            is_halted: false,
            last_updated: Utc::now(),
        }
    }

    /// Returns total P&L (realized + unrealized)
    #[inline]
    pub fn total_pl(&self) -> Decimal {
        self.realized_pl + self.unrealized_pl
    }

    /// Returns total loss (negative P&L)
    #[inline]
    pub fn total_loss(&self) -> Decimal {
        self.total_pl().min(Decimal::ZERO)
    }

    /// Checks if the current loss exceeds dollar limits
    pub fn exceeds_dollar_limit(&self, limits: &DailyLossLimits) -> bool {
        let max = limits.max_loss_dollars.value();
        if max.is_zero() {
            return false;
        }
        let current_loss = self.total_loss().abs();
        current_loss >= max
    }

    /// Checks if the current loss exceeds percentage limits
    pub fn exceeds_percent_limit(&self, limits: &DailyLossLimits) -> bool {
        // No percentage check possible until starting_balance is set
        if self.starting_balance.is_zero() {
            return false;
        }
        let current_loss = self.total_loss().abs();
        let percent_limit = limits.dollar_limit_from_percent(self.starting_balance);
        current_loss >= percent_limit
    }

    /// Checks if any loss limits are exceeded
    pub fn exceeds_limits(&self, limits: &DailyLossLimits) -> bool {
        self.exceeds_dollar_limit(limits) || self.exceeds_percent_limit(limits)
    }
}

/// Daily loss tracker with thread-safe state management
///
/// Provides async methods for P&L tracking and limit enforcement.
/// Uses RwLock for concurrent read access and exclusive write access.
#[derive(Clone)]
pub struct DailyLossTracker {
    limits: DailyLossLimits,
    state: Arc<RwLock<DailyPnlState>>,
}

impl DailyLossTracker {
    /// Creates a new daily loss tracker
    pub fn new(limits: DailyLossLimits) -> Self {
        let starting_balance = Decimal::ZERO; // Will be set on first update
        Self {
            limits,
            state: Arc::new(RwLock::new(DailyPnlState::new(starting_balance))),
        }
    }

    /// Creates a tracker from RiskConfig.
    ///
    /// This constructor cannot propagate a `Result` — callers (e.g.
    /// `RiskEngine::new`) require an infallible `Self`. If the config's
    /// `max_daily_loss_dollars`/`max_daily_loss_percent` are invalid (NaN,
    /// infinite, or negative), this fails safe: the returned tracker starts
    /// **already halted**, so `check_daily_loss_limit` and
    /// `should_halt_trading` immediately block trading instead of silently
    /// running with a wrong (zero, i.e. "unlimited") loss limit. The error
    /// is logged via `tracing::error!` for operators to catch at startup.
    ///
    /// Prefer [`DailyLossLimits::try_from_config`] plus [`Self::new`] in new
    /// code so invalid config is a startup error instead of a runtime halt.
    pub fn from_config(config: &RiskConfig, starting_balance: Decimal) -> Self {
        let (limits, is_halted) = match DailyLossLimits::try_from_config(config) {
            Ok(limits) => (limits, false),
            Err(err) => {
                error!(
                    error = %err,
                    "invalid daily loss limits config; starting daily loss tracker \
                     halted rather than trading with an incorrect limit"
                );
                (
                    DailyLossLimits {
                        max_loss_dollars: DollarAmount::zero(),
                        max_loss_percent: Percentage::zero(),
                    },
                    true,
                )
            }
        };

        let mut state = DailyPnlState::new(starting_balance);
        state.starting_balance = starting_balance;
        state.is_halted = is_halted;

        Self {
            limits,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Validates a potential P&L change against daily loss limits
    ///
    /// # Arguments
    ///
    /// * `pl_change` - The P&L change to validate (positive for profit, negative for loss)
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the change would not violate limits,
    /// `Ok(false)` if it would violate limits,
    /// `Err` if trading is halted or other error
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::limits::DailyLossTracker;
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = DailyLossLimits {
    ///     max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
    ///     max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    /// };
    /// let tracker = DailyLossTracker::new(limits);
    ///
    /// let can_trade = tracker.check_daily_loss_limit(Decimal::new(-100, 0)).await.unwrap();
    /// # });
    /// ```
    pub async fn check_daily_loss_limit(&self, pl_change: Decimal) -> Result<bool, String> {
        let state = self.state.read().await;

        if state.is_halted {
            return Err("Trading is halted due to daily loss limits".to_string());
        }

        // Calculate projected P&L after this change
        let projected_pl = state.total_pl() + pl_change;
        let projected_loss = projected_pl.min(Decimal::ZERO).abs();

        // Check against dollar limit (zero = unlimited, a no-op)
        let max_dollars = self.limits.max_loss_dollars.value();
        if !max_dollars.is_zero() && projected_loss >= max_dollars {
            return Ok(false);
        }

        // Check against percentage limit (skip when starting_balance is zero —
        // percent limit would be zero, causing any loss to violate)
        if !state.starting_balance.is_zero() {
            let percent_limit = self
                .limits
                .dollar_limit_from_percent(state.starting_balance);
            if projected_loss >= percent_limit {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Updates daily P&L from realized gains/losses
    ///
    /// # Arguments
    ///
    /// * `realized_pl` - Realized P&L change (positive for profit, negative for loss)
    ///
    /// # Returns
    ///
    /// Risk evaluation indicating if limits were violated
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::limits::DailyLossTracker;
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = DailyLossLimits {
    ///     max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
    ///     max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    /// };
    /// let tracker = DailyLossTracker::new(limits);
    ///
    /// let evaluation = tracker.update_daily_pnl(Decimal::new(-2500, 2)).await;
    /// # });
    /// ```
    pub async fn update_daily_pnl(&self, realized_pl: Decimal) -> RiskEvaluation {
        let mut state = self.state.write().await;

        // Update realized P&L
        state.realized_pl += realized_pl;
        state.last_updated = Utc::now();

        // Check if limits are now exceeded
        if state.exceeds_limits(&self.limits) {
            state.is_halted = true;
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Critical,
                suggested_action: Some("Halt all trading for the day".to_string()),
            };
        }

        RiskEvaluation::Passed
    }

    /// Updates unrealized P&L from position changes
    ///
    /// # Arguments
    ///
    /// * `unrealized_pl_change` - Change in unrealized P&L
    ///
    /// # Returns
    ///
    /// Risk evaluation indicating if limits were violated
    pub async fn update_unrealized_pnl(&self, unrealized_pl_change: Decimal) -> RiskEvaluation {
        let mut state = self.state.write().await;

        // Update unrealized P&L
        state.unrealized_pl += unrealized_pl_change;
        state.last_updated = Utc::now();

        // Check if limits are now exceeded
        if state.exceeds_limits(&self.limits) {
            state.is_halted = true;
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Critical,
                suggested_action: Some("Halt all trading for the day".to_string()),
            };
        }

        RiskEvaluation::Passed
    }

    /// Determines if trading should be halted due to loss limits
    ///
    /// # Returns
    ///
    /// `true` if trading should be halted, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::limits::DailyLossTracker;
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = DailyLossLimits {
    ///     max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
    ///     max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    /// };
    /// let tracker = DailyLossTracker::new(limits);
    ///
    /// let should_halt = tracker.should_halt_trading().await;
    /// assert!(!should_halt);
    /// # });
    /// ```
    pub async fn should_halt_trading(&self) -> bool {
        let state = self.state.read().await;
        state.is_halted || state.exceeds_limits(&self.limits)
    }

    /// Resets daily limits at market open (9:30 AM ET)
    ///
    /// Should be called at the start of each trading day to reset P&L tracking.
    ///
    /// # Arguments
    ///
    /// * `new_starting_balance` - The account balance at market open
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::limits::DailyLossTracker;
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = DailyLossLimits {
    ///     max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(),
    ///     max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),
    /// };
    /// let tracker = DailyLossTracker::new(limits);
    ///
    /// tracker.reset_daily_limits(Decimal::new(1000000, 2)).await;
    /// # });
    /// ```
    pub async fn reset_daily_limits(&self, new_starting_balance: Decimal) {
        let mut state = self.state.write().await;

        // Reset for new trading day
        state.trading_date = Utc::now().date_naive();
        state.starting_balance = new_starting_balance;
        state.realized_pl = Decimal::ZERO;
        state.unrealized_pl = Decimal::ZERO;
        state.is_halted = false;
        state.last_updated = Utc::now();
    }

    /// Gets the current daily P&L state (read-only)
    pub async fn get_daily_state(&self) -> DailyPnlState {
        self.state.read().await.clone()
    }

    /// Checks if it's time for daily reset (9:30 AM ET, DST-correct)
    ///
    /// Uses [`market_hours::market_open_utc`] (America/New_York via the IANA
    /// tz database) rather than a fixed UTC offset, so the check is correct
    /// year-round instead of drifting by an hour for roughly half the year.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current time to check
    ///
    /// # Returns
    ///
    /// `true` if it's time for daily reset. Returns `false` (does not reset)
    /// if the market-open boundary cannot be computed for the current
    /// Eastern-time calendar date — this should not happen for any real
    /// calendar date, but a failed computation must never be silently
    /// treated as "always reset".
    pub async fn should_reset_daily(&self, current_time: DateTime<Utc>) -> bool {
        let et_date = current_time.with_timezone(&New_York).date_naive();

        let market_open = match market_hours::market_open_utc(et_date) {
            Ok(open) => open,
            Err(err) => {
                error!(
                    error = %err,
                    date = %et_date,
                    "failed to compute market open for daily reset check; skipping reset"
                );
                return false;
            }
        };

        let state = self.state.read().await;
        let last_reset_date = state.trading_date;

        // Reset if the ET trading date has advanced past the last reset date
        // and the current instant is at or after that date's market open.
        et_date > last_reset_date && current_time >= market_open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::Decimal;

    fn create_test_limits() -> DailyLossLimits {
        DailyLossLimits {
            max_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(), // $500.00
            max_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),     // 2.00%
        }
    }

    #[tokio::test]
    async fn test_daily_loss_tracker_creation() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        let state = tracker.get_daily_state().await;
        assert_eq!(state.realized_pl, Decimal::ZERO);
        assert_eq!(state.unrealized_pl, Decimal::ZERO);
        assert!(!state.is_halted);
    }

    #[tokio::test]
    async fn test_check_daily_loss_limit_under_limit() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Small loss should be allowed
        let can_trade = tracker
            .check_daily_loss_limit(Decimal::new(-1000, 2))
            .await
            .unwrap();
        assert!(can_trade);
    }

    #[tokio::test]
    async fn test_check_daily_loss_limit_over_limit() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Large loss should be blocked
        let can_trade = tracker
            .check_daily_loss_limit(Decimal::new(-60000, 2))
            .await
            .unwrap();
        assert!(!can_trade);
    }

    #[tokio::test]
    async fn test_update_daily_pnl_violation() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Add a large loss that exceeds limits
        let evaluation = tracker.update_daily_pnl(Decimal::new(-60000, 2)).await;

        match evaluation {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(severity, ViolationSeverity::Critical);
            }
            _ => panic!("Expected violation"),
        }

        // Should now be halted
        assert!(tracker.should_halt_trading().await);
    }

    #[tokio::test]
    async fn test_reset_daily_limits() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Add some P&L
        tracker.update_daily_pnl(Decimal::new(-10000, 2)).await;

        // Reset with new balance
        let new_balance = Decimal::new(2000000, 2); // $20,000.00
        tracker.reset_daily_limits(new_balance).await;

        let state = tracker.get_daily_state().await;
        assert_eq!(state.starting_balance, new_balance);
        assert_eq!(state.realized_pl, Decimal::ZERO);
        assert_eq!(state.unrealized_pl, Decimal::ZERO);
        assert!(!state.is_halted);
    }

    #[tokio::test]
    async fn test_percentage_limit_calculation() {
        let starting_balance = Decimal::new(1000000, 2); // $10,000.00
        let limits = DailyLossLimits {
            max_loss_dollars: DollarAmount::new(Decimal::new(100000, 2)).unwrap(), // $1,000.00
            max_loss_percent: Percentage::new(Decimal::new(100, 1)).unwrap(),      // 10.0%
        };

        let percent_limit = limits.dollar_limit_from_percent(starting_balance);
        assert_eq!(percent_limit, Decimal::new(100000, 2)); // $1,000.00 (10% of $10,000)
    }

    #[tokio::test]
    async fn test_halted_trading_blocks_new_trades() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Manually halt trading
        {
            let mut state = tracker.state.write().await;
            state.is_halted = true;
        }

        // Should not allow new trades
        let result = tracker.check_daily_loss_limit(Decimal::new(-100, 0)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Trading is halted"));
    }

    fn valid_risk_config() -> RiskConfig {
        RiskConfig {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            ..RiskConfig::default()
        }
    }

    #[test]
    fn test_try_from_config_valid_values_succeeds() {
        let config = valid_risk_config();
        let limits = DailyLossLimits::try_from_config(&config).unwrap();
        assert_eq!(limits.max_loss_dollars.value(), Decimal::new(500, 0));
        assert_eq!(limits.max_loss_percent.value(), Decimal::new(2, 0));
    }

    #[test]
    fn test_try_from_config_nan_dollars_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = f64::NAN;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::InvalidMaxLossDollars(_))
        ));
    }

    #[test]
    fn test_try_from_config_infinite_dollars_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = f64::INFINITY;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::InvalidMaxLossDollars(_))
        ));
    }

    #[test]
    fn test_try_from_config_negative_infinite_dollars_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = f64::NEG_INFINITY;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::InvalidMaxLossDollars(_))
        ));
    }

    #[test]
    fn test_try_from_config_negative_dollars_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = -100.0;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::MaxLossDollarsOutOfRange(_))
        ));
    }

    #[test]
    fn test_try_from_config_nan_percent_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_percent = f64::NAN;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::InvalidMaxLossPercent(_))
        ));
    }

    #[test]
    fn test_try_from_config_infinite_percent_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_percent = f64::INFINITY;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::InvalidMaxLossPercent(_))
        ));
    }

    #[test]
    fn test_try_from_config_negative_percent_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_percent = -5.0;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::MaxLossPercentOutOfRange(_))
        ));
    }

    #[test]
    fn test_try_from_config_percent_over_100_returns_error() {
        let mut config = valid_risk_config();
        config.max_daily_loss_percent = 150.0;
        let result = DailyLossLimits::try_from_config(&config);
        assert!(matches!(
            result,
            Err(DailyLossLimitsError::MaxLossPercentOutOfRange(_))
        ));
    }

    #[test]
    fn test_from_config_nan_does_not_panic_and_returns_zero_limits() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = f64::NAN;
        // Must not panic; falls back to zero (paired with a halted tracker
        // in `DailyLossTracker::from_config`, see below).
        let limits = DailyLossLimits::from_config(&config, Decimal::ZERO);
        assert_eq!(limits.max_loss_dollars.value(), Decimal::ZERO);
        assert_eq!(limits.max_loss_percent.value(), Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_daily_loss_tracker_from_config_valid_starts_unhalted() {
        let config = valid_risk_config();
        let tracker = DailyLossTracker::from_config(&config, Decimal::new(1000000, 2));

        let state = tracker.get_daily_state().await;
        assert!(!state.is_halted);
    }

    #[tokio::test]
    async fn test_daily_loss_tracker_from_config_nan_starts_halted() {
        let mut config = valid_risk_config();
        config.max_daily_loss_dollars = f64::NAN;

        // Must not panic, and must fail safe: trading halted immediately
        // rather than silently running with a zero ("unlimited") limit.
        let tracker = DailyLossTracker::from_config(&config, Decimal::new(1000000, 2));
        let state = tracker.get_daily_state().await;
        assert!(state.is_halted);

        let result = tracker.check_daily_loss_limit(Decimal::new(-100, 0)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_daily_loss_tracker_from_config_infinite_percent_starts_halted() {
        let mut config = valid_risk_config();
        config.max_daily_loss_percent = f64::INFINITY;

        let tracker = DailyLossTracker::from_config(&config, Decimal::new(1000000, 2));
        let state = tracker.get_daily_state().await;
        assert!(state.is_halted);
    }

    #[tokio::test]
    async fn test_should_reset_daily_winter_boundary_minutes() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        // Force last reset date behind the check date so only the time
        // boundary is under test.
        {
            let mut state = tracker.state.write().await;
            state.trading_date = chrono::NaiveDate::from_ymd_opt(2024, 1, 14).unwrap();
        }

        // 9:29 ET (14:29 UTC) on Jan 16, 2024 (EST, UTC-5) — before open.
        let before_open = Utc.with_ymd_and_hms(2024, 1, 16, 14, 29, 0).unwrap();
        assert!(!tracker.should_reset_daily(before_open).await);

        // 9:30 ET (14:30 UTC) — at open, should reset.
        let at_open = Utc.with_ymd_and_hms(2024, 1, 16, 14, 30, 0).unwrap();
        assert!(tracker.should_reset_daily(at_open).await);
    }

    #[tokio::test]
    async fn test_should_reset_daily_summer_boundary_minutes() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        {
            let mut state = tracker.state.write().await;
            state.trading_date = chrono::NaiveDate::from_ymd_opt(2024, 7, 14).unwrap();
        }

        // 9:29 ET (13:29 UTC) on Jul 16, 2024 (EDT, UTC-4) — before open.
        let before_open = Utc.with_ymd_and_hms(2024, 7, 16, 13, 29, 0).unwrap();
        assert!(!tracker.should_reset_daily(before_open).await);

        // 9:30 ET (13:30 UTC) — at open, should reset.
        let at_open = Utc.with_ymd_and_hms(2024, 7, 16, 13, 30, 0).unwrap();
        assert!(tracker.should_reset_daily(at_open).await);
    }

    #[tokio::test]
    async fn test_should_reset_daily_same_trading_date_does_not_reset() {
        let limits = create_test_limits();
        let tracker = DailyLossTracker::new(limits);

        let today_et = chrono::NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        {
            let mut state = tracker.state.write().await;
            state.trading_date = today_et;
        }

        // Even well after market open on the *same* ET date, no reset is due.
        let later_same_day = Utc.with_ymd_and_hms(2024, 1, 16, 18, 0, 0).unwrap();
        assert!(!tracker.should_reset_daily(later_same_day).await);
    }
}
