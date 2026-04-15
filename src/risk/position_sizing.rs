//! Position Size Validation and Portfolio Allocation
//!
//! This module enforces position sizing limits to prevent over-concentration
//! in individual securities and maintain proper portfolio diversification.
//!
//! # Key Features
//!
//! - **Maximum Position Size**: Dollar amount limits per position
//! - **Portfolio Allocation**: Percentage of total equity limits
//! - **Pre-trade Validation**: Check before order execution
//! - **Real-time Monitoring**: Track position sizes during trading
//! - **Dynamic Sizing**: Calculate optimal position sizes based on risk parameters
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::position_sizing::{PositionSizer, PositionSizeLimits};
//! use daytrader::risk::types::{DollarAmount, Percentage};
//! use rust_decimal::Decimal;
//!
//! let limits = PositionSizeLimits {
//!     max_position_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(), // $5,000.00
//!     max_portfolio_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),    // 10.00%
//!     max_concurrent_positions: 5,
//! };
//!
//! let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2)); // $10,000 portfolio
//!
//! // Validate a $1,000 position
//! let can_open = sizer.validate_position_size(Decimal::new(100000, 2)).unwrap();
//! ```

use crate::config::RiskConfig;
use crate::risk::types::{DollarAmount, Percentage, RiskEvaluation, ViolationSeverity};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Position size limits configuration
///
/// Defines the maximum allowable size for individual positions and
/// portfolio allocation constraints.
#[derive(Debug, Clone)]
pub struct PositionSizeLimits {
    /// Maximum dollar amount per individual position
    pub max_position_dollars: DollarAmount,
    /// Maximum percentage of portfolio per position
    pub max_portfolio_percent: Percentage,
    /// Maximum number of concurrent positions
    pub max_concurrent_positions: u32,
}

impl PositionSizeLimits {
    /// Creates limits from RiskConfig
    pub fn from_config(config: &RiskConfig) -> Self {
        Self {
            max_position_dollars: DollarAmount::new(
                Decimal::try_from(config.max_position_size_dollars).unwrap(),
            )
            .unwrap(),
            max_portfolio_percent: Percentage::new(
                Decimal::try_from(config.max_position_size_percent).unwrap(),
            )
            .unwrap(),
            max_concurrent_positions: config.max_concurrent_positions,
        }
    }
}

/// Current position information
///
/// Tracks the size and value of an open position.
#[derive(Debug, Clone)]
pub struct PositionInfo {
    /// Symbol of the position
    pub symbol: String,
    /// Current quantity held
    pub quantity: Decimal,
    /// Current market value
    pub market_value: Decimal,
    /// Average entry price
    pub avg_entry_price: Decimal,
    /// Unrealized P&L
    pub unrealized_pl: Decimal,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl PositionInfo {
    /// Creates a new position info
    pub fn new(
        symbol: String,
        quantity: Decimal,
        avg_entry_price: Decimal,
        current_price: Decimal,
    ) -> Self {
        let market_value = quantity * current_price;
        let cost_basis = quantity * avg_entry_price;
        let unrealized_pl = market_value - cost_basis;

        Self {
            symbol,
            quantity,
            market_value,
            avg_entry_price,
            unrealized_pl,
            last_updated: Utc::now(),
        }
    }

    /// Updates position with new price information
    pub fn update_price(&mut self, current_price: Decimal) {
        self.market_value = self.quantity * current_price;
        let cost_basis = self.quantity * self.avg_entry_price;
        self.unrealized_pl = self.market_value - cost_basis;
        self.last_updated = Utc::now();
    }

    /// Updates position quantity (for partial fills)
    pub fn update_quantity(&mut self, new_quantity: Decimal, new_avg_price: Decimal) {
        self.quantity = new_quantity;
        self.avg_entry_price = new_avg_price;
        // Market value will be updated on next price update
        self.last_updated = Utc::now();
    }
}

/// Portfolio position tracking state
///
/// Maintains current positions and portfolio metrics for risk management.
#[derive(Debug, Clone)]
pub struct PortfolioState {
    /// Total portfolio equity value
    pub total_equity: Decimal,
    /// Current positions by symbol
    pub positions: HashMap<String, PositionInfo>,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl PortfolioState {
    /// Creates a new empty portfolio state
    pub fn new(total_equity: Decimal) -> Self {
        Self {
            total_equity,
            positions: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    /// Adds or updates a position
    pub fn update_position(&mut self, position: PositionInfo) {
        self.positions.insert(position.symbol.clone(), position);
        self.last_updated = Utc::now();
    }

    /// Removes a position
    pub fn remove_position(&mut self, symbol: &str) {
        self.positions.remove(symbol);
        self.last_updated = Utc::now();
    }

    /// Gets a position by symbol
    pub fn get_position(&self, symbol: &str) -> Option<&PositionInfo> {
        self.positions.get(symbol)
    }

    /// Returns the number of open positions
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Calculates total portfolio value including positions
    pub fn total_portfolio_value(&self) -> Decimal {
        self.total_equity
            + self
                .positions
                .values()
                .map(|p| p.market_value)
                .sum::<Decimal>()
    }

    /// Calculates the portfolio allocation percentage for a given value
    pub fn allocation_percentage(&self, position_value: Decimal) -> Percentage {
        if self.total_equity.is_zero() {
            return Percentage::new(Decimal::ZERO).unwrap();
        }
        Percentage::new((position_value / self.total_equity) * Decimal::new(1000, 1)).unwrap()
    }
}

/// Position sizer with thread-safe portfolio tracking
///
/// Provides validation and sizing calculations for position management.
/// Uses RwLock for concurrent access to portfolio state.
pub struct PositionSizer {
    limits: PositionSizeLimits,
    portfolio: Arc<RwLock<PortfolioState>>,
}

impl PositionSizer {
    /// Creates a new position sizer
    pub fn new(limits: PositionSizeLimits, initial_equity: Decimal) -> Self {
        Self {
            limits,
            portfolio: Arc::new(RwLock::new(PortfolioState::new(initial_equity))),
        }
    }

    /// Creates a sizer from RiskConfig
    pub fn from_config(config: &RiskConfig, initial_equity: Decimal) -> Self {
        let limits = PositionSizeLimits::from_config(config);
        Self::new(limits, initial_equity)
    }

    /// Validates position size before opening a new position
    ///
    /// # Arguments
    ///
    /// * `position_value` - The dollar value of the position to validate
    ///
    /// # Returns
    ///
    /// Risk evaluation indicating if the position size is acceptable
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_sizing::{PositionSizer, PositionSizeLimits};
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = PositionSizeLimits {
    ///     max_position_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(),
    ///     max_portfolio_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),
    ///     max_concurrent_positions: 5,
    /// };
    /// let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));
    ///
    /// let evaluation = sizer.validate_position_size(Decimal::new(100000, 2)).await;
    /// # });
    /// ```
    pub async fn validate_position_size(&self, position_value: Decimal) -> RiskEvaluation {
        let portfolio = self.portfolio.read().await;

        // Check dollar limit
        if position_value > self.limits.max_position_dollars.value() {
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Critical,
                suggested_action: Some(format!(
                    "Position size ${:.2} exceeds maximum ${:.2}",
                    position_value,
                    self.limits.max_position_dollars.value()
                )),
            };
        }

        // Check portfolio allocation percentage
        let allocation_pct = portfolio.allocation_percentage(position_value);
        if allocation_pct.value() > self.limits.max_portfolio_percent.value() {
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Moderate,
                suggested_action: Some(format!(
                    "Position allocation {:.2}% exceeds maximum {:.2}%",
                    allocation_pct.value(),
                    self.limits.max_portfolio_percent.value()
                )),
            };
        }

        // Check concurrent position limit
        if portfolio.position_count() >= self.limits.max_concurrent_positions as usize {
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Moderate,
                suggested_action: Some(format!(
                    "Maximum {} concurrent positions exceeded",
                    self.limits.max_concurrent_positions
                )),
            };
        }

        RiskEvaluation::Passed
    }

    /// Checks portfolio allocation limits for existing positions
    ///
    /// # Returns
    ///
    /// Risk evaluation for each position that violates allocation limits
    pub async fn check_portfolio_allocation(&self) -> Vec<(String, RiskEvaluation)> {
        let portfolio = self.portfolio.read().await;
        let mut violations = Vec::new();

        for (symbol, position) in &portfolio.positions {
            let allocation_pct = portfolio.allocation_percentage(position.market_value);

            if allocation_pct.value() > self.limits.max_portfolio_percent.value() {
                let violation = RiskEvaluation::Violated {
                    severity: ViolationSeverity::Warning,
                    suggested_action: Some(format!(
                        "Position {} allocation {:.2}% exceeds maximum {:.2}% - consider reducing",
                        symbol,
                        allocation_pct.value(),
                        self.limits.max_portfolio_percent.value()
                    )),
                };
                violations.push((symbol.clone(), violation));
            }
        }

        violations
    }

    /// Calculates the maximum allowable position size
    ///
    /// # Returns
    ///
    /// The maximum dollar amount that can be allocated to a new position
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_sizing::{PositionSizer, PositionSizeLimits};
    /// # use daytrader::risk::types::{DollarAmount, Percentage};
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let limits = PositionSizeLimits {
    ///     max_position_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(),
    ///     max_portfolio_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),
    ///     max_concurrent_positions: 5,
    /// };
    /// let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));
    ///
    /// let max_size = sizer.calculate_max_position_size().await;
    /// assert_eq!(max_size, Decimal::new(100000, 2)); // 10% of $1,000,000
    /// # });
    /// ```
    pub async fn calculate_max_position_size(&self) -> Decimal {
        let portfolio = self.portfolio.read().await;

        // Calculate based on portfolio percentage limit
        let percent_limit =
            self.limits.max_portfolio_percent.as_fraction() * portfolio.total_equity;

        // Take the minimum of dollar limit and percentage limit
        percent_limit.min(self.limits.max_position_dollars.value())
    }

    /// Calculates optimal position size based on risk parameters and signal confidence
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol for position sizing
    /// * `price` - Current market price
    /// * `confidence` - Signal confidence (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Recommended position size in dollars
    pub async fn calculate_position_size(
        &self,
        symbol: &str,
        price: Decimal,
        confidence: f64,
    ) -> Decimal {
        let max_size = self.calculate_max_position_size().await;

        // Apply confidence-based adjustment
        let confidence_multiplier = if confidence >= 0.8 {
            1.0 // Full size for high confidence
        } else if confidence >= 0.6 {
            0.8 // 80% size for medium confidence
        } else if confidence >= 0.4 {
            0.6 // 60% size for low confidence
        } else {
            0.4 // 40% size for very low confidence
        };

        // Check if we already have a position in this symbol
        let portfolio = self.portfolio.read().await;
        let existing_position = portfolio.get_position(symbol);

        let mut recommended_size =
            max_size * Decimal::from_f64_retain(confidence_multiplier).unwrap_or(Decimal::ONE);

        // Reduce size if we already have a position (avoid over-concentration)
        if existing_position.is_some() {
            recommended_size = recommended_size * Decimal::new(5, 1); // 50% of normal size
        }

        // Ensure we don't exceed position limits
        recommended_size.min(max_size)
    }

    /// Monitors position sizes and returns any violations
    ///
    /// This is typically called periodically or after position updates
    /// to check for violations that may have occurred due to price movements.
    ///
    /// # Returns
    ///
    /// List of symbols with position size violations
    pub async fn monitor_position_sizes(&self) -> Vec<String> {
        let portfolio = self.portfolio.read().await;
        let mut violations = Vec::new();

        for (symbol, position) in &portfolio.positions {
            // Check if position exceeds dollar limit
            if position.market_value > self.limits.max_position_dollars.value() {
                violations.push(symbol.clone());
                continue;
            }

            // Check if position exceeds portfolio allocation
            let allocation_pct = portfolio.allocation_percentage(position.market_value);
            if allocation_pct.value() > self.limits.max_portfolio_percent.value() {
                violations.push(symbol.clone());
            }
        }

        violations
    }

    /// Updates portfolio equity value
    ///
    /// # Arguments
    ///
    /// * `new_equity` - The new total equity value
    pub async fn update_equity(&self, new_equity: Decimal) {
        let mut portfolio = self.portfolio.write().await;
        portfolio.total_equity = new_equity;
        portfolio.last_updated = Utc::now();
    }

    /// Adds or updates a position in the portfolio
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol of the position
    /// * `quantity` - Current quantity
    /// * `avg_entry_price` - Average entry price
    /// * `current_price` - Current market price
    pub async fn update_position(
        &self,
        symbol: String,
        quantity: Decimal,
        avg_entry_price: Decimal,
        current_price: Decimal,
    ) {
        let position = PositionInfo::new(symbol.clone(), quantity, avg_entry_price, current_price);
        let mut portfolio = self.portfolio.write().await;
        portfolio.update_position(position);
    }

    /// Removes a position from the portfolio
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to remove
    pub async fn remove_position(&self, symbol: &str) {
        let mut portfolio = self.portfolio.write().await;
        portfolio.remove_position(symbol);
    }

    /// Gets the current portfolio state (read-only)
    pub async fn get_portfolio_state(&self) -> PortfolioState {
        self.portfolio.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn create_test_limits() -> PositionSizeLimits {
        PositionSizeLimits {
            max_position_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(), // $5,000.00
            max_portfolio_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),    // 10.00%
            max_concurrent_positions: 5,
        }
    }

    #[tokio::test]
    async fn test_position_sizer_creation() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2)); // $10,000 equity

        let portfolio = sizer.get_portfolio_state().await;
        assert_eq!(portfolio.total_equity, Decimal::new(1000000, 2));
        assert_eq!(portfolio.position_count(), 0);
    }

    #[tokio::test]
    async fn test_validate_position_size_under_limits() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // $1,000 position (within 10% limit of $10,000 equity)
        let evaluation = sizer.validate_position_size(Decimal::new(100000, 2)).await;
        assert!(evaluation.is_passed());
    }

    #[tokio::test]
    async fn test_validate_position_size_over_dollar_limit() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // $6,000 position (over $5,000 limit)
        let evaluation = sizer.validate_position_size(Decimal::new(600000, 2)).await;

        match evaluation {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(severity, ViolationSeverity::Critical);
            }
            _ => panic!("Expected violation"),
        }
    }

    #[tokio::test]
    async fn test_validate_position_size_over_percent_limit() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // $2,000 position (20% of $10,000 equity, over 10% limit)
        let evaluation = sizer.validate_position_size(Decimal::new(200000, 2)).await;

        match evaluation {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(severity, ViolationSeverity::Moderate);
            }
            _ => panic!("Expected violation"),
        }
    }

    #[tokio::test]
    async fn test_calculate_max_position_size() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        let max_size = sizer.calculate_max_position_size().await;
        assert_eq!(max_size, Decimal::new(100000, 2)); // 10% of $10,000
    }

    #[tokio::test]
    async fn test_update_and_monitor_positions() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // Add a position
        sizer
            .update_position(
                "AAPL".to_string(),
                Decimal::new(100, 0),   // 100 shares
                Decimal::new(15000, 2), // $150.00 avg entry
                Decimal::new(15500, 2), // $155.00 current
            )
            .await;

        let portfolio = sizer.get_portfolio_state().await;
        assert_eq!(portfolio.position_count(), 1);

        let position = portfolio.get_position("AAPL").unwrap();
        assert_eq!(position.market_value, Decimal::new(15500, 0)); // 100 * 155.00

        // Check for violations (should be none)
        let violations = sizer.monitor_position_sizes().await;
        assert!(violations.is_empty());
    }

    #[tokio::test]
    async fn test_portfolio_allocation_check() {
        let limits = create_test_limits();
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // Add a large position that exceeds allocation limit
        sizer
            .update_position(
                "AAPL".to_string(),
                Decimal::new(200, 0),   // 200 shares
                Decimal::new(15000, 2), // $150.00 avg entry
                Decimal::new(30000, 2), // $300.00 current (price appreciation)
            )
            .await;

        let violations = sizer.check_portfolio_allocation().await;
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].0, "AAPL");

        match &violations[0].1 {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(*severity, ViolationSeverity::Warning);
            }
            _ => panic!("Expected violation"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_position_limit() {
        let limits = PositionSizeLimits {
            max_position_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(),
            max_portfolio_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),
            max_concurrent_positions: 2, // Only 2 positions allowed
        };
        let sizer = PositionSizer::new(limits, Decimal::new(1000000, 2));

        // Add 2 positions
        sizer
            .update_position(
                "AAPL".to_string(),
                Decimal::new(10, 0),
                Decimal::new(15000, 2),
                Decimal::new(15000, 2),
            )
            .await;
        sizer
            .update_position(
                "GOOGL".to_string(),
                Decimal::new(5, 0),
                Decimal::new(20000, 2),
                Decimal::new(20000, 2),
            )
            .await;

        // Try to add a third position
        let evaluation = sizer.validate_position_size(Decimal::new(10000, 2)).await;

        match evaluation {
            RiskEvaluation::Violated { severity, .. } => {
                assert_eq!(severity, ViolationSeverity::Moderate);
            }
            _ => panic!("Expected violation"),
        }
    }
}
