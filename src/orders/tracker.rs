//! Real-time order status tracking
//!
//! This module provides thread-safe tracking of order statuses with
//! efficient concurrent access patterns.

use crate::brokers::types::OrderId;
use crate::events::bus::EventBus;
use crate::events::types::MarketEvent;
use crate::orders::types::{FillInfo, ManagedOrder, OrderError};
use chrono::DateTime;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Real-time order status tracker
///
/// Maintains thread-safe collections of orders in different lifecycle stages:
/// - Active orders (pending, accepted, partially filled)
/// - Archived orders (filled, cancelled, rejected, expired)
///
/// # Concurrency
///
/// Uses DashMap for active orders (lock-free concurrent access) and
/// RwLock for archived orders (read-heavy workload).
///
/// # Examples
///
/// ```no_run
/// use daytrader::orders::tracker::OrderTracker;
/// use daytrader::events::bus::EventBus;
///
/// # async fn example() {
/// let event_bus = EventBus::new(1000);
/// let tracker = OrderTracker::new(Arc::new(event_bus));
///
/// // Track orders, update statuses, etc.
/// # }
/// ```
pub struct OrderTracker {
    /// Active orders (concurrent lock-free access)
    active_orders: Arc<DashMap<OrderId, ManagedOrder>>,
    /// Archived orders (read-heavy, write-rare)
    archived_orders: Arc<RwLock<DashMap<OrderId, ManagedOrder>>>,
    /// Event bus for publishing status updates
    event_bus: Arc<EventBus>,
}

impl OrderTracker {
    /// Creates a new order tracker
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            active_orders: Arc::new(DashMap::new()),
            archived_orders: Arc::new(RwLock::new(DashMap::new())),
            event_bus,
        }
    }

    /// Starts tracking a new order
    ///
    /// Adds the order to active tracking and publishes an OrderPlaced event.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use daytrader::orders::tracker::OrderTracker;
    /// # use daytrader::orders::types::ManagedOrder;
    /// # async fn example(tracker: &OrderTracker, order: ManagedOrder) {
    /// tracker.track_order(order).await;
    /// # }
    /// ```
    pub async fn track_order(&self, order: ManagedOrder) {
        let order_id = order.id.clone();

        info!(
            order_id = %order_id,
            symbol = %order.symbol,
            side = ?order.side,
            quantity = order.quantity.value(),
            "Tracking new order"
        );

        self.active_orders.insert(order_id.clone(), order.clone());

        // Publish OrderPlaced event
        let event_order = crate::events::types::Order {
            id: order.id.clone(),
            symbol: order.symbol.clone(),
            side: match order.side {
                crate::brokers::traits::OrderSide::Buy => crate::events::types::OrderSide::Buy,
                crate::brokers::traits::OrderSide::Sell => crate::events::types::OrderSide::Sell,
            },
            order_type: match order.order_type {
                crate::brokers::traits::OrderType::Limit => crate::events::types::OrderType::Limit,
                _ => crate::events::types::OrderType::Market,
            },
            quantity: order.quantity,
            price: order.limit_price,
            status: match order.status {
                crate::brokers::traits::OrderStatus::Pending => {
                    crate::events::types::OrderStatus::Pending
                }
                crate::brokers::traits::OrderStatus::Accepted => {
                    crate::events::types::OrderStatus::Submitted
                }
                crate::brokers::traits::OrderStatus::PartiallyFilled => {
                    crate::events::types::OrderStatus::PartiallyFilled
                }
                crate::brokers::traits::OrderStatus::Filled => {
                    crate::events::types::OrderStatus::Filled
                }
                crate::brokers::traits::OrderStatus::Cancelled => {
                    crate::events::types::OrderStatus::Cancelled
                }
                crate::brokers::traits::OrderStatus::Rejected => {
                    crate::events::types::OrderStatus::Rejected
                }
                crate::brokers::traits::OrderStatus::Expired => {
                    crate::events::types::OrderStatus::Cancelled
                }
            },
            created_at: order.created_at,
            updated_at: order.updated_at,
        };
        self.event_bus.publish(MarketEvent::OrderPlaced {
            order: Arc::new(event_order),
        });
    }

    /// Updates an order's status
    ///
    /// Handles status transitions and archives orders when they reach
    /// terminal states (filled, cancelled, rejected, expired).
    ///
    /// # Errors
    ///
    /// Returns an error if the order is not found.
    pub async fn update_order_status(
        &self,
        order_id: &OrderId,
        new_status: crate::brokers::traits::OrderStatus,
        fill_info: Option<FillInfo>,
    ) -> Result<(), OrderError> {
        // Get mutable reference to order
        let mut order = self
            .active_orders
            .get_mut(order_id)
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))?;

        let old_status = order.status;
        order.status = new_status;
        order.updated_at = chrono::Utc::now();

        // Apply fill information if provided
        if let Some(fill) = fill_info.as_ref() {
            order.update_with_fill(fill);
        }

        debug!(
            order_id = %order_id,
            old_status = ?old_status,
            new_status = ?new_status,
            "Order status updated"
        );

        // Clone order for event publishing
        let order_clone = order.clone();
        drop(order); // Release lock

        // Publish appropriate event
        match new_status {
            crate::brokers::traits::OrderStatus::Filled => {
                if let Some(fill) = fill_info {
                    // Publish the CUMULATIVE fill from the order's own state:
                    // callers may pass per-tranche deltas (the status poller
                    // does), and consumers must see the complete quantity
                    // exactly once, on the terminal transition.
                    let filled_quantity =
                        crate::brokers::types::Quantity::new(order_clone.filled_quantity)
                            .unwrap_or(fill.filled_quantity);
                    self.event_bus.publish(MarketEvent::OrderFilled {
                        order_id: order_id.clone(),
                        fill: crate::events::types::Fill {
                            order_id: order_id.clone(),
                            filled_quantity,
                            filled_price: order_clone.filled_avg_price.unwrap_or(fill.filled_price),
                            filled_at: fill.timestamp,
                            commission: order_clone.commission.unwrap_or(fill.commission),
                        },
                    });
                }
            }
            crate::brokers::traits::OrderStatus::PartiallyFilled => {
                if let Some(fill) = fill_info {
                    // Use the OrderEvent for internal tracking
                    // The MarketEvent system uses simpler types
                    debug!(
                        order_id = %order_id,
                        filled_quantity = fill.filled_quantity.value(),
                        remaining = order_clone.remaining_quantity(),
                        "Order partially filled"
                    );
                }
            }
            crate::brokers::traits::OrderStatus::Cancelled => {
                self.event_bus.publish(MarketEvent::OrderCancelled {
                    order_id: order_id.clone(),
                    reason: crate::brokers::types::Reason::new("User requested"),
                });
            }
            crate::brokers::traits::OrderStatus::Rejected => {
                self.event_bus.publish(MarketEvent::OrderRejected {
                    order_id: order_id.clone(),
                    reason: crate::brokers::types::Reason::new(
                        order_clone.error_message.as_deref().unwrap_or("Unknown"),
                    ),
                });
            }
            _ => {}
        }

        // Archive if terminal
        if new_status.is_terminal() {
            self.archive_order(order_id).await?;
        }

        Ok(())
    }

    /// Retrieves an order by ID
    ///
    /// Searches active orders first, then archived orders.
    ///
    /// # Errors
    ///
    /// Returns an error if the order is not found.
    pub async fn get_order(&self, order_id: &OrderId) -> Result<ManagedOrder, OrderError> {
        // Check active orders first (most common case)
        if let Some(order) = self.active_orders.get(order_id) {
            return Ok(order.clone());
        }

        // Check archived orders
        let archived = self.archived_orders.read().await;
        archived
            .get(order_id)
            .map(|order| order.clone())
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))
    }

    /// Gets all active orders
    ///
    /// Returns orders that are not in terminal states.
    pub fn get_active_orders(&self) -> Vec<ManagedOrder> {
        self.active_orders
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Gets all archived orders
    ///
    /// Returns orders that are in terminal states.
    pub async fn get_archived_orders(&self) -> Vec<ManagedOrder> {
        let archived = self.archived_orders.read().await;
        archived.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Archives an order (moves from active to archived)
    ///
    /// Called when an order reaches a terminal state.
    async fn archive_order(&self, order_id: &OrderId) -> Result<(), OrderError> {
        let order = self
            .active_orders
            .remove(order_id)
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))?
            .1;

        info!(
            order_id = %order_id,
            status = ?order.status,
            "Archiving order"
        );

        let mut archived = self.archived_orders.write().await;
        archived.insert(order_id.clone(), order);

        Ok(())
    }

    /// Updates order with broker submission details
    ///
    /// Called after successful submission to broker.
    pub fn update_broker_id(
        &self,
        order_id: &OrderId,
        broker_order_id: String,
        submitted_at: DateTime<chrono::Utc>,
    ) -> Result<(), OrderError> {
        let mut order = self
            .active_orders
            .get_mut(order_id)
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))?;

        order.broker_order_id = Some(broker_order_id.clone());
        order.submitted_at = Some(submitted_at);
        order.status = crate::brokers::traits::OrderStatus::Accepted;
        order.updated_at = chrono::Utc::now();

        info!(
            order_id = %order_id,
            broker_order_id = %broker_order_id,
            "Order submitted to broker"
        );

        Ok(())
    }

    /// Updates order with error information
    ///
    /// Called when order submission or processing fails.
    pub fn update_error(
        &self,
        order_id: &OrderId,
        error_message: String,
    ) -> Result<(), OrderError> {
        let mut order = self
            .active_orders
            .get_mut(order_id)
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))?;

        order.error_message = Some(error_message);
        order.updated_at = chrono::Utc::now();

        Ok(())
    }

    /// Marks order validation as passed
    pub fn mark_validation_passed(&self, order_id: &OrderId) -> Result<(), OrderError> {
        let mut order = self
            .active_orders
            .get_mut(order_id)
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))?;

        order.pre_trade_validation_passed = true;
        order.risk_validation_timestamp = Some(chrono::Utc::now());
        order.updated_at = chrono::Utc::now();

        Ok(())
    }

    /// Returns the count of active orders
    #[inline]
    pub fn active_count(&self) -> usize {
        self.active_orders.len()
    }

    /// Returns the count of archived orders
    pub async fn archived_count(&self) -> usize {
        let archived = self.archived_orders.read().await;
        archived.len()
    }

    /// Clears archived orders (use for cleanup/testing)
    pub async fn clear_archived(&self) {
        let mut archived = self.archived_orders.write().await;
        archived.clear();
        info!("Cleared archived orders");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{OrderSide, OrderType, TimeInForce};
    use crate::brokers::types::{Price, Quantity, Symbol};
    use crate::orders::types::OrderRequest;
    use rust_decimal::Decimal;

    fn create_test_order() -> ManagedOrder {
        let request = OrderRequest::builder()
            .symbol(Symbol::new("AAPL").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(100).unwrap())
            .build()
            .unwrap();

        ManagedOrder::from_request(OrderId::new("test-order-1"), request)
    }

    #[tokio::test]
    async fn test_track_order() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        assert_eq!(tracker.active_count(), 1);
        assert!(tracker.get_order(&order_id).await.is_ok());
    }

    #[tokio::test]
    async fn test_update_order_status() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        // Update to accepted
        tracker
            .update_order_status(
                &order_id,
                crate::brokers::traits::OrderStatus::Accepted,
                None,
            )
            .await
            .unwrap();

        let updated = tracker.get_order(&order_id).await.unwrap();
        assert_eq!(
            updated.status,
            crate::brokers::traits::OrderStatus::Accepted
        );
    }

    #[tokio::test]
    async fn test_fill_order() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        // Fill order
        let fill = FillInfo {
            filled_quantity: Quantity::new(100).unwrap(),
            filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
            commission: Decimal::new(1, 0),
            timestamp: chrono::Utc::now(),
        };

        tracker
            .update_order_status(
                &order_id,
                crate::brokers::traits::OrderStatus::Filled,
                Some(fill),
            )
            .await
            .unwrap();

        // Order should be archived
        assert_eq!(tracker.active_count(), 0);
        assert_eq!(tracker.archived_count().await, 1);

        // Should still be retrievable
        let filled_order = tracker.get_order(&order_id).await.unwrap();
        assert_eq!(
            filled_order.status,
            crate::brokers::traits::OrderStatus::Filled
        );
        assert_eq!(filled_order.filled_quantity, 100);
    }

    #[tokio::test]
    async fn test_partial_fill() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        // Partial fill
        let fill = FillInfo {
            filled_quantity: Quantity::new(50).unwrap(),
            filled_price: Price::new(Decimal::new(15000, 2)).unwrap(),
            commission: Decimal::new(1, 0),
            timestamp: chrono::Utc::now(),
        };

        tracker
            .update_order_status(
                &order_id,
                crate::brokers::traits::OrderStatus::PartiallyFilled,
                Some(fill),
            )
            .await
            .unwrap();

        // Should still be active
        assert_eq!(tracker.active_count(), 1);
        assert_eq!(tracker.archived_count().await, 0);

        let partial_order = tracker.get_order(&order_id).await.unwrap();
        assert_eq!(partial_order.filled_quantity, 50);
        assert_eq!(partial_order.remaining_quantity(), 50);
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        // Cancel order
        tracker
            .update_order_status(
                &order_id,
                crate::brokers::traits::OrderStatus::Cancelled,
                None,
            )
            .await
            .unwrap();

        // Should be archived
        assert_eq!(tracker.active_count(), 0);
        assert_eq!(tracker.archived_count().await, 1);

        let cancelled_order = tracker.get_order(&order_id).await.unwrap();
        assert_eq!(
            cancelled_order.status,
            crate::brokers::traits::OrderStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn test_update_broker_id() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        let broker_id = "broker-order-123";
        let submitted_at = chrono::Utc::now();

        tracker
            .update_broker_id(&order_id, broker_id.to_string(), submitted_at)
            .unwrap();

        let updated = tracker.get_order(&order_id).await.unwrap();
        assert_eq!(updated.broker_order_id, Some(broker_id.to_string()));
        assert_eq!(
            updated.status,
            crate::brokers::traits::OrderStatus::Accepted
        );
        assert!(updated.submitted_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_validation_passed() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let order = create_test_order();
        let order_id = order.id.clone();

        tracker.track_order(order).await;

        tracker.mark_validation_passed(&order_id).unwrap();

        let validated = tracker.get_order(&order_id).await.unwrap();
        assert!(validated.pre_trade_validation_passed);
        assert!(validated.risk_validation_timestamp.is_some());
    }

    #[tokio::test]
    async fn test_order_not_found() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = OrderTracker::new(event_bus);

        let result = tracker.get_order(&OrderId::new("non-existent")).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OrderError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let event_bus = Arc::new(EventBus::new(100));
        let tracker = Arc::new(OrderTracker::new(event_bus));

        // Track multiple orders concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let tracker_clone = Arc::clone(&tracker);
            let handle = tokio::spawn(async move {
                let mut order = create_test_order();
                order.id = OrderId::new(format!("order-{}", i));
                tracker_clone.track_order(order).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(tracker.active_count(), 10);
    }
}
