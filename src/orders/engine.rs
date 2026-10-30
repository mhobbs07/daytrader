//! Order engine - Core order placement and lifecycle management
//!
//! The OrderEngine coordinates all order-related operations including
//! validation, risk checking, submission, and status tracking.

use crate::brokers::traits::{Account, OrderExecutionProvider};
use crate::brokers::types::OrderId;
use crate::events::bus::EventBus;
use crate::orders::broker_adapter::BrokerAdapter;
use crate::orders::tracker::OrderTracker;
use crate::orders::types::{ManagedOrder, OrderConfig, OrderError, OrderRequest};
use crate::risk::engine::RiskEngine;
use crate::utils::clock::{Clock, SystemClock};
use crate::utils::market_hours::is_market_open;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ============================================================================
// Order Engine
// ============================================================================

/// Core order management engine
///
/// Coordinates order placement with validation, risk checking, broker
/// submission, and status tracking.
///
/// # Architecture
///
/// The engine follows a multi-stage pipeline:
/// 1. Parameter validation
/// 2. Market hours checking
/// 3. Buying power validation
/// 4. Risk engine validation
/// 5. Pattern trade validation (if applicable)
/// 6. Database persistence
/// 7. Broker submission
/// 8. Status tracking
/// 9. Event publication
///
/// # Examples
///
/// ```no_run
/// use daytrader::orders::engine::OrderEngine;
/// use daytrader::orders::types::OrderRequest;
/// use daytrader::brokers::types::{Symbol, Quantity};
/// use daytrader::brokers::traits::{OrderSide, OrderType};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let engine: OrderEngine = todo!();
/// let request = OrderRequest::builder()
///     .symbol(Symbol::new("AAPL")?)
///     .side(OrderSide::Buy)
///     .order_type(OrderType::Market)
///     .quantity(Quantity::new(100)?)
///     .build()?;
///
/// let order_id = engine.place_order(request).await?;
/// println!("Order placed: {}", order_id);
/// # Ok(())
/// # }
/// ```
pub struct OrderEngine {
    /// Broker client for account info and order execution
    broker: Arc<dyn OrderExecutionProvider>,
    /// Risk engine for pre-trade validation
    risk_engine: Arc<RiskEngine>,
    /// Order tracker for status monitoring
    tracker: Arc<OrderTracker>,
    /// Broker adapter for order submission
    broker_adapter: Arc<dyn BrokerAdapter>,
    /// Event bus for publishing order events
    event_bus: Arc<EventBus>,
    /// Database pool for persistence
    db_pool: Arc<sqlx::SqlitePool>,
    /// Configuration
    config: OrderConfig,
    /// Order ID counter
    order_counter: Arc<AtomicU64>,
    /// Account ID for database operations
    account_id: i64,
    /// Time source for market-hours checks (injectable for deterministic tests)
    clock: Arc<dyn Clock>,
}

impl OrderEngine {
    /// Creates a new order engine
    ///
    /// # Parameters
    ///
    /// - `broker`: Broker client implementing OrderExecutionProvider
    /// - `risk_engine`: Risk engine for pre-trade validation
    /// - `broker_adapter`: Adapter for broker-specific order operations
    /// - `event_bus`: Event bus for publishing order events
    /// - `db_pool`: Database connection pool
    /// - `account_id`: Account ID for database operations
    /// - `config`: Order management configuration
    pub fn new(
        broker: Arc<dyn OrderExecutionProvider>,
        risk_engine: Arc<RiskEngine>,
        broker_adapter: Arc<dyn BrokerAdapter>,
        event_bus: Arc<EventBus>,
        db_pool: Arc<sqlx::SqlitePool>,
        account_id: i64,
        config: OrderConfig,
    ) -> Self {
        let tracker = Arc::new(OrderTracker::new(Arc::clone(&event_bus)));

        Self {
            broker,
            risk_engine,
            tracker,
            broker_adapter,
            event_bus,
            db_pool,
            config,
            order_counter: Arc::new(AtomicU64::new(0)),
            account_id,
            clock: Arc::new(SystemClock),
        }
    }

    /// Replaces the engine's time source
    ///
    /// Production uses the default [`SystemClock`]; tests inject a
    /// [`crate::utils::clock::FixedClock`] pinned inside market hours so
    /// order placement is deterministic regardless of when the suite runs.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Gets access to the broker provider
    pub fn broker(&self) -> &Arc<dyn OrderExecutionProvider> {
        &self.broker
    }

    /// Places a new order with full validation pipeline
    ///
    /// # Validation Pipeline
    ///
    /// 1. Validates order parameters
    /// 2. Checks market hours
    /// 3. Validates buying power
    /// 4. Runs risk validation
    /// 5. Validates pattern trades (if applicable)
    /// 6. Persists to database
    /// 7. Submits to broker
    /// 8. Tracks order status
    /// 9. Publishes events
    ///
    /// # Errors
    ///
    /// Returns an error if any validation step fails or submission fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use daytrader::orders::engine::OrderEngine;
    /// # use daytrader::orders::types::OrderRequest;
    /// # async fn example(engine: &OrderEngine, request: OrderRequest) -> Result<(), Box<dyn std::error::Error>> {
    /// let order_id = engine.place_order(request).await?;
    /// println!("Order placed: {}", order_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn place_order(&self, request: OrderRequest) -> Result<OrderId, OrderError> {
        let start = std::time::Instant::now();

        info!(
            symbol = %request.symbol,
            side = ?request.side,
            order_type = ?request.order_type,
            quantity = request.quantity.value(),
            "Placing order"
        );

        // 1. Validate order parameters
        request.validate()?;
        debug!("Order parameters validated");

        // 2. Check market hours, weekends, and holidays (fail closed: if the
        // check errors we cannot prove the market is open, so the order is
        // rejected)
        match is_market_open(self.clock.now()) {
            Ok(true) => debug!("Market hours check passed"),
            Ok(false) => {
                warn!(symbol = %request.symbol, "Order rejected: market is closed");
                return Err(OrderError::MarketClosed);
            }
            Err(e) => {
                error!(error = %e, "Market hours check failed; rejecting order");
                return Err(OrderError::MarketClosed);
            }
        }

        // 3. Get account info and validate buying power
        let account = self.broker.get_account().await?;
        self.validate_buying_power(&request, &account)?;
        debug!("Buying power validated");

        // 4. Create managed order with generated ID
        let order_id = self.generate_order_id();
        let mut managed_order = ManagedOrder::from_request(order_id.clone(), request.clone());

        // 5. Run risk validation
        // Note: RiskEngine validates based on events, so we check trading allowed
        if !self.risk_engine.is_trading_allowed().await {
            return Err(OrderError::RiskViolation(
                "Trading is currently halted".into(),
            ));
        }
        managed_order.pre_trade_validation_passed = true;
        managed_order.risk_validation_timestamp = Some(Utc::now());
        debug!("Risk validation passed");

        // 6. Pattern-specific validation (if applicable)
        if let Some(ref pattern_name) = request.pattern_name {
            self.validate_pattern_trade(pattern_name).await?;
            debug!(pattern = %pattern_name, "Pattern trade validated");
        }

        // 7. Persist to database
        self.save_order(&managed_order).await?;
        debug!(order_id = %order_id, "Order persisted to database");

        // 8. Start tracking
        self.tracker.track_order(managed_order.clone()).await;

        // 9. Mark validation passed in tracker
        self.tracker.mark_validation_passed(&order_id)?;

        // 10. Submit to broker
        let broker_order_id = self.broker_adapter.submit_order(&managed_order).await?;
        debug!(
            order_id = %order_id,
            broker_order_id = %broker_order_id,
            "Order submitted to broker"
        );

        // 11. Update with broker ID
        let submitted_at = Utc::now();
        self.tracker
            .update_broker_id(&order_id, broker_order_id.clone(), submitted_at)?;

        // 12. Update in database
        self.update_broker_id_in_db(&order_id, &broker_order_id, submitted_at)
            .await?;

        let elapsed = start.elapsed();
        info!(
            order_id = %order_id,
            broker_order_id = %broker_order_id,
            elapsed_ms = elapsed.as_millis(),
            "Order placement complete"
        );

        Ok(order_id)
    }

    /// Cancels an existing order
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Order is not found
    /// - Order is not cancellable (already filled, cancelled, etc.)
    /// - Broker cancellation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use daytrader::orders::engine::OrderEngine;
    /// # use daytrader::brokers::types::OrderId;
    /// # async fn example(engine: &OrderEngine, order_id: OrderId) -> Result<(), Box<dyn std::error::Error>> {
    /// engine.cancel_order(&order_id).await?;
    /// println!("Order cancelled");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<(), OrderError> {
        info!(order_id = %order_id, "Cancelling order");

        // Get order
        let order = self.tracker.get_order(order_id).await?;

        // Validate cancellable
        if !order.is_cancellable() {
            return Err(OrderError::NotCancellable(order_id.clone(), order.status));
        }

        // Cancel with broker if submitted
        if let Some(ref broker_id) = order.broker_order_id {
            self.broker_adapter.cancel_order(broker_id).await?;
            debug!(
                order_id = %order_id,
                broker_order_id = %broker_id,
                "Order cancelled with broker"
            );
        }

        // Update status
        self.tracker
            .update_order_status(
                order_id,
                crate::brokers::traits::OrderStatus::Cancelled,
                None,
            )
            .await?;

        // Update in database
        self.update_order_status_in_db(order_id, crate::brokers::traits::OrderStatus::Cancelled)
            .await?;

        info!(order_id = %order_id, "Order cancelled successfully");

        Ok(())
    }

    /// Gets an order by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the order is not found.
    pub async fn get_order(&self, order_id: &OrderId) -> Result<ManagedOrder, OrderError> {
        self.tracker.get_order(order_id).await
    }

    /// Gets all active (open) orders
    pub fn get_active_orders(&self) -> Vec<ManagedOrder> {
        self.tracker.get_active_orders()
    }

    /// Gets all archived orders
    pub async fn get_archived_orders(&self) -> Vec<ManagedOrder> {
        self.tracker.get_archived_orders().await
    }

    /// Polls the broker for status changes on all submitted active orders
    ///
    /// For each active order that has a broker order ID, fetches the broker's
    /// view and applies any status transition through the tracker, which
    /// publishes the corresponding lifecycle events (`OrderFilled`,
    /// `OrderCancelled`, ...) and archives terminal orders. The database row
    /// is updated for every applied transition.
    ///
    /// Intended to be called periodically by the application runtime; brokers
    /// without push-based order updates (REST polling, the mock broker) get
    /// their fills surfaced this way.
    ///
    /// Returns the number of orders whose status changed. Per-order broker
    /// errors are logged and skipped so one bad order cannot stall the sweep.
    ///
    /// # Errors
    ///
    /// Returns an error if applying a status transition to the tracker fails.
    pub async fn sync_order_statuses(&self) -> Result<usize, OrderError> {
        let mut updated = 0;

        for order in self.tracker.get_active_orders() {
            let Some(broker_order_id) = order.broker_order_id.clone() else {
                continue;
            };

            let broker_order = match self.broker.get_order(&OrderId::new(&broker_order_id)).await {
                Ok(broker_order) => broker_order,
                Err(e) => {
                    warn!(
                        order_id = %order.id,
                        broker_order_id = %broker_order_id,
                        error = %e,
                        "Failed to poll broker order status"
                    );
                    continue;
                }
            };

            if broker_order.status == order.status {
                continue;
            }

            let fill_info = match broker_order.status {
                crate::brokers::traits::OrderStatus::Filled
                | crate::brokers::traits::OrderStatus::PartiallyFilled => {
                    // Brokers report filled_quantity CUMULATIVELY, while
                    // ManagedOrder::update_with_fill applies fills as
                    // increments — feeding the cumulative value through
                    // double-counts every earlier tranche. Book only the
                    // delta since the last observed state.
                    let delta = broker_order
                        .filled_quantity
                        .saturating_sub(order.filled_quantity);

                    if delta == 0 {
                        // Status-only transition (e.g. PartiallyFilled ->
                        // Filled with no new shares); nothing to book
                        None
                    } else {
                        let Some(new_avg) = broker_order.filled_avg_price else {
                            warn!(
                                order_id = %order.id,
                                "Broker reported fill without a price; skipping"
                            );
                            continue;
                        };
                        // Derive this tranche's price from the cumulative
                        // averages so the weighted average stays exact
                        let tranche_price = match order.filled_avg_price {
                            Some(prev_avg) if order.filled_quantity > 0 => {
                                let total_new =
                                    new_avg.value() * Decimal::from(broker_order.filled_quantity);
                                let total_prev =
                                    prev_avg.value() * Decimal::from(order.filled_quantity);
                                crate::brokers::types::Price::new(
                                    (total_new - total_prev) / Decimal::from(delta),
                                )
                                .unwrap_or(new_avg)
                            }
                            _ => new_avg,
                        };
                        match crate::brokers::types::Quantity::new(delta) {
                            Ok(filled_quantity) => Some(crate::orders::types::FillInfo {
                                filled_quantity,
                                filled_price: tranche_price,
                                commission: Decimal::ZERO,
                                timestamp: broker_order.filled_at.unwrap_or_else(Utc::now),
                            }),
                            Err(_) => None,
                        }
                    }
                }
                _ => None,
            };

            self.tracker
                .update_order_status(&order.id, broker_order.status, fill_info)
                .await?;
            self.update_order_status_in_db(&OrderId::new(&broker_order_id), broker_order.status)
                .await?;
            updated += 1;
        }

        Ok(updated)
    }

    // ========================================================================
    // Validation Methods
    // ========================================================================

    /// Validates buying power for an order
    fn validate_buying_power(
        &self,
        request: &OrderRequest,
        account: &Account,
    ) -> Result<(), OrderError> {
        // Only check for buy orders
        if request.side != crate::brokers::traits::OrderSide::Buy {
            return Ok(());
        }

        // Estimate order value
        let estimated_value = request.estimated_value().unwrap_or_else(|| {
            // For market orders without price, we can't validate
            // This should be caught by requiring limit price or current market price
            warn!(
                symbol = %request.symbol,
                "Cannot estimate order value for market order without price"
            );
            Decimal::ZERO
        });

        // Check buying power
        if estimated_value > account.buying_power {
            return Err(OrderError::InsufficientBuyingPower {
                required: estimated_value,
                available: account.buying_power,
            });
        }

        Ok(())
    }

    /// Validates pattern trade against limits
    async fn validate_pattern_trade(
        &self,
        pattern_name: &crate::brokers::types::PatternName,
    ) -> Result<(), OrderError> {
        // Get pattern evaluator from risk engine
        let evaluator = self.risk_engine.get_pattern_risk_evaluator();
        let evaluator_guard = evaluator.read().await;

        // Check if pattern is enabled
        if let Some(state) = evaluator_guard.get_pattern_state(pattern_name) {
            if !state.is_enabled {
                return Err(OrderError::PatternDisabled(pattern_name.to_string()));
            }
        }

        // TODO: Check daily trade limit for this pattern
        // This would require querying the database for today's pattern trades
        // For now, rely on risk engine's pattern validation

        Ok(())
    }

    // ========================================================================
    // Database Operations
    // ========================================================================

    /// Saves order to database
    async fn save_order(&self, order: &ManagedOrder) -> Result<(), OrderError> {
        let symbol_str = order.symbol.as_str().to_string();
        let side_str = format!("{:?}", order.side).to_lowercase();
        let order_type_str = format!("{:?}", order.order_type).to_lowercase();
        let quantity_val = order.quantity.value();
        let limit_price_val = order.limit_price.map(|p| p.as_f64());
        let stop_price_val = order.stop_price.map(|p| p.as_f64());
        let trailing_val = order.trailing_amount.and_then(|d| d.to_f64());
        let status_str = order_status_to_db(&order.status);
        let tif_str = time_in_force_to_db(&order.time_in_force);
        let filled_avg_val = order.filled_avg_price.map(|p| p.as_f64());
        let commission_val = order.commission.and_then(|c| c.to_f64());
        let pattern_name_str = order.pattern_name.as_ref().map(|p| p.as_str().to_string());
        // Use internal order.id as placeholder broker_order_id to keep the UNIQUE
        // constraint happy before the broker assigns its own id.
        let initial_broker_id = order
            .broker_order_id
            .clone()
            .unwrap_or_else(|| order.id.as_str().to_string());
        sqlx::query!(
            r#"
            INSERT INTO orders (
                account_id, broker_order_id, symbol, side, order_type, quantity,
                filled_quantity, limit_price, stop_price, trailing_percent, status,
                time_in_force, submitted_at, filled_at, filled_avg_price, commission,
                error_message, is_pattern_trade, pattern_name
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.account_id,
            initial_broker_id,
            symbol_str,
            side_str,
            order_type_str,
            quantity_val,
            order.filled_quantity,
            limit_price_val,
            stop_price_val,
            trailing_val,
            status_str,
            tif_str,
            order.submitted_at,
            order.filled_at,
            filled_avg_val,
            commission_val,
            order.error_message,
            order.is_pattern_trade,
            pattern_name_str,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    /// Updates broker order ID in database
    async fn update_broker_id_in_db(
        &self,
        order_id: &OrderId,
        broker_order_id: &str,
        submitted_at: chrono::DateTime<Utc>,
    ) -> Result<(), OrderError> {
        // save_order used the internal order_id as the placeholder broker_order_id,
        // so we can locate the row by that and swap in the real broker id here.
        let internal_id = order_id.as_str();
        sqlx::query!(
            "UPDATE orders SET broker_order_id = ?, submitted_at = ?, status = 'pending' WHERE broker_order_id = ?",
            broker_order_id,
            submitted_at,
            internal_id,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    /// Updates order status in database
    async fn update_order_status_in_db(
        &self,
        order_id: &OrderId,
        status: crate::brokers::traits::OrderStatus,
    ) -> Result<(), OrderError> {
        // Find by broker_order_id (after initial save, internal ID needs mapping)
        // This is a simplified approach - production would use internal_order_id
        let status_str = order_status_to_db(&status);
        let order_id_str = order_id.as_str();
        sqlx::query!(
            "UPDATE orders SET status = ? WHERE broker_order_id = ?",
            status_str,
            order_id_str,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Generates a unique order ID
    fn generate_order_id(&self) -> OrderId {
        let count = self.order_counter.fetch_add(1, Ordering::SeqCst);
        let uuid = Uuid::new_v4();
        OrderId::new(format!("ord-{}-{}", count, uuid.simple()))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Converts OrderStatus to database string
fn order_status_to_db(status: &crate::brokers::traits::OrderStatus) -> &'static str {
    match status {
        crate::brokers::traits::OrderStatus::Pending => "pending",
        crate::brokers::traits::OrderStatus::Accepted => "pending",
        crate::brokers::traits::OrderStatus::PartiallyFilled => "partial",
        crate::brokers::traits::OrderStatus::Filled => "filled",
        crate::brokers::traits::OrderStatus::Cancelled => "cancelled",
        crate::brokers::traits::OrderStatus::Rejected => "rejected",
        crate::brokers::traits::OrderStatus::Expired => "cancelled", // Map expired to cancelled
    }
}

/// Converts TimeInForce to database string
fn time_in_force_to_db(tif: &crate::brokers::traits::TimeInForce) -> &'static str {
    match tif {
        crate::brokers::traits::TimeInForce::Day => "day",
        crate::brokers::traits::TimeInForce::GTC => "gtc",
        crate::brokers::traits::TimeInForce::IOC => "ioc",
        crate::brokers::traits::TimeInForce::FOK => "fok",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{OrderSide, OrderType, TimeInForce};
    use crate::brokers::types::{Quantity, Symbol};
    use crate::config::RiskConfig;
    use crate::orders::broker_adapter::MockBrokerAdapter;
    use crate::risk::types::AccountId;
    use crate::utils::clock::FixedClock;
    use chrono::TimeZone;
    use std::collections::HashMap;

    /// Wednesday 2024-06-12 15:00 UTC == 11:00 EDT — inside market hours
    fn open_market_clock() -> Arc<FixedClock> {
        Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2024, 6, 12, 15, 0, 0).unwrap(),
        ))
    }

    // Mock OrderExecutionProvider for testing
    struct MockOrderExecutionProvider {
        buying_power: Decimal,
    }

    impl MockOrderExecutionProvider {
        fn new(buying_power: Decimal) -> Self {
            Self { buying_power }
        }
    }

    #[async_trait::async_trait]
    impl OrderExecutionProvider for MockOrderExecutionProvider {
        async fn place_order(
            &self,
            _order: crate::brokers::traits::Order,
        ) -> Result<OrderId, crate::utils::errors::BrokerError> {
            Ok(OrderId::new("mock-broker-123"))
        }

        async fn cancel_order(
            &self,
            _order_id: &OrderId,
        ) -> Result<(), crate::utils::errors::BrokerError> {
            Ok(())
        }

        async fn get_order(
            &self,
            _order_id: &OrderId,
        ) -> Result<crate::brokers::traits::Order, crate::utils::errors::BrokerError> {
            use crate::brokers::types::{Price, Quantity, Symbol};

            Ok(crate::brokers::traits::Order {
                id: OrderId::new("mock-order"),
                symbol: Symbol::new("AAPL").unwrap(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: Quantity::new(100).unwrap(),
                limit_price: None,
                stop_price: None,
                trailing_amount: None,
                status: crate::brokers::traits::OrderStatus::Accepted,
                time_in_force: TimeInForce::Day,
                filled_quantity: 0,
                filled_avg_price: None,
                submitted_at: Some(Utc::now()),
                filled_at: None,
            })
        }

        async fn get_open_orders(
            &self,
        ) -> Result<Vec<crate::brokers::traits::Order>, crate::utils::errors::BrokerError> {
            Ok(vec![])
        }

        async fn get_account(&self) -> Result<Account, crate::utils::errors::BrokerError> {
            Ok(Account {
                account_number: "test-account".to_string(),
                buying_power: self.buying_power,
                cash: self.buying_power,
                portfolio_value: self.buying_power,
                day_trade_count: 0,
                is_pattern_day_trader: false,
                equity: self.buying_power,
                last_equity: self.buying_power,
            })
        }

        async fn get_positions(
            &self,
        ) -> Result<Vec<crate::brokers::traits::Position>, crate::utils::errors::BrokerError>
        {
            Ok(vec![])
        }

        async fn get_position(
            &self,
            _symbol: &crate::brokers::types::Symbol,
        ) -> Result<Option<crate::brokers::traits::Position>, crate::utils::errors::BrokerError>
        {
            Ok(None)
        }
    }

    async fn create_test_engine() -> OrderEngine {
        let event_bus = Arc::new(EventBus::new(1000));

        let risk_config = RiskConfig {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 5000.0,
            max_position_size_percent: 10.0,
            max_concurrent_positions: 5,
            enforce_pdt_rule: true,
            pdt_min_equity: 25000.0,
            max_day_trades_under_pdt: 3,
            max_trades_per_pattern_per_day: 5,
            stop_pattern_after_consecutive_losses: 3,
            required_win_rate_for_auto: 60.0,
            min_pattern_sample_size: 20,
            min_account_balance: 1000.0,
            emergency_liquidate_threshold: 500.0,
            pattern_profiles: HashMap::new(),
            confidence_multipliers: vec![],
            pattern_performance_thresholds:
                crate::risk::types::PatternPerformanceThresholds::default(),
        };

        let risk_engine = Arc::new(RiskEngine::new(
            risk_config,
            Arc::clone(&event_bus),
            AccountId::new("test-account"),
            Decimal::new(5000000, 2), // $50,000
        ));

        let broker = Arc::new(MockOrderExecutionProvider::new(Decimal::new(5000000, 2)));
        let broker_adapter = Arc::new(MockBrokerAdapter::new_success());

        // Create in-memory database for testing
        let db_pool = Arc::new(sqlx::SqlitePool::connect(":memory:").await.unwrap());

        // Run migrations
        sqlx::query(include_str!("../db/schema.sql"))
            .execute(&*db_pool)
            .await
            .unwrap();

        // Insert test account
        sqlx::query!("INSERT INTO accounts (broker, account_number, account_type) VALUES ('alpaca', 'test-account', 'paper')")
            .execute(&*db_pool)
            .await
            .unwrap();

        OrderEngine::new(
            broker,
            risk_engine,
            broker_adapter,
            event_bus,
            db_pool,
            1, // account_id
            OrderConfig::default(),
        )
        .with_clock(open_market_clock())
    }

    #[tokio::test]
    async fn test_place_market_order() {
        let engine = create_test_engine().await;

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(result.is_ok());

        let order_id = result.unwrap();
        let order = engine.get_order(&order_id).await.unwrap();

        assert!(order.pre_trade_validation_passed);
        assert!(order.broker_order_id.is_some());
    }

    /// Builds an engine backed by a real `MockBroker` (auto-fill on) so the
    /// submit -> poll -> fill lifecycle can be exercised end to end.
    async fn create_test_engine_with_auto_fill_broker() -> (OrderEngine, Arc<EventBus>) {
        use crate::brokers::mock::{MockBroker, MockConfig};
        use crate::orders::broker_adapter::AlpacaAdapter;

        let event_bus = Arc::new(EventBus::new(1000));

        let risk_engine = Arc::new(RiskEngine::new(
            RiskConfig {
                max_daily_loss_dollars: 500.0,
                max_daily_loss_percent: 2.0,
                halt_trading_on_limit: true,
                max_position_size_dollars: 5000.0,
                max_position_size_percent: 10.0,
                max_concurrent_positions: 5,
                enforce_pdt_rule: true,
                pdt_min_equity: 25000.0,
                max_day_trades_under_pdt: 3,
                max_trades_per_pattern_per_day: 5,
                stop_pattern_after_consecutive_losses: 3,
                required_win_rate_for_auto: 60.0,
                min_pattern_sample_size: 20,
                min_account_balance: 1000.0,
                emergency_liquidate_threshold: 500.0,
                pattern_profiles: HashMap::new(),
                confidence_multipliers: vec![],
                pattern_performance_thresholds:
                    crate::risk::types::PatternPerformanceThresholds::default(),
            },
            Arc::clone(&event_bus),
            AccountId::new("test-account"),
            Decimal::new(5000000, 2),
        ));

        let broker = Arc::new(MockBroker::new(MockConfig {
            auto_fill_orders: true,
            ..MockConfig::default()
        }));
        // AlpacaAdapter is a plain passthrough over any OrderExecutionProvider,
        // so submitted orders land in the mock's own order store and the ids
        // returned by submit_order resolve on the poll path.
        let broker_adapter = Arc::new(AlpacaAdapter::new(broker.clone()));

        let db_pool = Arc::new(sqlx::SqlitePool::connect(":memory:").await.unwrap());
        sqlx::query(include_str!("../db/schema.sql"))
            .execute(&*db_pool)
            .await
            .unwrap();
        sqlx::query!("INSERT INTO accounts (broker, account_number, account_type) VALUES ('alpaca', 'test-account', 'paper')")
            .execute(&*db_pool)
            .await
            .unwrap();

        let engine = OrderEngine::new(
            broker,
            risk_engine,
            broker_adapter,
            Arc::clone(&event_bus),
            db_pool,
            1,
            OrderConfig::default(),
        )
        .with_clock(open_market_clock());

        (engine, event_bus)
    }

    #[tokio::test]
    async fn test_sync_order_statuses_applies_fill_and_publishes_event() {
        let (engine, event_bus) = create_test_engine_with_auto_fill_broker().await;
        let mut receiver = event_bus.subscribe();

        let limit = crate::brokers::types::Price::new(Decimal::new(15000, 2)).unwrap();
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(10).unwrap())
            .limit_price(limit)
            .build()
            .unwrap();

        let order_id = engine.place_order(request).await.unwrap();

        let updated = engine.sync_order_statuses().await.unwrap();
        assert_eq!(updated, 1);

        // Filled orders are terminal, so the order moves to the archive
        let order = engine.get_order(&order_id).await.unwrap();
        assert_eq!(order.status, crate::brokers::traits::OrderStatus::Filled);
        assert_eq!(order.filled_avg_price, Some(limit));
        assert!(engine.get_active_orders().is_empty());

        // An OrderFilled event must have been published for the risk engine
        let mut saw_fill = false;
        while let Ok(event) = receiver.try_recv() {
            if let crate::events::types::MarketEvent::OrderFilled { fill, .. } = event {
                assert_eq!(fill.filled_price, limit);
                saw_fill = true;
            }
        }
        assert!(saw_fill, "expected an OrderFilled event on the bus");
    }

    /// Provider that reports a cumulative partial fill on the first poll and
    /// a cumulative full fill on the second — the shape real brokers use.
    struct PartialThenFullProvider {
        polls: std::sync::atomic::AtomicU64,
    }

    #[async_trait::async_trait]
    impl OrderExecutionProvider for PartialThenFullProvider {
        async fn place_order(
            &self,
            _order: crate::brokers::traits::Order,
        ) -> Result<OrderId, crate::utils::errors::BrokerError> {
            Ok(OrderId::new("partial-broker-1"))
        }

        async fn cancel_order(
            &self,
            _order_id: &OrderId,
        ) -> Result<(), crate::utils::errors::BrokerError> {
            Ok(())
        }

        async fn get_order(
            &self,
            _order_id: &OrderId,
        ) -> Result<crate::brokers::traits::Order, crate::utils::errors::BrokerError> {
            use crate::brokers::types::Price;

            let poll = self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Poll 0: 5 of 10 filled at $150.00. Poll 1+: all 10 filled at a
            // cumulative average of $151.00 (second tranche at $152.00).
            let (status, filled_quantity, avg) = if poll == 0 {
                (
                    crate::brokers::traits::OrderStatus::PartiallyFilled,
                    5,
                    Decimal::new(15000, 2),
                )
            } else {
                (
                    crate::brokers::traits::OrderStatus::Filled,
                    10,
                    Decimal::new(15100, 2),
                )
            };

            Ok(crate::brokers::traits::Order {
                id: OrderId::new("partial-broker-1"),
                symbol: Symbol::new("AAPL").unwrap(),
                side: OrderSide::Buy,
                order_type: OrderType::Limit,
                quantity: Quantity::new(10).unwrap(),
                limit_price: Some(Price::new(Decimal::new(15200, 2)).unwrap()),
                stop_price: None,
                trailing_amount: None,
                status,
                time_in_force: TimeInForce::Day,
                filled_quantity,
                filled_avg_price: Some(Price::new(avg).unwrap()),
                submitted_at: Some(Utc::now()),
                filled_at: Some(Utc::now()),
            })
        }

        async fn get_open_orders(
            &self,
        ) -> Result<Vec<crate::brokers::traits::Order>, crate::utils::errors::BrokerError> {
            Ok(vec![])
        }

        async fn get_account(&self) -> Result<Account, crate::utils::errors::BrokerError> {
            Ok(Account {
                account_number: "test-account".to_string(),
                buying_power: Decimal::new(5000000, 2),
                cash: Decimal::new(5000000, 2),
                portfolio_value: Decimal::new(5000000, 2),
                day_trade_count: 0,
                is_pattern_day_trader: false,
                equity: Decimal::new(5000000, 2),
                last_equity: Decimal::new(5000000, 2),
            })
        }

        async fn get_positions(
            &self,
        ) -> Result<Vec<crate::brokers::traits::Position>, crate::utils::errors::BrokerError>
        {
            Ok(vec![])
        }

        async fn get_position(
            &self,
            _symbol: &crate::brokers::types::Symbol,
        ) -> Result<Option<crate::brokers::traits::Position>, crate::utils::errors::BrokerError>
        {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn test_sync_partial_then_full_fill_books_deltas_not_cumulative() {
        use crate::orders::broker_adapter::AlpacaAdapter;

        let event_bus = Arc::new(EventBus::new(1000));
        let risk_engine = Arc::new(RiskEngine::new(
            RiskConfig {
                max_daily_loss_dollars: 500.0,
                max_daily_loss_percent: 2.0,
                halt_trading_on_limit: true,
                max_position_size_dollars: 5000.0,
                max_position_size_percent: 10.0,
                max_concurrent_positions: 5,
                enforce_pdt_rule: true,
                pdt_min_equity: 25000.0,
                max_day_trades_under_pdt: 3,
                max_trades_per_pattern_per_day: 5,
                stop_pattern_after_consecutive_losses: 3,
                required_win_rate_for_auto: 60.0,
                min_pattern_sample_size: 20,
                min_account_balance: 1000.0,
                emergency_liquidate_threshold: 500.0,
                pattern_profiles: HashMap::new(),
                confidence_multipliers: vec![],
                pattern_performance_thresholds:
                    crate::risk::types::PatternPerformanceThresholds::default(),
            },
            Arc::clone(&event_bus),
            AccountId::new("test-account"),
            Decimal::new(5000000, 2),
        ));

        let broker = Arc::new(PartialThenFullProvider {
            polls: std::sync::atomic::AtomicU64::new(0),
        });
        let broker_adapter = Arc::new(AlpacaAdapter::new(
            broker.clone() as Arc<dyn OrderExecutionProvider>
        ));

        let db_pool = Arc::new(sqlx::SqlitePool::connect(":memory:").await.unwrap());
        sqlx::query(include_str!("../db/schema.sql"))
            .execute(&*db_pool)
            .await
            .unwrap();
        sqlx::query!("INSERT INTO accounts (broker, account_number, account_type) VALUES ('alpaca', 'test-account', 'paper')")
            .execute(&*db_pool)
            .await
            .unwrap();

        let engine = OrderEngine::new(
            broker,
            risk_engine,
            broker_adapter,
            Arc::clone(&event_bus),
            db_pool,
            1,
            OrderConfig::default(),
        )
        .with_clock(open_market_clock());

        let mut receiver = event_bus.subscribe();

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(10).unwrap())
            .limit_price(crate::brokers::types::Price::new(Decimal::new(15200, 2)).unwrap())
            .build()
            .unwrap();
        let order_id = engine.place_order(request).await.unwrap();

        // Poll 1: partial (5 of 10 at $150)
        assert_eq!(engine.sync_order_statuses().await.unwrap(), 1);
        let order = engine.get_order(&order_id).await.unwrap();
        assert_eq!(
            order.filled_quantity, 5,
            "cumulative broker qty must be booked as a 5-share delta"
        );

        // Poll 2: full (10 of 10, cumulative avg $151 => tranche at $152)
        assert_eq!(engine.sync_order_statuses().await.unwrap(), 1);
        let order = engine.get_order(&order_id).await.unwrap();
        assert_eq!(
            order.filled_quantity, 10,
            "must not double-count the first tranche (was 15 before the fix)"
        );
        assert_eq!(
            order.filled_avg_price.map(|p| p.value()),
            Some(Decimal::new(15100, 2)),
            "weighted average must match the broker's cumulative average"
        );

        // Exactly one OrderFilled event, carrying the CUMULATIVE quantity
        let mut fills = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if let crate::events::types::MarketEvent::OrderFilled { fill, .. } = event {
                fills.push(fill);
            }
        }
        assert_eq!(fills.len(), 1, "one terminal OrderFilled event");
        assert_eq!(fills[0].filled_quantity.value(), 10);
        assert_eq!(fills[0].filled_price.value(), Decimal::new(15100, 2));
    }

    #[tokio::test]
    async fn test_sync_order_statuses_no_change_returns_zero() {
        let (engine, _event_bus) = create_test_engine_with_auto_fill_broker().await;

        // No orders placed: nothing to sync
        let updated = engine.sync_order_statuses().await.unwrap();
        assert_eq!(updated, 0);
    }

    #[tokio::test]
    async fn test_place_order_market_closed_weekend_rejected() {
        // Saturday 2024-06-15 15:00 UTC — market is closed
        let engine = create_test_engine()
            .await
            .with_clock(Arc::new(FixedClock::new(
                Utc.with_ymd_and_hms(2024, 6, 15, 15, 0, 0).unwrap(),
            )));

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(matches!(result.unwrap_err(), OrderError::MarketClosed));
    }

    #[tokio::test]
    async fn test_place_order_market_closed_holiday_rejected() {
        // Independence Day 2024 (Thursday July 4) 15:00 UTC == 11:00 EDT —
        // in-session wall time but a market holiday
        let engine = create_test_engine()
            .await
            .with_clock(Arc::new(FixedClock::new(
                Utc.with_ymd_and_hms(2024, 7, 4, 15, 0, 0).unwrap(),
            )));

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(matches!(result.unwrap_err(), OrderError::MarketClosed));
    }

    #[tokio::test]
    async fn test_place_order_market_closed_after_hours_rejected() {
        // Wednesday 2024-06-12 21:00 UTC == 17:00 EDT — after the close
        let engine = create_test_engine()
            .await
            .with_clock(Arc::new(FixedClock::new(
                Utc.with_ymd_and_hms(2024, 6, 12, 21, 0, 0).unwrap(),
            )));

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(matches!(result.unwrap_err(), OrderError::MarketClosed));
    }

    #[tokio::test]
    async fn test_place_limit_order() {
        let engine = create_test_engine().await;

        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(100).unwrap())
            .limit_price(crate::brokers::types::Price::new(Decimal::new(15000, 2)).unwrap())
            .time_in_force(TimeInForce::GTC)
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_insufficient_buying_power() {
        // Create engine with low buying power
        let event_bus = Arc::new(EventBus::new(1000));
        let risk_config = RiskConfig {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 5000.0,
            max_position_size_percent: 10.0,
            max_concurrent_positions: 5,
            enforce_pdt_rule: true,
            pdt_min_equity: 25000.0,
            max_day_trades_under_pdt: 3,
            max_trades_per_pattern_per_day: 5,
            stop_pattern_after_consecutive_losses: 3,
            required_win_rate_for_auto: 60.0,
            min_pattern_sample_size: 20,
            min_account_balance: 1000.0,
            emergency_liquidate_threshold: 500.0,
            pattern_profiles: HashMap::new(),
            confidence_multipliers: vec![],
            pattern_performance_thresholds:
                crate::risk::types::PatternPerformanceThresholds::default(),
        };

        let risk_engine = Arc::new(RiskEngine::new(
            risk_config,
            Arc::clone(&event_bus),
            AccountId::new("test-account"),
            Decimal::new(100000, 2), // $1,000
        ));

        let broker = Arc::new(MockOrderExecutionProvider::new(Decimal::new(100000, 2))); // $1,000
        let broker_adapter = Arc::new(MockBrokerAdapter::new_success());

        let db_pool = Arc::new(sqlx::SqlitePool::connect(":memory:").await.unwrap());
        sqlx::query(include_str!("../db/schema.sql"))
            .execute(&*db_pool)
            .await
            .unwrap();
        sqlx::query!("INSERT INTO accounts (broker, account_number, account_type) VALUES ('alpaca', 'test-account', 'paper')")
            .execute(&*db_pool)
            .await
            .unwrap();

        let engine = OrderEngine::new(
            broker,
            risk_engine,
            broker_adapter,
            event_bus,
            db_pool,
            1,
            OrderConfig::default(),
        )
        .with_clock(open_market_clock());

        // Try to buy $2000 worth of stock with only $1000
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(100).unwrap())
            .limit_price(crate::brokers::types::Price::new(Decimal::new(2000, 2)).unwrap())
            .build()
            .unwrap();

        let result = engine.place_order(request).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrderError::InsufficientBuyingPower { .. }
        ));
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let engine = create_test_engine().await;

        // Place an order
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let order_id = engine.place_order(request).await.unwrap();

        // Cancel the order
        let result = engine.cancel_order(&order_id).await;
        assert!(result.is_ok());

        // Verify order is cancelled
        let order = engine.get_order(&order_id).await.unwrap();
        assert_eq!(order.status, crate::brokers::traits::OrderStatus::Cancelled);
        assert!(order.is_terminal());
    }

    #[tokio::test]
    async fn test_cannot_cancel_filled_order() {
        let engine = create_test_engine().await;

        // Create a filled order
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        let order_id = engine.place_order(request).await.unwrap();

        // Manually update to filled
        engine
            .tracker
            .update_order_status(&order_id, crate::brokers::traits::OrderStatus::Filled, None)
            .await
            .unwrap();

        // Try to cancel
        let result = engine.cancel_order(&order_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrderError::NotCancellable(_, _)
        ));
    }

    #[tokio::test]
    async fn test_get_active_orders() {
        let engine = create_test_engine().await;

        // Place multiple orders
        for i in 0..3 {
            let request = OrderRequest::builder()
                .symbol(Symbol::new("AAPL").unwrap())
                .side(OrderSide::Buy)
                .order_type(OrderType::Market)
                .quantity(Quantity::new(100).unwrap())
                .build()
                .unwrap();

            engine.place_order(request).await.unwrap();
        }

        let active_orders = engine.get_active_orders();
        assert_eq!(active_orders.len(), 3);
    }
}
