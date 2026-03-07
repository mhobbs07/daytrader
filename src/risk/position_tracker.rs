//! Position Tracker - Thread-safe position lifecycle tracking for day trade detection
//!
//! This module provides position tracking capabilities to accurately determine when
//! trades constitute day trades (same-day open and close). It maintains position
//! lifecycle information using FIFO (First In, First Out) methodology.
//!
//! # Key Features
//!
//! - **Thread-safe**: Uses Arc<RwLock> for concurrent access
//! - **FIFO tracking**: Accurately tracks position components opened at different times
//! - **Day trade detection**: Identifies same-day position opens and closes
//! - **Memory efficient**: Periodic cleanup of old position data
//! - **Performance optimized**: Fast lookups using HashMap storage
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::position_tracker::PositionTracker;
//! use daytrader::brokers::types::Symbol;
//! use daytrader::events::types::{OrderSide, Fill};
//! use rust_decimal::Decimal;
//! use chrono::Utc;
//!
//! # tokio_test::block_on(async {
//! let tracker = PositionTracker::new();
//! let symbol = Symbol::new("AAPL").unwrap();
//!
//! // Buy to open position
//! let is_day_trade = tracker.process_fill(
//!     &symbol,
//!     OrderSide::Buy,
//!     100,
//!     Decimal::new(15000, 2),
//!     Utc::now(),
//! ).await.unwrap();
//! assert!(!is_day_trade); // Opening position is not a day trade
//!
//! // Sell same day to close position
//! let is_day_trade = tracker.process_fill(
//!     &symbol,
//!     OrderSide::Sell,
//!     50,
//!     Decimal::new(15100, 2),
//!     Utc::now(),
//! ).await.unwrap();
//! assert!(is_day_trade); // Closing position same day = day trade
//! # });
//! ```

use crate::brokers::types::Symbol;
use crate::events::types::OrderSide;
use crate::risk::types::{PositionTrackingError, TrackedPosition};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Maximum age in days for position components before cleanup
const MAX_POSITION_COMPONENT_AGE_DAYS: i64 = 30;

/// Position tracker with thread-safe concurrent access
///
/// Maintains a mapping of symbols to their tracked positions, providing
/// day trade detection capabilities through position lifecycle monitoring.
///
/// # Thread Safety
///
/// All methods are async and use RwLock for safe concurrent access.
/// Multiple readers can access position data simultaneously, while
/// position updates require exclusive write access.
///
/// # Memory Management
///
/// Positions are automatically cleaned up after [`MAX_POSITION_COMPONENT_AGE_DAYS`] days
/// to prevent unbounded memory growth. Only active positions and recent history
/// are retained for day trade detection.
#[derive(Debug, Clone)]
pub struct PositionTracker {
    /// Thread-safe storage of tracked positions by symbol
    positions: Arc<RwLock<HashMap<String, TrackedPosition>>>,
}

impl PositionTracker {
    /// Creates a new position tracker
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::risk::position_tracker::PositionTracker;
    ///
    /// let tracker = PositionTracker::new();
    /// ```
    pub fn new() -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Processes a trade fill and returns whether it constitutes a day trade
    ///
    /// This is the primary interface for day trade detection. It updates the
    /// position tracking state and returns true if the fill results in a day trade
    /// (position opened and closed on the same trading day).
    ///
    /// # Arguments
    ///
    /// * `symbol` - The security symbol being traded
    /// * `side` - Whether this is a buy or sell order
    /// * `quantity` - Number of shares filled
    /// * `price` - Price per share of the fill
    /// * `fill_time` - When the fill occurred
    ///
    /// # Returns
    ///
    /// `Ok(true)` if this fill constitutes a day trade, `Ok(false)` otherwise,
    /// or `Err` if there was an error processing the fill.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # use daytrader::brokers::types::Symbol;
    /// # use daytrader::events::types::OrderSide;
    /// # use rust_decimal::Decimal;
    /// # use chrono::Utc;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    /// let symbol = Symbol::new("AAPL").unwrap();
    ///
    /// // Open position
    /// let is_day_trade = tracker.process_fill(
    ///     &symbol,
    ///     OrderSide::Buy,
    ///     100,
    ///     Decimal::new(15000, 2),
    ///     Utc::now(),
    /// ).await.unwrap();
    /// assert!(!is_day_trade);
    ///
    /// // Close position same day
    /// let is_day_trade = tracker.process_fill(
    ///     &symbol,
    ///     OrderSide::Sell,
    ///     100,
    ///     Decimal::new(15100, 2),
    ///     Utc::now(),
    /// ).await.unwrap();
    /// assert!(is_day_trade);
    /// # });
    /// ```
    pub async fn process_fill(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        quantity: u64,
        price: Decimal,
        fill_time: DateTime<Utc>,
    ) -> Result<bool, PositionTrackingError> {
        self.validate_inputs(quantity, price)?;

        let mut positions = self.positions.write().await;
        let symbol_str = symbol.as_str().to_string();

        // Get or create position for this symbol
        let position = positions
            .entry(symbol_str.clone())
            .or_insert_with(|| TrackedPosition::new(symbol.clone()));

        // Update position with fill and get day trade result
        let is_day_trade = position.update_with_fill(side.clone(), quantity, price, fill_time);

        tracing::debug!(
            symbol = %symbol,
            side = ?side,
            quantity = quantity,
            price = %price,
            is_day_trade = is_day_trade,
            net_quantity = position.net_quantity,
            "Processed trade fill"
        );

        Ok(is_day_trade)
    }

    /// Gets the current position for a symbol (read-only)
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to lookup
    ///
    /// # Returns
    ///
    /// `Some(TrackedPosition)` if a position exists, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # use daytrader::brokers::types::Symbol;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    /// let symbol = Symbol::new("AAPL").unwrap();
    ///
    /// let position = tracker.get_position(&symbol).await;
    /// assert!(position.is_none()); // No position initially
    /// # });
    /// ```
    pub async fn get_position(&self, symbol: &Symbol) -> Option<TrackedPosition> {
        let positions = self.positions.read().await;
        positions.get(symbol.as_str()).cloned()
    }

    /// Gets all tracked positions (read-only)
    ///
    /// Returns a snapshot of all currently tracked positions.
    ///
    /// # Returns
    ///
    /// A HashMap mapping symbol strings to their tracked positions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    ///
    /// let all_positions = tracker.get_all_positions().await;
    /// assert!(all_positions.is_empty()); // No positions initially
    /// # });
    /// ```
    pub async fn get_all_positions(&self) -> HashMap<String, TrackedPosition> {
        self.positions.read().await.clone()
    }

    /// Checks if a symbol has any position components opened on a specific date
    ///
    /// This is useful for day trade analysis when you need to know if any
    /// portion of a position was established on a given trading day.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to check
    /// * `date` - The date to check for opened components
    ///
    /// # Returns
    ///
    /// `Ok(true)` if position has components opened on the date, `Ok(false)` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # use daytrader::brokers::types::Symbol;
    /// # use daytrader::events::types::OrderSide;
    /// # use rust_decimal::Decimal;
    /// # use chrono::Utc;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    /// let symbol = Symbol::new("AAPL").unwrap();
    /// let today = Utc::now().date_naive();
    ///
    /// // Open position
    /// tracker.process_fill(
    ///     &symbol,
    ///     OrderSide::Buy,
    ///     100,
    ///     Decimal::new(15000, 2),
    ///     Utc::now(),
    /// ).await.unwrap();
    ///
    /// let has_today_components = tracker.has_components_opened_on_date(&symbol, today).await.unwrap();
    /// assert!(has_today_components);
    /// # });
    /// ```
    pub async fn has_components_opened_on_date(
        &self,
        symbol: &Symbol,
        date: chrono::NaiveDate,
    ) -> Result<bool, PositionTrackingError> {
        let positions = self.positions.read().await;
        
        match positions.get(symbol.as_str()) {
            Some(position) => Ok(position.has_components_opened_on_date(date)),
            None => Ok(false), // No position means no components
        }
    }

    /// Removes flat positions (zero net quantity) from tracking
    ///
    /// This cleanup operation removes positions that have been fully closed
    /// to prevent unbounded memory growth. Should be called periodically.
    ///
    /// # Returns
    ///
    /// The number of positions removed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    ///
    /// let removed_count = tracker.cleanup_flat_positions().await;
    /// println!("Removed {} flat positions", removed_count);
    /// # });
    /// ```
    pub async fn cleanup_flat_positions(&self) -> usize {
        let mut positions = self.positions.write().await;
        let initial_count = positions.len();

        positions.retain(|_symbol, position| !position.is_flat());

        let final_count = positions.len();
        let removed_count = initial_count - final_count;

        if removed_count > 0 {
            tracing::debug!(
                removed_count = removed_count,
                remaining_count = final_count,
                "Cleaned up flat positions"
            );
        }

        removed_count
    }

    /// Removes old position components beyond the retention period
    ///
    /// This cleanup operation removes position components older than
    /// [`MAX_POSITION_COMPONENT_AGE_DAYS`] to prevent unbounded memory growth
    /// while preserving recent history needed for day trade detection.
    ///
    /// # Returns
    ///
    /// The total number of old components removed across all positions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    ///
    /// let removed_count = tracker.cleanup_old_components().await;
    /// println!("Removed {} old position components", removed_count);
    /// # });
    /// ```
    pub async fn cleanup_old_components(&self) -> usize {
        let mut positions = self.positions.write().await;
        let cutoff_date = Utc::now().date_naive() - chrono::Duration::days(MAX_POSITION_COMPONENT_AGE_DAYS);
        let mut total_removed = 0;

        for (_symbol, position) in positions.iter_mut() {
            let initial_components = position.components.len();
            
            // Remove components older than cutoff date
            position.components.retain(|component| {
                component.opened_date >= cutoff_date
            });

            let final_components = position.components.len();
            total_removed += initial_components - final_components;

            // If we removed components, update the position's net quantity consistency
            if initial_components != final_components {
                // Recalculate net quantity from remaining components
                let remaining_quantity: u64 = position.components.iter()
                    .map(|c| c.quantity)
                    .sum();

                if remaining_quantity == 0 && !position.components.is_empty() {
                    // This shouldn't happen with proper FIFO, but handle gracefully
                    tracing::warn!(
                        symbol = %position.symbol,
                        "Inconsistent position state after cleanup - clearing components"
                    );
                    position.components.clear();
                    position.net_quantity = 0;
                    position.side = crate::risk::types::PositionSide::Flat;
                }
            }
        }

        if total_removed > 0 {
            tracing::debug!(
                removed_components = total_removed,
                cutoff_date = %cutoff_date,
                "Cleaned up old position components"
            );
        }

        total_removed
    }

    /// Performs comprehensive cleanup of positions and components
    ///
    /// This combines both flat position removal and old component cleanup
    /// for convenient periodic maintenance.
    ///
    /// # Returns
    ///
    /// A tuple of (removed_positions, removed_components).
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    ///
    /// let (removed_positions, removed_components) = tracker.cleanup().await;
    /// println!("Removed {} positions and {} components", removed_positions, removed_components);
    /// # });
    /// ```
    pub async fn cleanup(&self) -> (usize, usize) {
        let removed_components = self.cleanup_old_components().await;
        let removed_positions = self.cleanup_flat_positions().await;
        (removed_positions, removed_components)
    }

    /// Returns the total number of symbols being tracked
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::position_tracker::PositionTracker;
    /// # tokio_test::block_on(async {
    /// let tracker = PositionTracker::new();
    ///
    /// let count = tracker.position_count().await;
    /// assert_eq!(count, 0); // No positions initially
    /// # });
    /// ```
    pub async fn position_count(&self) -> usize {
        self.positions.read().await.len()
    }

    /// Validates input parameters for position tracking
    fn validate_inputs(&self, quantity: u64, price: Decimal) -> Result<(), PositionTrackingError> {
        if quantity == 0 {
            return Err(PositionTrackingError::InvalidQuantity { quantity });
        }

        if price <= Decimal::ZERO {
            return Err(PositionTrackingError::InvalidPrice { price });
        }

        Ok(())
    }
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::Symbol;
    use rust_decimal::Decimal;

    fn create_symbol(symbol: &str) -> Symbol {
        Symbol::new(symbol).unwrap()
    }

    #[tokio::test]
    async fn test_position_tracker_creation() {
        let tracker = PositionTracker::new();
        assert_eq!(tracker.position_count().await, 0);
    }

    #[tokio::test]
    async fn test_simple_day_trade() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let now = Utc::now();

        // Buy to open
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::new(15000, 2),
            now,
        ).await.unwrap();
        assert!(!is_day_trade);

        // Sell to close same day
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Sell,
            100,
            Decimal::new(15100, 2),
            now,
        ).await.unwrap();
        assert!(is_day_trade);

        // Position should be flat now
        let position = tracker.get_position(&symbol).await.unwrap();
        assert!(position.is_flat());
    }

    #[tokio::test]
    async fn test_partial_day_trade() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let now = Utc::now();

        // Buy to open 200 shares
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            200,
            Decimal::new(15000, 2),
            now,
        ).await.unwrap();
        assert!(!is_day_trade);

        // Sell 100 shares same day (partial close)
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Sell,
            100,
            Decimal::new(15100, 2),
            now,
        ).await.unwrap();
        assert!(is_day_trade); // Partial same-day close is still a day trade

        // Check remaining position
        let position = tracker.get_position(&symbol).await.unwrap();
        assert_eq!(position.net_quantity, 100);
        assert!(!position.is_flat());
    }

    #[tokio::test]
    async fn test_no_day_trade_different_days() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let today = Utc::now();
        let tomorrow = today + chrono::Duration::days(1);

        // Buy today
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::new(15000, 2),
            today,
        ).await.unwrap();
        assert!(!is_day_trade);

        // Sell tomorrow (not same day)
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Sell,
            100,
            Decimal::new(15100, 2),
            tomorrow,
        ).await.unwrap();
        assert!(!is_day_trade); // Different days = not a day trade
    }

    #[tokio::test]
    async fn test_short_position_day_trade() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let now = Utc::now();

        // Sell to open short position
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Sell,
            100,
            Decimal::new(15000, 2),
            now,
        ).await.unwrap();
        assert!(!is_day_trade);

        // Buy to cover same day
        let is_day_trade = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::new(14900, 2),
            now,
        ).await.unwrap();
        assert!(is_day_trade);

        // Position should be flat
        let position = tracker.get_position(&symbol).await.unwrap();
        assert!(position.is_flat());
    }

    #[tokio::test]
    async fn test_validation_errors() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let now = Utc::now();

        // Invalid quantity (zero)
        let result = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            0,
            Decimal::new(15000, 2),
            now,
        ).await;
        assert!(matches!(result, Err(PositionTrackingError::InvalidQuantity { .. })));

        // Invalid price (zero)
        let result = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::ZERO,
            now,
        ).await;
        assert!(matches!(result, Err(PositionTrackingError::InvalidPrice { .. })));

        // Invalid price (negative)
        let result = tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::new(-100, 0),
            now,
        ).await;
        assert!(matches!(result, Err(PositionTrackingError::InvalidPrice { .. })));
    }

    #[tokio::test]
    async fn test_cleanup_flat_positions() {
        let tracker = PositionTracker::new();
        let symbol1 = create_symbol("AAPL");
        let symbol2 = create_symbol("GOOGL");
        let now = Utc::now();

        // Create and close position for AAPL (will be flat)
        tracker.process_fill(&symbol1, OrderSide::Buy, 100, Decimal::new(15000, 2), now).await.unwrap();
        tracker.process_fill(&symbol1, OrderSide::Sell, 100, Decimal::new(15100, 2), now).await.unwrap();

        // Create open position for GOOGL (will not be flat)
        tracker.process_fill(&symbol2, OrderSide::Buy, 100, Decimal::new(280000, 2), now).await.unwrap();

        assert_eq!(tracker.position_count().await, 2);

        // Cleanup should remove the flat AAPL position
        let removed = tracker.cleanup_flat_positions().await;
        assert_eq!(removed, 1);
        assert_eq!(tracker.position_count().await, 1);

        // Remaining position should be GOOGL
        let position = tracker.get_position(&symbol2).await.unwrap();
        assert!(!position.is_flat());
    }

    #[tokio::test]
    async fn test_has_components_opened_on_date() {
        let tracker = PositionTracker::new();
        let symbol = create_symbol("AAPL");
        let today = Utc::now();
        let yesterday = today - chrono::Duration::days(1);

        // Open position yesterday
        tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            100,
            Decimal::new(15000, 2),
            yesterday,
        ).await.unwrap();

        // Check for yesterday's components
        let has_yesterday = tracker.has_components_opened_on_date(
            &symbol,
            yesterday.date_naive(),
        ).await.unwrap();
        assert!(has_yesterday);

        // Check for today's components
        let has_today = tracker.has_components_opened_on_date(
            &symbol,
            today.date_naive(),
        ).await.unwrap();
        assert!(!has_today);

        // Add more position today
        tracker.process_fill(
            &symbol,
            OrderSide::Buy,
            50,
            Decimal::new(15100, 2),
            today,
        ).await.unwrap();

        // Now should have both
        let has_yesterday = tracker.has_components_opened_on_date(
            &symbol,
            yesterday.date_naive(),
        ).await.unwrap();
        assert!(has_yesterday);

        let has_today = tracker.has_components_opened_on_date(
            &symbol,
            today.date_naive(),
        ).await.unwrap();
        assert!(has_today);
    }
}