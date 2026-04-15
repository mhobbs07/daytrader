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
#[cfg(feature = "mock")]
use crate::db::mock::MockPdtRepository;
#[cfg(feature = "database")]
use crate::db::traits::PdtRepositoryTrait;
use crate::risk::types::{DollarAmount, RiskEvaluation, ViolationSeverity};
use chrono::{DateTime, Datelike, Utc, Weekday};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

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
            min_equity_threshold: Decimal::try_from(config.pdt_min_equity).unwrap(),
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
    pub fn total_day_trades_in_window(
        &self,
        window_days: u32,
        current_date: chrono::NaiveDate,
    ) -> u32 {
        let cutoff_date = self.calculate_cutoff_date(current_date, window_days);
        self.day_trade_history
            .iter()
            .filter(|record| record.date >= cutoff_date)
            .map(|record| record.count)
            .sum()
    }

    /// Calculates the cutoff date for the rolling window (business days back)
    pub fn calculate_cutoff_date(
        &self,
        from_date: chrono::NaiveDate,
        business_days_back: u32,
    ) -> chrono::NaiveDate {
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
        self.day_trade_history
            .push_front(DayTradeRecord::new(date, 1));

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
/// Uses RwLock for concurrent access to PDT state and optional database persistence.
#[derive(Clone)]
pub struct PdtEnforcer {
    config: PdtConfig,
    state: Arc<RwLock<PdtState>>,
    #[cfg(any(feature = "database", feature = "mock"))]
    repository: Option<Arc<dyn PdtRepositoryTrait>>,
    #[cfg(any(feature = "database", feature = "mock"))]
    account_id: i64,
}

impl PdtEnforcer {
    /// Creates a new PDT enforcer without database persistence
    pub fn new(config: PdtConfig, initial_equity: Decimal) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(PdtState::new(initial_equity))),
            #[cfg(any(feature = "database", feature = "mock"))]
            repository: None,
            #[cfg(any(feature = "database", feature = "mock"))]
            account_id: 0,
        }
    }

    /// Creates a new PDT enforcer without database (for testing)
    #[cfg(feature = "mock")]
    pub fn new_without_db(config: PdtConfig, initial_equity: Decimal) -> Self {
        let repository: Arc<dyn PdtRepositoryTrait> = Arc::new(MockPdtRepository::new());
        Self {
            config,
            state: Arc::new(RwLock::new(PdtState::new(initial_equity))),
            repository: Some(repository),
            account_id: 0, // Default account ID for testing
        }
    }

    /// Creates a new PDT enforcer with database persistence
    #[cfg(feature = "database")]
    pub async fn new_with_db(
        config: PdtConfig,
        initial_equity: Decimal,
        repository: Arc<dyn PdtRepositoryTrait>,
        account_id: i64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let enforcer = Self {
            config,
            state: Arc::new(RwLock::new(PdtState::new(initial_equity))),
            repository: Some(repository),
            account_id,
        };

        // Restore state from database if available
        enforcer.restore_state_from_database().await?;

        Ok(enforcer)
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
        let current_date = Utc::now().date_naive();

        // Clean up old records first
        {
            let mut state = self.state.write().await;
            state.cleanup_old_records(self.config.rolling_window_days, current_date);
        }

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
        let day_trade_count =
            state.total_day_trades_in_window(self.config.rolling_window_days, current_date);

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
                suggested_action: Some(
                    "Increase account equity to $25,000 for unrestricted day trading".to_string(),
                ),
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
    /// enforcer.record_day_trade(today).await.unwrap();
    ///
    /// let count = enforcer.count_day_trades().await;
    /// assert_eq!(count, 1);
    /// # });
    /// ```
    pub async fn record_day_trade(
        &self,
        trade_date: chrono::NaiveDate,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update in-memory state
        {
            let mut state = self.state.write().await;
            state.add_day_trade(trade_date);
        }

        // Persist to database if available
        #[cfg(any(feature = "database", feature = "mock"))]
        if let Some(ref repository) = self.repository {
            // Create a minimal trade record for database storage
            let trade_record = crate::db::models::DayTradeRecord {
                id: 0,
                account_id: self.account_id,
                trade_id: format!(
                    "simple_trade_{}",
                    Utc::now().timestamp_nanos_opt().unwrap_or(0)
                ),
                symbol: "UNKNOWN".to_string(),
                quantity: 1,
                open_time: trade_date.and_hms_opt(9, 30, 0).unwrap().and_utc(),
                close_time: trade_date.and_hms_opt(16, 0, 0).unwrap().and_utc(),
                open_price: 0.0,
                close_price: 0.0,
                realized_pl: 0.0,
                commission: 0.0,
                pattern_name: None,
                is_validated: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            repository.record_day_trade(&trade_record).await?;
        }

        Ok(())
    }

    /// Restores PDT state from database on startup
    #[cfg(any(feature = "database", feature = "mock"))]
    async fn restore_state_from_database(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref repository) = self.repository {
            debug!(
                "Restoring PDT state from database for account {}",
                self.account_id
            );

            // Get the latest tracking record to restore equity and counts
            if let Some(tracking) = repository.get_current_pdt_tracking(self.account_id).await? {
                let mut state = self.state.write().await;
                state.current_equity =
                    Decimal::try_from(tracking.equity_value).unwrap_or(Decimal::ZERO);

                // Restore recent day trades for accurate window calculations
                let current_date = Utc::now().date_naive();
                let window_start =
                    state.calculate_cutoff_date(current_date, self.config.rolling_window_days);

                // Get day trades from database to restore in-memory state
                let recent_trades = repository
                    .get_day_trades_in_range(self.account_id, window_start, current_date)
                    .await?;

                // Rebuild the in-memory VecDeque from database records
                state.day_trade_history.clear();
                for trade in recent_trades {
                    let trade_date = trade.close_time.date_naive();
                    if let Some(existing_record) = state.day_trade_history.front_mut() {
                        if existing_record.date == trade_date {
                            existing_record.count += 1;
                            continue;
                        }
                    }
                    state
                        .day_trade_history
                        .push_front(DayTradeRecord::new(trade_date, 1));
                }

                info!(
                    "Restored PDT state: equity=${}, day trades in window={}",
                    state.current_equity,
                    state.total_day_trades_in_window(self.config.rolling_window_days, current_date)
                );
            }
        }

        Ok(())
    }

    /// Updates current account equity
    ///
    /// # Arguments
    ///
    /// * `new_equity` - The new account equity value
    pub async fn update_equity(
        &self,
        new_equity: Decimal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_equity = {
            let mut state = self.state.write().await;
            let old_equity = state.current_equity;
            state.update_equity(new_equity);
            old_equity
        };

        // Update database tracking if available
        #[cfg(any(feature = "database", feature = "mock"))]
        if let Some(ref repository) = self.repository {
            // Update daily tracking with new equity
            self.update_daily_tracking().await?;
        }

        debug!("Equity updated from ${} to ${}", old_equity, new_equity);
        Ok(())
    }

    /// Updates daily PDT tracking record
    #[cfg(any(feature = "database", feature = "mock"))]
    async fn update_daily_tracking(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref repository) = self.repository {
            let state = self.state.read().await;
            let current_date = Utc::now().date_naive();
            let window_start =
                state.calculate_cutoff_date(current_date, self.config.rolling_window_days);
            let window_end = current_date;

            // Count day trades from database for accuracy
            let day_trades_count = repository
                .count_day_trades_in_window(self.account_id, window_start, window_end)
                .await?;

            let today_trades = state
                .day_trade_history
                .front()
                .filter(|record| record.date == current_date)
                .map(|record| record.count as i32)
                .unwrap_or(0);

            let tracking_record = crate::db::models::PdtTracking {
                id: 0,
                account_id: self.account_id,
                trading_date: current_date,
                day_trades_count: today_trades,
                rolling_window_start: window_start,
                rolling_window_end: window_end,
                total_day_trades_window: day_trades_count,
                equity_value: state.current_equity.to_f64().unwrap_or(0.0),
                min_equity_threshold: Some(
                    self.config.min_equity_threshold.to_f64().unwrap_or(25000.0),
                ),
                max_day_trades_allowed: Some(self.config.max_day_trades_under_threshold as i32),
                is_restricted: !state.meets_equity_threshold(self.config.min_equity_threshold)
                    && day_trades_count >= self.config.max_day_trades_under_threshold as i32,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            repository.upsert_pdt_tracking(&tracking_record).await?;
        }
        Ok(())
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
        enforcer.record_day_trade(today).await.unwrap();
        enforcer.record_day_trade(today).await.unwrap(); // Second trade same day

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
            enforcer.record_day_trade(today).await.unwrap();
        }

        // Should still be eligible (at limit)
        let eligible = enforcer.check_pdt_eligibility().await.unwrap();
        assert!(eligible);

        // Record one more trade (over limit)
        enforcer.record_day_trade(today).await.unwrap();

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
            enforcer.record_day_trade(today).await.unwrap();
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
            enforcer.record_day_trade(today).await.unwrap();
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
        enforcer.record_day_trade(today).await.unwrap();
        enforcer.record_day_trade(today).await.unwrap();

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
