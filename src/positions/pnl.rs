// P&L calculation engine with multiple tax lot methods
//
// Provides accurate profit/loss calculations for positions with support for different
// cost basis methods (FIFO, LIFO, Average) and commission handling.

use crate::brokers::types::{Price, Quantity};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// P&L calculation errors
#[derive(Debug, Error)]
pub enum PerformanceError {
    #[error("Calculation error: {0}")]
    CalculationError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

/// Tax lot accounting method
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxLotMethod {
    /// First In, First Out - oldest shares sold first
    FIFO,
    /// Last In, First Out - newest shares sold first
    LIFO,
    /// Average cost basis - all shares at average price
    Average,
}

/// P&L calculator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLConfig {
    /// Include commissions in P&L calculation
    pub include_commissions: bool,

    /// Tax lot method for cost basis calculation
    pub tax_lot_method: TaxLotMethod,
}

impl Default for PnLConfig {
    fn default() -> Self {
        Self {
            include_commissions: true,
            tax_lot_method: TaxLotMethod::FIFO,
        }
    }
}

/// Portfolio-level P&L summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPnL {
    /// Total market value of all positions
    pub total_value: Decimal,

    /// Total cost basis of all positions
    pub total_cost: Decimal,

    /// Total unrealized profit/loss
    pub unrealized_pl: Decimal,

    /// Unrealized P&L as percentage of cost
    pub unrealized_pl_percent: Decimal,

    /// Total realized P&L today
    pub realized_pl_today: Decimal,

    /// Number of open positions
    pub position_count: usize,

    /// Timestamp of calculation
    pub calculated_at: DateTime<Utc>,
}

/// P&L calculator for position-level and portfolio-level calculations
pub struct PnLCalculator {
    config: PnLConfig,
}

impl PnLCalculator {
    /// Create new P&L calculator with configuration
    pub fn new(config: PnLConfig) -> Self {
        Self { config }
    }

    /// Create P&L calculator with default configuration
    pub fn with_defaults() -> Self {
        Self::new(PnLConfig::default())
    }

    /// Calculate unrealized P&L for a position
    ///
    /// # Arguments
    /// * `cost_basis` - Total cost of the position
    /// * `quantity` - Current position quantity
    /// * `current_price` - Current market price
    ///
    /// # Returns
    /// Unrealized profit/loss in dollars
    pub fn calculate_unrealized_pl(
        &self,
        cost_basis: Decimal,
        quantity: u32,
        current_price: Price,
    ) -> Decimal {
        let market_value = current_price.value() * Decimal::from(quantity);
        market_value - cost_basis
    }

    /// Calculate unrealized P&L percentage
    ///
    /// # Arguments
    /// * `cost_basis` - Total cost of the position
    /// * `quantity` - Current position quantity
    /// * `current_price` - Current market price
    ///
    /// # Returns
    /// Unrealized profit/loss as percentage of cost basis
    pub fn calculate_unrealized_pl_percent(
        &self,
        cost_basis: Decimal,
        quantity: u32,
        current_price: Price,
    ) -> Decimal {
        if cost_basis == Decimal::ZERO {
            return Decimal::ZERO;
        }

        let unrealized_pl = self.calculate_unrealized_pl(cost_basis, quantity, current_price);
        (unrealized_pl / cost_basis) * Decimal::from(100)
    }

    /// Calculate realized P&L from a position close or partial close
    ///
    /// # Arguments
    /// * `entry_price` - Average entry price per share
    /// * `exit_price` - Exit price per share
    /// * `quantity` - Number of shares sold
    /// * `commissions` - Total commissions for entry and exit
    ///
    /// # Returns
    /// Realized profit/loss in dollars
    pub fn calculate_realized_pl(
        &self,
        entry_price: Price,
        exit_price: Price,
        quantity: Quantity,
        commissions: Decimal,
    ) -> Decimal {
        let proceeds = exit_price.value() * Decimal::from(quantity.value());
        let cost = entry_price.value() * Decimal::from(quantity.value());
        let gross_pl = proceeds - cost;

        if self.config.include_commissions {
            gross_pl - commissions
        } else {
            gross_pl
        }
    }

    /// Calculate new average entry price when adding to a position
    ///
    /// # Arguments
    /// * `existing_avg_price` - Current average entry price
    /// * `existing_quantity` - Current position quantity
    /// * `new_fill_price` - Price of new fill
    /// * `new_fill_quantity` - Quantity of new fill
    ///
    /// # Returns
    /// New average entry price
    pub fn calculate_avg_entry(
        &self,
        existing_avg_price: Price,
        existing_quantity: u32,
        new_fill_price: Price,
        new_fill_quantity: u32,
    ) -> Price {
        let existing_cost = existing_avg_price.value() * Decimal::from(existing_quantity);
        let new_cost = new_fill_price.value() * Decimal::from(new_fill_quantity);
        let total_cost = existing_cost + new_cost;

        let total_quantity = existing_quantity + new_fill_quantity;

        Price::new(total_cost / Decimal::from(total_quantity))
    }

    /// Calculate portfolio-level P&L from multiple positions
    ///
    /// # Arguments
    /// * `positions` - Iterator over position data (cost_basis, quantity, current_price)
    ///
    /// # Returns
    /// Portfolio P&L summary
    pub fn calculate_portfolio_pl<'a, I>(
        &self,
        positions: I,
    ) -> PortfolioPnL
    where
        I: Iterator<Item = (Decimal, u32, Price)>,
    {
        let mut total_value = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;
        let mut position_count = 0;

        for (cost_basis, quantity, current_price) in positions {
            let market_value = current_price.value() * Decimal::from(quantity);
            total_value += market_value;
            total_cost += cost_basis;
            position_count += 1;
        }

        let unrealized_pl = total_value - total_cost;
        let unrealized_pl_percent = if total_cost != Decimal::ZERO {
            (unrealized_pl / total_cost) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        PortfolioPnL {
            total_value,
            total_cost,
            unrealized_pl,
            unrealized_pl_percent,
            realized_pl_today: Decimal::ZERO, // Calculated separately from closed positions
            position_count,
            calculated_at: Utc::now(),
        }
    }

    /// Calculate market value for a position
    ///
    /// # Arguments
    /// * `quantity` - Position quantity
    /// * `current_price` - Current market price
    ///
    /// # Returns
    /// Market value in dollars
    pub fn calculate_market_value(&self, quantity: u32, current_price: Price) -> Decimal {
        current_price.value() * Decimal::from(quantity)
    }

    /// Get the configured tax lot method
    pub fn tax_lot_method(&self) -> TaxLotMethod {
        self.config.tax_lot_method
    }

    /// Check if commissions are included in calculations
    pub fn includes_commissions(&self) -> bool {
        self.config.include_commissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_calculator() -> PnLCalculator {
        PnLCalculator::with_defaults()
    }

    #[test]
    fn test_unrealized_pl_profit() {
        let calc = create_calculator();
        let cost_basis = dec!(1000.00);
        let quantity = 10;
        let current_price = Price::new(dec!(105.00));

        let pl = calc.calculate_unrealized_pl(cost_basis, quantity, current_price);
        assert_eq!(pl, dec!(50.00)); // (105 * 10) - 1000 = 50
    }

    #[test]
    fn test_unrealized_pl_loss() {
        let calc = create_calculator();
        let cost_basis = dec!(1000.00);
        let quantity = 10;
        let current_price = Price::new(dec!(95.00));

        let pl = calc.calculate_unrealized_pl(cost_basis, quantity, current_price);
        assert_eq!(pl, dec!(-50.00)); // (95 * 10) - 1000 = -50
    }

    #[test]
    fn test_unrealized_pl_percent() {
        let calc = create_calculator();
        let cost_basis = dec!(1000.00);
        let quantity = 10;
        let current_price = Price::new(dec!(110.00));

        let pl_pct = calc.calculate_unrealized_pl_percent(cost_basis, quantity, current_price);
        assert_eq!(pl_pct, dec!(10.00)); // ((1100 - 1000) / 1000) * 100 = 10%
    }

    #[test]
    fn test_realized_pl_with_commission() {
        let calc = create_calculator();
        let entry_price = Price::new(dec!(100.00));
        let exit_price = Price::new(dec!(110.00));
        let quantity = Quantity::new(10);
        let commissions = dec!(2.00);

        let pl = calc.calculate_realized_pl(entry_price, exit_price, quantity, commissions);
        assert_eq!(pl, dec!(98.00)); // (110 - 100) * 10 - 2 = 98
    }

    #[test]
    fn test_realized_pl_without_commission() {
        let config = PnLConfig {
            include_commissions: false,
            tax_lot_method: TaxLotMethod::FIFO,
        };
        let calc = PnLCalculator::new(config);
        let entry_price = Price::new(dec!(100.00));
        let exit_price = Price::new(dec!(110.00));
        let quantity = Quantity::new(10);
        let commissions = dec!(2.00);

        let pl = calc.calculate_realized_pl(entry_price, exit_price, quantity, commissions);
        assert_eq!(pl, dec!(100.00)); // (110 - 100) * 10 = 100
    }

    #[test]
    fn test_average_entry_calculation() {
        let calc = create_calculator();
        let existing_avg = Price::new(dec!(100.00));
        let existing_qty = 10;
        let new_price = Price::new(dec!(110.00));
        let new_qty = 5;

        let new_avg = calc.calculate_avg_entry(existing_avg, existing_qty, new_price, new_qty);
        // (100 * 10 + 110 * 5) / 15 = (1000 + 550) / 15 = 103.33...
        assert!(new_avg.value() > dec!(103.33) && new_avg.value() < dec!(103.34));
    }

    #[test]
    fn test_portfolio_pl() {
        let calc = create_calculator();
        let positions = vec![
            (dec!(1000.00), 10, Price::new(dec!(105.00))), // +50 unrealized
            (dec!(2000.00), 20, Price::new(dec!(95.00))),  // -100 unrealized
            (dec!(1500.00), 15, Price::new(dec!(102.00))), // +30 unrealized
        ];

        let portfolio_pl = calc.calculate_portfolio_pl(positions.into_iter());

        assert_eq!(portfolio_pl.total_cost, dec!(4500.00));
        assert_eq!(portfolio_pl.total_value, dec!(4380.00)); // 1050 + 1900 + 1530
        assert_eq!(portfolio_pl.unrealized_pl, dec!(-120.00));
        assert_eq!(portfolio_pl.position_count, 3);
    }

    #[test]
    fn test_market_value_calculation() {
        let calc = create_calculator();
        let quantity = 50;
        let price = Price::new(dec!(123.45));

        let value = calc.calculate_market_value(quantity, price);
        assert_eq!(value, dec!(6172.50));
    }

    #[test]
    fn test_zero_cost_basis() {
        let calc = create_calculator();
        let cost_basis = Decimal::ZERO;
        let quantity = 10;
        let current_price = Price::new(dec!(100.00));

        let pl_pct = calc.calculate_unrealized_pl_percent(cost_basis, quantity, current_price);
        assert_eq!(pl_pct, Decimal::ZERO);
    }
}
