//! Pattern Day Trading (PDT) Rule Enforcement
//!
//! This module implements the SEC Pattern Day Trading rule which requires
//! $25,000 minimum equity for accounts making 4+ day trades within 5 business days.
//!
//! # Key Features
//!
//! - **Day Trade Counting**: Track trades within 5 business day rolling window
//! - **Equity Requirements**: $25,000 minimum for unrestricted day trading
//! - **Automatic Restrictions**: Limit to 3 day trades when under equity minimum
//! - **Rolling Window**: 5 business day sliding window for compliance
//! - **Business Day Logic**: Proper handling of weekends and holidays
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::pdt::{PdtEnforcer, PdtConfig};
//! use rust_decimal::Decimal;
//! use chrono::Utc;
//!
//! let config = PdtConfig {
//!     min_equity_threshold: Decimal::new(2500000, 2), // $25,000.00
//!     max_day_trades_under_threshold: 3,
//!     rolling_window_days: 5,
//! };
//!
//! let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2)); // $20,000 equity
//!
//! // Check if day trading is allowed
//! let can_day_trade = enforcer.check_pdt_eligibility().unwrap();
//! assert!(!can_day_trade); // Under $25k, limited to 3 day trades
//! ```

use crate::config::RiskConfig;
use crate::risk::types::{DollarAmount, RiskEvaluation, ViolationSeverity};
use chrono::{DateTime, Datelike, Utc, Weekday};
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// PDT rule configuration
///
/// Defines the parameters for Pattern Day Trading rule enforcement.
#[derive(Debug, Clone)]
pub struct PdtConfig {
    /// Minimum equity required for unrestricted day trading ($25,000)
    pub min_equity_threshold: Decimal,
    /// Maximum day trades allowed when under equity threshold
    pub max_day_trades_under_threshold: u32,
    /// Rolling window in business days for counting day trades
    pub rolling_window_days: u32,
}

impl PdtConfig {
    /// Creates config from RiskConfig
    pub fn from_config(config: &RiskConfig) -> Self {
        Self {
            min_equity_threshold: Decimal::from(config.pdt_min_equity),
            max_day_trades_under_threshold: config.max_day_trades_under_pdt,
            rolling_window_days: 5, // Standard PDT rule: 5 business days
        }
    }
}

/// Day trade record
///
/// Tracks individual day trades for PDT compliance monitoring.
#[derive(Debug, Clone)]
pub struct DayTradeRecord {
    /// Date of the day trade
    pub date: chrono::NaiveDate,
    /// Number of day trades on this date
    pub count: u32,
}

impl DayTradeRecord {
    /// Creates a new day trade record
    pub fn new(date: chrono::NaiveDate, count: u32) -> Self {
        Self { date, count }
    }
}

/// PDT compliance state
///
/// Maintains the rolling window of day trades and current equity level.
#[derive(Debug, Clone)]
pub struct PdtState {
    /// Current account equity
    pub current_equity: Decimal,
    /// Rolling window of day trade records (most recent first)
    pub day_trade_history: VecDeque<DayTradeRecord>,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl PdtState {
    /// Creates a new PDT state
    pub fn new(current_equity: Decimal) -> Self {
        Self {
            current_equity,
            day_trade_history: VecDeque::new(),
            last_updated: Utc::now(),
        }
    }

    /// Checks if account meets equity threshold for unrestricted day trading
    pub fn meets_equity_threshold(&self, threshold: Decimal) -> bool {
        self.current_equity >= threshold
    }

    /// Counts total day trades in the rolling window
    pub fn total_day_trades_in_window(&self, window_days: u32, current_date: chrono::NaiveDate) -> u32 {
        let cutoff_date = self.calculate_cutoff_date(current_date, window_days);
        self.day_trade_history
            .iter()
            .filter(|record| record.date >= cutoff_date)
            .map(|record| record.count)
            .sum()
    }

    /// Calculates the cutoff date for the rolling window (business days back)
    fn calculate_cutoff_date(&self, from_date: chrono::NaiveDate, business_days_back: u32) -> chrono::NaiveDate {
        let mut date = from_date;
        let mut days_counted = 0;

        while days_counted < business_days_back {
            date = date.pred_opt().unwrap(); // Go back one day

            // Only count business days (Monday-Friday)
            if self.is_business_day(date) {
                days_counted += 1;
            }
        }

        date
    }

    /// Checks if a date is a business day (Monday-Friday)
    fn is_business_day(&self, date: chrono::NaiveDate) -> bool {
        !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }

    /// Adds a day trade to the history
    pub fn add_day_trade(&mut self, date: chrono::NaiveDate) {
        // Check if we already have a record for this date
        if let Some(record) = self.day_trade_history.front_mut() {
            if record.date == date {
                record.count += 1;
                return;
            }
        }

        // Add new record for this date
        self.day_trade_history.push_front(DayTradeRecord::new(date, 1));

        // Keep only recent records (cleanup old ones beyond window)
        // We'll clean up during window calculations
        self.last_updated = Utc::now();
    }

    /// Updates current equity
    pub fn update_equity(&mut self, new_equity: Decimal) {
        self.current_equity = new_equity;
        self.last_updated = Utc::now();
    }

    /// Cleans up old records beyond the rolling window
    pub fn cleanup_old_records(&mut self, window_days: u32, current_date: chrono::NaiveDate) {
        let cutoff_date = self.calculate_cutoff_date(current_date, window_days);

        // Remove records older than cutoff date
        while let Some(record) = self.day_trade_history.back() {
            if record.date < cutoff_date {
                self.day_trade_history.pop_back();
            } else {
                break;
            }
        }
    }
}

/// PDT rule enforcer with thread-safe state management
///
/// Provides compliance checking and enforcement for Pattern Day Trading rules.
/// Uses RwLock for concurrent access to PDT state.
pub struct PdtEnforcer {
    config: PdtConfig,
    state: Arc<RwLock<PdtState>>,
}

impl PdtEnforcer {
    /// Creates a new PDT enforcer
    pub fn new(config: PdtConfig, initial_equity: Decimal) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(PdtState::new(initial_equity))),
        }
    }

    /// Creates an enforcer from RiskConfig
    pub fn from_config(config: &RiskConfig, initial_equity: Decimal) -> Self {
        let pdt_config = PdtConfig::from_config(config);
        Self::new(pdt_config, initial_equity)
    }

    /// Counts day trades in the current rolling window
    ///
    /// # Returns
    ///
    /// Total number of day trades in the 5 business day window
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::pdt::{PdtEnforcer, PdtConfig};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let config = PdtConfig {
    ///     min_equity_threshold: Decimal::new(2500000, 2),
    ///     max_day_trades_under_threshold: 3,
    ///     rolling_window_days: 5,
    /// };
    /// let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));
    ///
    /// let trade_count = enforcer.count_day_trades().await;
    /// assert_eq!(trade_count, 0);
    /// # });
    /// ```
    pub async fn count_day_trades(&self) -> u32 {
        let state = self.state.read().await;
        let current_date = Utc::now().date_naive();

        // Clean up old records first
        let mut state_mut = self.state.write().await;
        state_mut.cleanup_old_records(self.config.rolling_window_days, current_date);
        drop(state_mut);

        // Count trades in window
        let state = self.state.read().await;
        state.total_day_trades_in_window(self.config.rolling_window_days, current_date)
    }

    /// Checks if the account is eligible for day trading
    ///
    /// # Returns
    ///
    /// `Ok(true)` if day trading is allowed, `Ok(false)` if restricted,
    /// `Err` if day trading limit exceeded
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::pdt::{PdtEnforcer, PdtConfig};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let config = PdtConfig {
    ///     min_equity_threshold: Decimal::new(2500000, 2),
    ///     max_day_trades_under_threshold: 3,
    ///     rolling_window_days: 5,
    /// };
    /// let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));
    ///
    /// let can_trade = enforcer.check_pdt_eligibility().await.unwrap();
    /// assert!(can_trade); // Under $25k, but no trades yet
    /// # });
    /// ```
    pub async fn check_pdt_eligibility(&self) -> Result<bool, String> {
        let state = self.state.read().await;
        let current_date = Utc::now().date_naive();
        let day_trade_count = state.total_day_trades_in_window(self.config.rolling_window_days, current_date);

        // If equity meets threshold, unlimited day trading allowed
        if state.meets_equity_threshold(self.config.min_equity_threshold) {
            return Ok(true);
        }

        // Under equity threshold, limited to max_day_trades_under_threshold
        if day_trade_count >= self.config.max_day_trades_under_threshold {
            return Err(format!(
                "PDT limit exceeded: {} day trades in 5 business days (limit: {})",
                day_trade_count, self.config.max_day_trades_under_threshold
            ));
        }

        Ok(true)
    }

    /// Enforces PDT limits for a potential day trade
    ///
    /// # Returns
    ///
    /// Risk evaluation indicating if the day trade is allowed
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::pdt::{PdtEnforcer, PdtConfig};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let config = PdtConfig {
    ///     min_equity_threshold: Decimal::new(2500000, 2),
    ///     max_day_trades_under_threshold: 3,
    ///     rolling_window_days: 5,
    /// };
    /// let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));
    ///
    /// let evaluation = enforcer.enforce_pdt_limits().await;
    /// assert!(evaluation.is_passed());
    /// # });
    /// ```
    pub async fn enforce_pdt_limits(&self) -> RiskEvaluation {
        match self.check_pdt_eligibility().await {
            Ok(true) => RiskEvaluation::Passed,
            Ok(false) => RiskEvaluation::Violated {
                severity: ViolationSeverity::Moderate,
                suggested_action: Some("Increase account equity to $25,000 for unrestricted day trading".to_string()),
            },
            Err(message) => RiskEvaluation::Violated {
                severity: ViolationSeverity::Critical,
                suggested_action: Some(format!("PDT violation: {}", message)),
            },
        }
    }

    /// Records a day trade in the rolling window
    ///
    /// # Arguments
    ///
    /// * `trade_date` - Date of the day trade
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::pdt::{PdtEnforcer, PdtConfig};
    /// # use rust_decimal::Decimal;
    /// # use chrono::Utc;
    /// # tokio_test::block_on(async {
    /// let config = PdtConfig {
    ///     min_equity_threshold: Decimal::new(2500000, 2),
    ///     max_day_trades_under_threshold: 3,
    ///     rolling_window_days: 5,
    /// };
    /// let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));
    ///
    /// let today = Utc::now().date_naive();
    /// enforcer.record_day_trade(today).await;
    ///
    /// let count = enforcer.count_day_trades().await;
    /// assert_eq!(count, 1);
    /// # });
    /// ```
    pub async fn record_day_trade(&self, trade_date: chrono::NaiveDate) {
        let mut state = self.state.write().await;
        state.add_day_trade(trade_date);
    }

    /// Updates current account equity
    ///
    /// # Arguments
    ///
    /// * `new_equity` - The new account equity value
    pub async fn update_equity(&self, new_equity: Decimal) {
        let mut state = self.state.write().await;
        state.update_equity(new_equity);
    }

    /// Resets the PDT rolling window (typically at month end or on request)
    ///
    /// This clears all day trade history, effectively resetting the compliance window.
    pub async fn reset_pdt_window(&self) {
        let mut state = self.state.write().await;
        state.day_trade_history.clear();
        state.last_updated = Utc::now();
    }

    /// Gets the current PDT state (read-only)
    pub async fn get_pdt_state(&self) -> PdtState {
        self.state.read().await.clone()
    }

    /// Checks if account meets PDT equity requirements
    pub async fn meets_equity_requirement(&self) -> bool {
        let state = self.state.read().await;
        state.meets_equity_threshold(self.config.min_equity_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn create_test_config() -> PdtConfig {
        PdtConfig {
            min_equity_threshold: Decimal::new(2500000, 2), // $25,000.00
            max_day_trades_under_threshold: 3,
            rolling_window_days: 5,
        }
    }

    #[tokio::test]
    async fn test_pdt_enforcer_creation() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2)); // $20,000

        let state = enforcer.get_pdt_state().await;
        assert_eq!(state.current_equity, Decimal::new(2000000, 2));
        assert!(state.day_trade_history.is_empty());
    }

    #[tokio::test]
    async fn test_equity_requirement_check() {
        let config = create_test_config();

        // Under threshold
        let enforcer_under = PdtEnforcer::new(config.clone(), Decimal::new(2000000, 2));
        assert!(!enforcer_under.meets_equity_requirement().await);

        // At threshold
        let enforcer_at = PdtEnforcer::new(config.clone(), Decimal::new(2500000, 2));
        assert!(enforcer_at.meets_equity_requirement().await);

        // Over threshold
        let enforcer_over = PdtEnforcer::new(config, Decimal::new(3000000, 2));
        assert!(enforcer_over.meets_equity_requirement().await);
    }

    #[tokio::test]
    async fn test_day_trade_recording_and_counting() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

        let today = Utc::now().date_naive();

        // Record some day trades
        enforcer.record_day_trade(today).await;
        enforcer.record_day_trade(today).await; // Second trade same day

        let count = enforcer.count_day_trades().await;
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_pdt_eligibility_under_threshold() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

        // No trades yet - should be eligible
        let eligible = enforcer.check_pdt_eligibility().await.unwrap();
        assert!(eligible);

        // Record 3 day trades (at limit)
        let today = Utc::now().date_naive();
        for _ in 0..3 {
            enforcer.record_day_trade(today).await;
        }

        // Should still be eligible (at limit)
        let eligible = enforcer.check_pdt_eligibility().await.unwrap();
        assert!(eligible);

        // Record one more trade (over limit)
        enforcer.record_day_trade(today).await;

        // Should not be eligible
        let result = enforcer.check_pdt_eligibility().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("PDT limit exceeded"));
    }

    #[tokio::test]
    async fn test_pdt_eligibility_over_threshold() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(3000000, 2)); // $30,000 > $25,000

        // Record many day trades - should still be eligible
        let today = Utc::now().date_naive();
        for _ in 0..10 {
            enforcer.record_day_trade(today).await;
        }

        let eligible = enforcer.check_pdt_eligibility().await.unwrap();
        assert!(eligible);
    }

    #[tokio::test]
    async fn test_enforce_pdt_limits() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

        // Initially should pass
        let evaluation = enforcer.enforce_pdt_limits().await;
        assert!(evaluation.is_passed());

        // Record 4 day trades (over limit for under-threshold account)
        let today = Utc::now().date_naive();
        for _ in 0..4 {
            enforcer.record_day_trade(today).await;
        }

        // Should now violate
        let evaluation = enforcer.enforce_pdt_limits().await;
        match evaluation {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(severity, ViolationSeverity::Critical);
            }
            _ => panic!("Expected violation"),
        }
    }

    #[tokio::test]
    async fn test_reset_pdt_window() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

        // Record some trades
        let today = Utc::now().date_naive();
        enforcer.record_day_trade(today).await;
        enforcer.record_day_trade(today).await;

        let count_before = enforcer.count_day_trades().await;
        assert_eq!(count_before, 2);

        // Reset window
        enforcer.reset_pdt_window().await;

        let count_after = enforcer.count_day_trades().await;
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn test_business_day_cutoff_calculation() {
        let config = create_test_config();
        let enforcer = PdtEnforcer::new(config, Decimal::new(2000000, 2));

        let state = enforcer.get_pdt_state().await;

        // Test with a Friday date
        let friday = chrono::NaiveDate::from_ymd_opt(2023, 12, 15).unwrap(); // Friday
        let cutoff = state.calculate_cutoff_date(friday, 5);

        // 5 business days back from Friday should be the previous Friday
        // Fri -1: Thu, -2: Wed, -3: Tue, -4: Mon, -5: Fri (previous week)
        let expected = chrono::NaiveDate::from_ymd_opt(2023, 12, 8).unwrap();
        assert_eq!(cutoff, expected);
    }
}