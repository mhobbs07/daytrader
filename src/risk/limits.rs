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
use chrono::{DateTime, NaiveTime, Timelike, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    /// Creates limits from RiskConfig
    pub fn from_config(config: &RiskConfig, starting_balance: Decimal) -> Self {
        Self {
            max_loss_dollars: DollarAmount::new(
                Decimal::try_from(config.max_daily_loss_dollars).unwrap(),
            )
            .unwrap(),
            max_loss_percent: Percentage::new(
                Decimal::try_from(config.max_daily_loss_percent).unwrap(),
            )
            .unwrap(),
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
        let current_loss = self.total_loss().abs();
        current_loss >= limits.max_loss_dollars.value()
    }

    /// Checks if the current loss exceeds percentage limits
    pub fn exceeds_percent_limit(&self, limits: &DailyLossLimits) -> bool {
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

    /// Creates a tracker from RiskConfig
    pub fn from_config(config: &RiskConfig, starting_balance: Decimal) -> Self {
        let limits = DailyLossLimits::from_config(config, starting_balance);
        let mut state = DailyPnlState::new(starting_balance);
        state.starting_balance = starting_balance;

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

        // Check against dollar limit
        if projected_loss >= self.limits.max_loss_dollars.value() {
            return Ok(false);
        }

        // Check against percentage limit
        let percent_limit = self
            .limits
            .dollar_limit_from_percent(state.starting_balance);
        if projected_loss >= percent_limit {
            return Ok(false);
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

    /// Checks if it's time for daily reset (9:30 AM ET)
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current time to check
    ///
    /// # Returns
    ///
    /// `true` if it's time for daily reset
    pub async fn should_reset_daily(&self, current_time: DateTime<Utc>) -> bool {
        // Market open is 9:30 AM ET
        let market_open_et = NaiveTime::from_hms_opt(9, 30, 0).unwrap();

        // Convert current time to ET (assuming EST, UTC-5; in production, handle DST)
        let current_et = current_time.naive_utc() - chrono::Duration::hours(5);
        let current_naive_et = current_et.time();
        let current_date = current_time.date_naive();

        let state = self.state.read().await;
        let last_reset_date = state.trading_date;

        // Reset if current date is after last reset date and time is at or after market open
        current_date > last_reset_date && current_naive_et >= market_open_et
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
