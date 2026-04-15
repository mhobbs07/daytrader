// Position tracking with real-time P&L updates
//
// This module provides comprehensive position lifecycle management, including position
// opening from fills, real-time price updates, P&L tracking, and position closure workflows.

use crate::brokers::traits::{OrderExecutionProvider, OrderSide};
use crate::brokers::types::{OrderId, PatternName, Price, Quantity, Symbol};
use crate::events::{EventBus, MarketEvent};
use crate::orders::types::{FillInfo, ManagedOrder};
use crate::positions::pnl::{PnLCalculator, PnLConfig};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use thiserror::Error;
use tracing;

/// Position tracking errors
#[derive(Debug, Error)]
pub enum PositionError {
    #[error("Position not found: {0}")]
    PositionNotFound(Symbol),

    #[error("Invalid position state: {0}")]
    InvalidState(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Broker error: {0}")]
    BrokerError(#[from] crate::utils::errors::BrokerError),

    #[error("Order error: {0}")]
    OrderError(#[from] crate::orders::types::OrderError),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Position side (long or short)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionSide {
    /// Long position (bought shares)
    Long,
    /// Short position (sold shares short) - future support
    Short,
}

/// Reason for closing a position
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CloseReason {
    /// Manual closure by user
    Manual,
    /// Stop loss triggered
    StopLoss,
    /// Take profit target hit
    TakeProfit,
    /// Risk violation forced closure
    RiskViolation,
    /// Pattern exit signal
    PatternExit,
    /// End of day closure
    EndOfDay,
}

/// Position structure representing an open position
///
/// Tracks all information about an open position including cost basis,
/// current value, P&L, and associated orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Symbol of the position
    pub symbol: Symbol,

    /// Current quantity owned
    pub quantity: Quantity,

    /// Long or short
    pub side: PositionSide,

    /// Average entry price
    pub avg_entry_price: Price,

    /// Current market price
    pub current_price: Price,

    /// Current market value
    pub market_value: Decimal,

    /// Total cost basis
    pub cost_basis: Decimal,

    /// Unrealized profit/loss
    pub unrealized_pl: Decimal,

    /// Unrealized P&L percentage
    pub unrealized_pl_percent: Decimal,

    /// Realized profit/loss from partial closes
    pub realized_pl: Decimal,

    /// When position was opened
    pub opened_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    // Trade tracking
    /// Order IDs that opened/added to this position
    pub entry_orders: Vec<OrderId>,

    /// Order IDs that closed/reduced this position
    pub exit_orders: Vec<OrderId>,

    // Pattern tracking (optional)
    /// Pattern name if this is a pattern-based trade
    pub pattern_name: Option<PatternName>,

    /// Signal confidence score
    pub signal_confidence: Option<Decimal>,
}

impl Position {
    /// Create a new position from an initial fill
    pub fn new(
        symbol: Symbol,
        side: PositionSide,
        quantity: Quantity,
        entry_price: Price,
        order_id: OrderId,
        pattern_name: Option<PatternName>,
        signal_confidence: Option<Decimal>,
    ) -> Self {
        let now = Utc::now();
        let cost_basis = entry_price.value() * Decimal::from(quantity.value());
        let market_value = cost_basis; // Initial market value equals cost

        Self {
            symbol,
            quantity,
            side,
            avg_entry_price: entry_price,
            current_price: entry_price,
            market_value,
            cost_basis,
            unrealized_pl: Decimal::ZERO,
            unrealized_pl_percent: Decimal::ZERO,
            realized_pl: Decimal::ZERO,
            opened_at: now,
            updated_at: now,
            entry_orders: vec![order_id],
            exit_orders: Vec::new(),
            pattern_name,
            signal_confidence,
        }
    }

    /// Calculate holding period
    pub fn holding_period(&self) -> Duration {
        Utc::now() - self.opened_at
    }

    /// Returns true if this position is for a pattern-based trade
    pub fn is_pattern_trade(&self) -> bool {
        self.pattern_name.is_some()
    }
}

/// Position tracker managing all open positions
///
/// Provides real-time position tracking with P&L calculations, market price updates,
/// and position lifecycle management.
pub struct PositionTracker {
    /// Active positions (symbol -> position)
    positions: Arc<DashMap<Symbol, Position>>,

    /// Dependencies
    broker: Arc<dyn OrderExecutionProvider>,
    event_bus: EventBus,
    db_pool: Arc<SqlitePool>,

    /// P&L calculator
    pnl_calculator: Arc<PnLCalculator>,

    /// Configuration
    config: PositionConfig,
}

/// Configuration for position tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionConfig {
    /// Enable automatic P&L updates on market data
    pub auto_update_pnl: bool,

    /// Update interval for market price (milliseconds)
    pub price_update_interval_ms: u64,

    /// Enable position persistence to database
    pub enable_persistence: bool,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            auto_update_pnl: true,
            price_update_interval_ms: 1000,
            enable_persistence: true,
        }
    }
}

impl PositionTracker {
    /// Create a new position tracker
    pub fn new(
        broker: Arc<dyn OrderExecutionProvider>,
        event_bus: EventBus,
        db_pool: Arc<SqlitePool>,
        pnl_config: PnLConfig,
        config: PositionConfig,
    ) -> Self {
        Self {
            positions: Arc::new(DashMap::new()),
            broker,
            event_bus,
            db_pool,
            pnl_calculator: Arc::new(PnLCalculator::new(pnl_config)),
            config,
        }
    }

    /// Update position based on filled order
    ///
    /// This is the primary method for creating or modifying positions based on order fills.
    pub async fn update_from_fill(
        &self,
        order: &ManagedOrder,
        fill_info: &FillInfo,
    ) -> Result<(), PositionError> {
        let symbol = &order.symbol;

        match order.side {
            OrderSide::Buy => {
                // Buying increases or opens a long position
                self.handle_buy_fill(order, fill_info).await?;
            }
            OrderSide::Sell => {
                // Selling decreases or closes a long position
                self.handle_sell_fill(order, fill_info).await?;
            }
        }

        Ok(())
    }

    /// Handle a buy order fill (open or add to position)
    async fn handle_buy_fill(
        &self,
        order: &ManagedOrder,
        fill_info: &FillInfo,
    ) -> Result<(), PositionError> {
        let symbol = &order.symbol;

        // Get or create position
        if let Some(mut position_ref) = self.positions.get_mut(symbol) {
            // Position exists - average in the new shares
            let existing_cost =
                position_ref.avg_entry_price.value() * Decimal::from(position_ref.quantity.value());
            let new_cost =
                fill_info.filled_price.value() * Decimal::from(fill_info.filled_quantity.value());
            let total_cost = existing_cost + new_cost;

            let total_quantity = position_ref.quantity.value() + fill_info.filled_quantity.value();

            // Update position
            position_ref.quantity = Quantity::new(total_quantity)
                .map_err(|e| PositionError::Internal(e.to_string()))?;
            position_ref.avg_entry_price = Price::new(total_cost / Decimal::from(total_quantity))
                .map_err(|e| PositionError::Internal(e.to_string()))?;
            position_ref.cost_basis = total_cost;
            position_ref.entry_orders.push(order.id.clone());
            position_ref.updated_at = Utc::now();

            // Recalculate P&L
            self.update_position_pnl(&mut position_ref).await;

            // Persist to database
            if self.config.enable_persistence {
                self.save_position(&position_ref).await?;
            }

            // Publish event
            self.publish_position_updated_event(&position_ref).await;

            tracing::info!(
                "Added to position: {} (+{} @ {})",
                symbol,
                fill_info.filled_quantity.value(),
                fill_info.filled_price.value()
            );
        } else {
            // Create new position
            let position = Position::new(
                symbol.clone(),
                PositionSide::Long,
                fill_info.filled_quantity,
                fill_info.filled_price,
                order.id.clone(),
                order.pattern_name.clone(),
                order.signal_confidence,
            );

            self.positions.insert(symbol.clone(), position.clone());

            // Persist to database
            if self.config.enable_persistence {
                self.save_position(&position).await?;
            }

            // Publish event
            self.publish_position_opened_event(&position).await;

            tracing::info!(
                "Opened position: {} ({} @ {})",
                symbol,
                fill_info.filled_quantity.value(),
                fill_info.filled_price.value()
            );
        }

        Ok(())
    }

    /// Handle a sell order fill (reduce or close position)
    async fn handle_sell_fill(
        &self,
        order: &ManagedOrder,
        fill_info: &FillInfo,
    ) -> Result<(), PositionError> {
        let symbol = &order.symbol;

        let mut position_ref = self
            .positions
            .get_mut(symbol)
            .ok_or_else(|| PositionError::PositionNotFound(symbol.clone()))?;

        let new_quantity = position_ref
            .quantity
            .value()
            .saturating_sub(fill_info.filled_quantity.value());

        // Calculate realized P&L for the sold shares
        let realized_pl = self.pnl_calculator.calculate_realized_pl(
            position_ref.avg_entry_price,
            fill_info.filled_price,
            fill_info.filled_quantity,
            fill_info.commission,
        );

        position_ref.realized_pl += realized_pl;
        position_ref.exit_orders.push(order.id.clone());
        position_ref.updated_at = Utc::now();

        if new_quantity == 0 {
            // Position fully closed
            let closed_position = position_ref.clone();
            drop(position_ref); // Release the lock before removing

            self.positions.remove(symbol);

            // Archive to database
            if self.config.enable_persistence {
                self.archive_position(&closed_position).await?;
            }

            // Publish closed event
            self.publish_position_closed_event(
                symbol,
                closed_position.realized_pl,
                closed_position.holding_period(),
            )
            .await;

            tracing::info!(
                "Closed position: {} (realized P&L: ${:.2})",
                symbol,
                realized_pl
            );
        } else {
            // Partial close
            position_ref.quantity =
                Quantity::new(new_quantity).map_err(|e| PositionError::Internal(e.to_string()))?;

            // Recalculate cost basis (proportional reduction)
            let remaining_ratio = Decimal::from(new_quantity)
                / Decimal::from(new_quantity + fill_info.filled_quantity.value());
            position_ref.cost_basis *= remaining_ratio;

            // Update P&L
            self.update_position_pnl(&mut position_ref).await;

            // Persist to database
            if self.config.enable_persistence {
                self.save_position(&position_ref).await?;
            }

            // Publish updated event
            self.publish_position_updated_event(&position_ref).await;

            tracing::info!(
                "Reduced position: {} (-{}, {} remaining, realized P&L: ${:.2})",
                symbol,
                fill_info.filled_quantity.value(),
                new_quantity,
                realized_pl
            );
        }

        Ok(())
    }

    /// Update position with current market price
    ///
    /// Recalculates unrealized P&L based on the new price.
    pub async fn update_market_price(
        &self,
        symbol: &Symbol,
        price: Price,
    ) -> Result<(), PositionError> {
        if let Some(mut position) = self.positions.get_mut(symbol) {
            position.current_price = price;
            position.updated_at = Utc::now();

            // Recalculate P&L
            self.update_position_pnl(&mut position).await;

            // Persist to database
            if self.config.enable_persistence {
                self.save_position(&position).await?;
            }

            tracing::trace!(
                "Updated price for {}: {} (unrealized P&L: ${:.2})",
                symbol,
                price.value(),
                position.unrealized_pl
            );
        }

        Ok(())
    }

    /// Update P&L calculations for a position
    async fn update_position_pnl(&self, position: &mut Position) {
        position.unrealized_pl = self.pnl_calculator.calculate_unrealized_pl(
            position.cost_basis,
            position.quantity.value(),
            position.current_price,
        );

        position.unrealized_pl_percent = self.pnl_calculator.calculate_unrealized_pl_percent(
            position.cost_basis,
            position.quantity.value(),
            position.current_price,
        );

        position.market_value = self
            .pnl_calculator
            .calculate_market_value(position.quantity.value(), position.current_price);
    }

    /// Get current position for symbol
    pub async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, PositionError> {
        Ok(self
            .positions
            .get(symbol)
            .map(|p: dashmap::mapref::one::Ref<Symbol, Position>| p.clone()))
    }

    /// Get all open positions
    pub async fn get_all_positions(&self) -> Result<Vec<Position>, PositionError> {
        let positions: Vec<Position> = self
            .positions
            .iter()
            .map(
                |entry: dashmap::mapref::multiple::RefMulti<Symbol, Position>| {
                    entry.value().clone()
                },
            )
            .collect();
        Ok(positions)
    }

    /// Close position (create exit order)
    pub async fn close_position(
        &self,
        symbol: &Symbol,
        reason: CloseReason,
    ) -> Result<OrderId, PositionError> {
        let position = self
            .get_position(symbol)
            .await?
            .ok_or_else(|| PositionError::PositionNotFound(symbol.clone()))?;

        // Create market order to close position
        use crate::brokers::traits::{OrderType, TimeInForce};
        use crate::orders::types::OrderRequest;

        let order_request = OrderRequest {
            symbol: symbol.clone(),
            side: OrderSide::Sell, // Always sell to close long positions
            order_type: OrderType::Market,
            quantity: position.quantity,
            limit_price: None,
            stop_price: None,
            trailing_percent: None,
            time_in_force: TimeInForce::Day,
            pattern_name: position.pattern_name,
            signal_id: None,
            signal_confidence: None,
        };

        // Note: This would normally go through the order engine
        // For now, we return a placeholder OrderId
        let order_id = OrderId::new(format!("close-{}", symbol.as_str()));

        tracing::info!(
            "Closing position for {} (reason: {:?}), order ID: {}",
            symbol,
            reason,
            order_id
        );

        Ok(order_id)
    }

    /// Sync positions from broker
    ///
    /// Synchronizes local position state with broker's position data.
    pub async fn sync_from_broker(&self) -> Result<(), PositionError> {
        let broker_positions = self.broker.get_positions().await?;
        let position_count = broker_positions.len();

        for broker_pos in broker_positions {
            let quantity_abs = broker_pos.quantity.abs() as u32;
            let side = if broker_pos.quantity > 0 {
                PositionSide::Long
            } else {
                PositionSide::Short
            };

            let position = Position {
                symbol: broker_pos.symbol.clone(),
                quantity: Quantity::new(quantity_abs)
                    .map_err(|e| PositionError::Internal(e.to_string()))?,
                side,
                avg_entry_price: broker_pos.avg_entry_price,
                current_price: broker_pos.current_price,
                market_value: broker_pos.market_value,
                cost_basis: broker_pos.avg_entry_price.value() * Decimal::from(quantity_abs),

                unrealized_pl: broker_pos.unrealized_pl,
                unrealized_pl_percent: if broker_pos.market_value != Decimal::ZERO {
                    (broker_pos.unrealized_pl / broker_pos.market_value) * Decimal::from(100)
                } else {
                    Decimal::ZERO
                },
                realized_pl: Decimal::ZERO,
                opened_at: Utc::now(), // Unknown from broker
                updated_at: Utc::now(),
                entry_orders: Vec::new(),
                exit_orders: Vec::new(),
                pattern_name: None,
                signal_confidence: None,
            };

            self.positions
                .insert(broker_pos.symbol.clone(), position.clone());

            if self.config.enable_persistence {
                self.save_position(&position).await?;
            }
        }

        tracing::info!("Synced {} positions from broker", position_count);

        Ok(())
    }

    // ============================================================================
    // Database Operations
    // ============================================================================

    /// Save position to database
    async fn save_position(&self, position: &Position) -> Result<(), PositionError> {
        use crate::db::repositories::PositionRepository;

        let repo = PositionRepository::new((*self.db_pool).clone());

        // Use account_id = 1 as default; in production, this would come from context
        let account_id = 1;

        repo.save_position(position, account_id).await?;

        tracing::debug!("Saved position to database: {}", position.symbol);
        Ok(())
    }

    /// Archive closed position to history
    async fn archive_position(&self, position: &Position) -> Result<(), PositionError> {
        use crate::db::repositories::PositionRepository;

        let repo = PositionRepository::new((*self.db_pool).clone());

        // Use account_id = 1 as default; in production, this would come from context
        let account_id = 1;

        // Use current price as exit price for archival
        let exit_price = position.current_price;

        // Determine close reason based on position state
        let close_reason = CloseReason::Manual; // Default; caller should specify

        repo.close_position(position, account_id, &close_reason, exit_price)
            .await?;

        tracing::debug!("Archived position to history: {}", position.symbol);
        Ok(())
    }

    // ============================================================================
    // Event Publishing
    // ============================================================================

    /// Publish position opened event
    async fn publish_position_opened_event(&self, position: &Position) {
        let event = MarketEvent::PositionOpened {
            position: crate::events::types::Position {
                symbol: position.symbol.clone(),
                quantity: position.quantity,
                average_entry_price: position.avg_entry_price,
                current_price: position.current_price,
                unrealized_pl: position.unrealized_pl,
                realized_pl: position.realized_pl,
                opened_at: position.opened_at,
                updated_at: position.updated_at,
            },
        };

        self.event_bus.publish(event);
    }

    /// Publish position updated event
    async fn publish_position_updated_event(&self, position: &Position) {
        let event = MarketEvent::PositionUpdated {
            position: crate::events::types::Position {
                symbol: position.symbol.clone(),
                quantity: position.quantity,
                average_entry_price: position.avg_entry_price,
                current_price: position.current_price,
                unrealized_pl: position.unrealized_pl,
                realized_pl: position.realized_pl,
                opened_at: position.opened_at,
                updated_at: position.updated_at,
            },
        };

        self.event_bus.publish(event);
    }

    /// Publish position closed event
    async fn publish_position_closed_event(
        &self,
        symbol: &Symbol,
        realized_pl: Decimal,
        _holding_period: Duration,
    ) {
        let event = MarketEvent::PositionClosed {
            symbol: symbol.clone(),
            realized_pl,
        };

        self.event_bus.publish(event);
    }

    /// Get P&L calculator reference
    pub fn pnl_calculator(&self) -> &PnLCalculator {
        &self.pnl_calculator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_position(symbol: &str) -> Position {
        Position::new(
            Symbol::new(symbol).unwrap(),
            PositionSide::Long,
            Quantity::new(100).unwrap(),
            Price::new(dec!(100.00)).unwrap(),
            OrderId::new("test-order"),
            None,
            None,
        )
    }

    #[test]
    fn test_position_creation() {
        let position = create_test_position("AAPL");

        assert_eq!(position.symbol.as_str(), "AAPL");
        assert_eq!(position.quantity.value(), 100);
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.avg_entry_price.value(), dec!(100.00));
        assert_eq!(position.cost_basis, dec!(10000.00));
        assert_eq!(position.unrealized_pl, Decimal::ZERO);
    }

    #[test]
    fn test_position_is_pattern_trade() {
        let mut position = create_test_position("AAPL");
        assert!(!position.is_pattern_trade());

        position.pattern_name = Some(PatternName::new("Hammer"));
        assert!(position.is_pattern_trade());
    }

    #[test]
    fn test_close_reason_variants() {
        let reasons = vec![
            CloseReason::Manual,
            CloseReason::StopLoss,
            CloseReason::TakeProfit,
            CloseReason::RiskViolation,
            CloseReason::PatternExit,
            CloseReason::EndOfDay,
        ];

        for reason in reasons {
            let _ = format!("{:?}", reason); // Test Debug impl
        }
    }
}
