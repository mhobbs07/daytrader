# Phase 4A: Order Management System - Detailed Plan

**Date:** January 25, 2026  
**Status:** Planning  
**Timeline:** Weeks 16-17 (2 weeks)  
**Dependencies:** Phase 1 (Infrastructure), Phase 2 (Risk Management), Phase 3 (Pattern Recognition)

---

## Executive Summary

Phase 4A implements the foundational order management system that handles the complete order lifecycle from creation through execution, tracking, and persistence. This phase establishes the infrastructure necessary for automated trading while maintaining strict risk controls and comprehensive audit trails.

### Key Objectives

- Implement type-safe order types and validation
- Build order engine with comprehensive validation pipeline
- Create real-time order status tracking system
- Integrate order persistence with database
- Establish broker adapter for order submission
- Achieve <100ms order placement latency
- Maintain zero risk management violations

### Success Criteria

- ✅ All order types supported (market, limit, stop, stop-limit, trailing stop)
- ✅ Complete order validation pipeline operational
- ✅ Real-time order tracking with event bus integration
- ✅ Database persistence for all orders
- ✅ Broker integration for order submission
- ✅ >85% test coverage
- ✅ Performance targets met (<100ms order placement)

---

## Current State Analysis

### Existing Infrastructure

**Broker Abstraction (Phase 1)**
- ✅ [`OrderExecutionProvider`](../src/brokers/traits.rs:386) trait defined
- ✅ [`Order`](../src/brokers/traits.rs:119) struct exists with basic fields
- ✅ [`OrderSide`](../src/brokers/traits.rs:204), [`OrderType`](../src/brokers/traits.rs:214), [`OrderStatus`](../src/brokers/traits.rs:230) enums defined
- ✅ [`TimeInForce`](../src/brokers/traits.rs:250) enum with Day, GTC, IOC, FOK
- ✅ Type-safe newtypes: [`Symbol`](../src/brokers/types.rs:27), [`Price`](../src/brokers/types.rs:89), [`Quantity`](../src/brokers/types.rs:140), [`OrderId`](../src/brokers/types.rs:234)

**Database Schema (Phase 1)**
- ✅ [`orders`](../src/db/schema.sql:50) table exists with comprehensive fields
- ✅ Support for pattern metadata (`is_pattern_trade`, `pattern_name`)
- ✅ Status tracking (`status`, `filled_quantity`, `filled_avg_price`)
- ✅ Indexes for performance (`idx_orders_account_time`, `idx_orders_status`)

**Event Bus (Phase 1)**
- ✅ Event bus infrastructure ready for order events
- ✅ Zero-copy design with `Arc<T>` established

**Risk Management (Phase 2)**
- ✅ [`RiskEngine`](../src/risk/engine.rs) ready for pre-trade validation
- ✅ Pattern-specific risk limits implemented
- ✅ Daily loss tracking operational

### What Needs to Be Built

**Order Module** (New)
- ❌ Enhanced order types with pattern metadata
- ❌ Order validation pipeline
- ❌ Order engine for lifecycle management
- ❌ Order tracker for real-time status
- ❌ Broker adapter for submission

**Database Integration** (Extension)
- ❌ Order repository CRUD operations
- ❌ Order history queries
- ❌ Pattern trade filtering

**Event System** (Extension)
- ❌ Order event types definition
- ❌ Event publishing on state changes

---

## Architecture Design

### Module Structure

```
src/orders/
├── mod.rs                  # Module exports and public API
├── types.rs                # Order types, enums, and validation (~450 lines)
├── engine.rs               # Order engine core logic (~600 lines)
├── tracker.rs              # Order status tracking (~400 lines)
└── broker_adapter.rs       # Broker integration (~350 lines)
```

### Type System Design

#### Core Order Types

```rust
// Enhanced OrderRequest for internal use
pub struct OrderRequest {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Quantity,
    pub limit_price: Option<Price>,
    pub stop_price: Option<Price>,
    pub trailing_percent: Option<Decimal>,
    pub time_in_force: TimeInForce,
    
    // Pattern metadata (optional)
    pub pattern_name: Option<String>,
    pub signal_id: Option<String>,
    pub signal_confidence: Option<Decimal>,
}

// Internal Order representation (extends broker Order)
pub struct ManagedOrder {
    // Core order fields from broker::Order
    pub id: OrderId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Quantity,
    pub limit_price: Option<Price>,
    pub stop_price: Option<Price>,
    pub trailing_amount: Option<Decimal>,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
    pub filled_quantity: u32,
    pub filled_avg_price: Option<Price>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    
    // Enhanced fields for internal tracking
    pub broker_order_id: Option<String>,
    pub commission: Option<Decimal>,
    pub error_message: Option<String>,
    
    // Pattern tracking
    pub is_pattern_trade: bool,
    pub pattern_name: Option<String>,
    pub signal_id: Option<String>,
    pub signal_confidence: Option<Decimal>,
    
    // Risk tracking
    pub pre_trade_validation_passed: bool,
    pub risk_validation_timestamp: Option<DateTime<Utc>>,
}

// Fill information
pub struct FillInfo {
    pub filled_quantity: Quantity,
    pub filled_price: Price,
    pub commission: Decimal,
    pub timestamp: DateTime<Utc>,
}
```

#### Event Types

```rust
#[derive(Debug, Clone)]
pub enum OrderEvent {
    OrderValidated {
        order_id: OrderId,
        request: Arc<OrderRequest>,
    },
    OrderValidationFailed {
        request: Arc<OrderRequest>,
        reason: String,
    },
    OrderPlaced {
        order: Arc<ManagedOrder>,
    },
    OrderSubmitted {
        order_id: OrderId,
        broker_order_id: String,
        submitted_at: DateTime<Utc>,
    },
    OrderPartiallyFilled {
        order_id: OrderId,
        fill_info: Arc<FillInfo>,
        remaining_quantity: u32,
    },
    OrderFilled {
        order_id: OrderId,
        fill_info: Arc<FillInfo>,
    },
    OrderCancelled {
        order_id: OrderId,
        reason: String,
        cancelled_at: DateTime<Utc>,
    },
    OrderRejected {
        order_id: OrderId,
        reason: String,
        rejected_at: DateTime<Utc>,
    },
    OrderExpired {
        order_id: OrderId,
        expired_at: DateTime<Utc>,
    },
}
```

### Order Engine Architecture

```rust
pub struct OrderEngine {
    // Dependencies
    broker: Arc<dyn OrderExecutionProvider>,
    risk_engine: Arc<RiskEngine>,
    signal_engine: Arc<RwLock<SignalEngine>>,
    event_bus: EventBus,
    db_pool: Arc<SqlitePool>,
    
    // Order tracking (thread-safe concurrent collections)
    pending_orders: Arc<RwLock<HashMap<OrderId, ManagedOrder>>>,
    active_orders: Arc<DashMap<OrderId, ManagedOrder>>,
    
    // Metrics
    metrics: Arc<OrderMetrics>,
    
    // Configuration
    config: OrderConfig,
}

impl OrderEngine {
    /// Place a new order with full validation
    pub async fn place_order(
        &self,
        request: OrderRequest,
    ) -> Result<OrderId, OrderError> {
        // 1. Validate order parameters
        self.validate_order_parameters(&request)?;
        
        // 2. Check market hours
        self.validate_market_hours(&request)?;
        
        // 3. Get account info and validate buying power
        let account = self.broker.get_account().await?;
        self.validate_buying_power(&request, &account)?;
        
        // 4. Run risk validation
        self.risk_engine.validate_order(&request).await?;
        
        // 5. Pattern-specific validation (if applicable)
        if request.pattern_name.is_some() {
            self.validate_pattern_trade(&request).await?;
        }
        
        // 6. Create managed order
        let order_id = self.generate_order_id();
        let managed_order = self.create_managed_order(order_id.clone(), request)?;
        
        // 7. Persist to database
        self.save_order(&managed_order).await?;
        
        // 8. Submit to broker
        let broker_order_id = self.submit_to_broker(&managed_order).await?;
        
        // 9. Update order with broker ID
        managed_order.broker_order_id = Some(broker_order_id.clone());
        managed_order.submitted_at = Some(Utc::now());
        managed_order.status = OrderStatus::Accepted;
        
        // 10. Track order
        self.track_order(managed_order.clone()).await?;
        
        // 11. Publish events
        self.publish_order_placed_event(&managed_order).await;
        
        Ok(order_id)
    }
    
    /// Cancel an existing order
    pub async fn cancel_order(
        &self,
        order_id: &OrderId,
    ) -> Result<(), OrderError> {
        // Retrieve order
        let order = self.get_order(order_id).await?;
        
        // Validate can be cancelled
        if !order.status.is_cancellable() {
            return Err(OrderError::NotCancellable(order_id.clone()));
        }
        
        // Cancel with broker
        if let Some(broker_id) = &order.broker_order_id {
            self.broker.cancel_order(&OrderId::new(broker_id)).await?;
        }
        
        // Update status
        self.update_order_status(
            order_id,
            OrderStatus::Cancelled,
            None,
            None,
        ).await?;
        
        // Publish event
        self.publish_order_cancelled_event(order_id, "User requested").await;
        
        Ok(())
    }
    
    /// Get order by ID
    pub async fn get_order(
        &self,
        order_id: &OrderId,
    ) -> Result<ManagedOrder, OrderError> {
        // Check active orders first
        if let Some(order) = self.active_orders.get(order_id) {
            return Ok(order.clone());
        }
        
        // Check pending orders
        if let Some(order) = self.pending_orders.read().await.get(order_id) {
            return Ok(order.clone());
        }
        
        // Query database
        self.db_pool
            .get_order(order_id)
            .await?
            .ok_or_else(|| OrderError::NotFound(order_id.clone()))
    }
    
    /// Get all open orders
    pub async fn get_open_orders(&self) -> Result<Vec<ManagedOrder>, OrderError> {
        let mut orders = Vec::new();
        
        // Collect from active orders
        for entry in self.active_orders.iter() {
            orders.push(entry.value().clone());
        }
        
        Ok(orders)
    }
    
    /// Update order status (called from broker events)
    pub async fn update_order_status(
        &self,
        order_id: &OrderId,
        status: OrderStatus,
        filled_qty: Option<Quantity>,
        filled_price: Option<Price>,
    ) -> Result<(), OrderError> {
        // Get order
        let mut order = self.get_order(order_id).await?;
        
        // Update status
        order.status = status;
        
        // Update fill information
        if let Some(qty) = filled_qty {
            order.filled_quantity = qty.value();
        }
        if let Some(price) = filled_price {
            order.filled_avg_price = Some(price);
        }
        
        // Update timestamps
        if status == OrderStatus::Filled || status == OrderStatus::PartiallyFilled {
            if order.filled_at.is_none() {
                order.filled_at = Some(Utc::now());
            }
        }
        
        // Persist changes
        self.update_order_in_db(&order).await?;
        
        // Update tracking
        self.update_tracked_order(&order).await?;
        
        // Publish event
        self.publish_status_update_event(&order).await;
        
        Ok(())
    }
}
```

### Validation Pipeline

```rust
impl OrderEngine {
    /// Validate order parameters
    fn validate_order_parameters(
        &self,
        request: &OrderRequest,
    ) -> Result<(), OrderError> {
        // Validate quantity
        if request.quantity.value() == 0 {
            return Err(OrderError::ValidationFailed(
                "Quantity must be positive".into()
            ));
        }
        
        // Validate limit price for limit orders
        if request.order_type == OrderType::Limit 
            && request.limit_price.is_none() {
            return Err(OrderError::ValidationFailed(
                "Limit price required for limit orders".into()
            ));
        }
        
        // Validate stop price for stop orders
        if matches!(request.order_type, OrderType::Stop | OrderType::StopLimit)
            && request.stop_price.is_none() {
            return Err(OrderError::ValidationFailed(
                "Stop price required for stop orders".into()
            ));
        }
        
        // Validate trailing percent
        if request.order_type == OrderType::TrailingStop {
            if let Some(pct) = request.trailing_percent {
                if pct <= Decimal::ZERO || pct >= Decimal::ONE {
                    return Err(OrderError::ValidationFailed(
                        "Trailing percent must be between 0 and 1".into()
                    ));
                }
            } else {
                return Err(OrderError::ValidationFailed(
                    "Trailing percent required for trailing stop orders".into()
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate market hours
    fn validate_market_hours(
        &self,
        request: &OrderRequest,
    ) -> Result<(), OrderError> {
        // Check if market is open
        // (Implementation depends on market hours utility)
        // For now, assume we have a market_hours module
        
        let now = Utc::now();
        if !self.config.market_hours.is_open(now) {
            if self.config.allow_extended_hours {
                // Allow if extended hours are enabled
                Ok(())
            } else {
                Err(OrderError::MarketClosed)
            }
        } else {
            Ok(())
        }
    }
    
    /// Validate buying power
    fn validate_buying_power(
        &self,
        request: &OrderRequest,
        account: &Account,
    ) -> Result<(), OrderError> {
        // Only check for buy orders
        if request.side != OrderSide::Buy {
            return Ok(());
        }
        
        // Estimate order value
        let estimated_price = request.limit_price
            .or(request.stop_price)
            .map(|p| p.value())
            .unwrap_or_else(|| {
                // For market orders, we'd need current price
                // For now, use a conservative estimate or require limit price
                Decimal::ZERO
            });
        
        let order_value = estimated_price * Decimal::from(request.quantity.value());
        
        // Check buying power
        if order_value > account.buying_power {
            return Err(OrderError::InsufficientBuyingPower {
                required: order_value,
                available: account.buying_power,
            });
        }
        
        Ok(())
    }
    
    /// Validate pattern trade
    async fn validate_pattern_trade(
        &self,
        request: &OrderRequest,
    ) -> Result<(), OrderError> {
        if let Some(pattern_name) = &request.pattern_name {
            // Check if pattern is enabled
            let pattern_stats = self.db_pool
                .get_pattern_stats(pattern_name)
                .await?;
            
            if !pattern_stats.is_enabled {
                return Err(OrderError::PatternDisabled(pattern_name.clone()));
            }
            
            // Check daily trade limit for this pattern
            let today = Utc::now().date_naive();
            let today_count = self.db_pool
                .count_pattern_trades_today(pattern_name, today)
                .await?;
            
            if today_count >= pattern_stats.max_trades_per_day as usize {
                return Err(OrderError::PatternDailyLimitReached {
                    pattern: pattern_name.clone(),
                    limit: pattern_stats.max_trades_per_day,
                });
            }
        }
        
        Ok(())
    }
}
```

---

## Implementation Plan

### Sub-Phase 4A-1: Order Types & Models (Days 1-3)

**File:** `src/orders/types.rs` (~450 lines)

**Deliverables:**
1. ✅ Order type definitions
   - `OrderRequest` struct
   - `ManagedOrder` struct
   - `FillInfo` struct
   - `OrderConfig` struct

2. ✅ Validation methods
   - Parameter validation
   - Business rule validation
   - Type conversions

3. ✅ Helper functions
   - Order state checks
   - Value calculations
   - Display formatting

4. ✅ Error types
   - `OrderError` enum
   - Error conversions
   - Context-rich messages

**Tests:**
- Unit tests for order validation
- Property tests for edge cases
- Conversion tests

**Acceptance Criteria:**
- All order types compile without warnings
- Validation catches all invalid states
- Comprehensive error messages
- >90% test coverage

---

### Sub-Phase 4A-2: Order Engine Core (Days 4-7)

**File:** `src/orders/engine.rs` (~600 lines)

**Deliverables:**
1. ✅ Order engine structure
   - Constructor with dependencies
   - Configuration management
   - Metrics tracking

2. ✅ Core methods
   - `place_order()`
   - `cancel_order()`
   - `get_order()`
   - `get_open_orders()`
   - `update_order_status()`

3. ✅ Validation pipeline
   - Parameter validation
   - Market hours check
   - Buying power validation
   - Risk engine integration
   - Pattern trade validation

4. ✅ Event integration
   - Event publishing
   - Event subscriptions

**Tests:**
- Unit tests for validation logic
- Integration tests with risk engine
- Error handling scenarios
- Concurrent order processing

**Acceptance Criteria:**
- All validation steps functional
- Risk engine integration working
- Event publishing operational
- <100ms order placement latency
- >85% test coverage

---

### Sub-Phase 4A-3: Order Tracking (Days 8-9)

**File:** `src/orders/tracker.rs` (~400 lines)

**Deliverables:**
1. ✅ Order tracker structure
   - Concurrent collections
   - Status management
   - History tracking

2. ✅ Tracking methods
   - `track_order()`
   - `update_status()`
   - `get_order()`
   - `archive_order()`

3. ✅ Event subscriptions
   - Broker event handling
   - Status update propagation
   - Archival triggers

**Tests:**
- Concurrent access tests
- Status transition tests
- Event propagation tests
- Archive functionality tests

**Acceptance Criteria:**
- Thread-safe concurrent access
- Correct status transitions
- Event publishing works
- >85% test coverage

---

### Sub-Phase 4A-4: Database Integration (Days 10-11)

**Extension:** `src/db/repositories.rs` (+~200 lines)

**Deliverables:**
1. ✅ Order repository methods
   - `save_order()`
   - `update_order()`
   - `get_order()`
   - `get_orders_for_symbol()`
   - `get_open_orders()`
   - `get_order_history()`
   - `get_pattern_orders()`

2. ✅ Query optimization
   - Proper indexing usage
   - Efficient joins
   - Batch operations

**Tests:**
- CRUD operation tests
- Query performance tests
- Concurrent access tests
- Transaction integrity tests

**Acceptance Criteria:**
- All CRUD operations functional
- <50ms query latency
- Transaction safety
- >85% test coverage

---

### Sub-Phase 4A-5: Broker Adapter (Days 12-14)

**File:** `src/orders/broker_adapter.rs` (~350 lines)

**Deliverables:**
1. ✅ Broker adapter structure
   - Order submission
   - Status synchronization
   - Error handling

2. ✅ Core methods
   - `submit_order()`
   - `cancel_broker_order()`
   - `sync_order_status()`
   - `start_monitoring()`

3. ✅ Format conversions
   - Internal to broker format
   - Broker to internal format
   - Error mapping

4. ✅ WebSocket monitoring
   - Order update subscriptions
   - Real-time status updates
   - Reconnection logic

**Tests:**
- Order submission tests
- Status sync tests
- Format conversion tests
- Error recovery tests

**Acceptance Criteria:**
- Alpaca integration working
- Status updates real-time
- Error recovery operational
- >85% test coverage

---

## Performance Targets

| Operation | Target | Stretch Goal | Measurement |
|-----------|--------|--------------|-------------|
| Order Placement | <100ms | <50ms | End-to-end from API call to broker submission |
| Order Validation | <50ms | <20ms | Complete validation pipeline |
| Status Update | <50ms | <20ms | From broker event to database update |
| Order Retrieval | <10ms | <5ms | Get order by ID from cache |
| Database Write | <50ms | <20ms | Insert/update order in database |

---

## Testing Strategy

### Unit Tests

**Coverage Target:** >90%

**Focus Areas:**
- Order validation logic
- Status transitions
- Format conversions
- Error handling

### Integration Tests

**Test Scenarios:**
1. Complete order lifecycle (place → fill → archive)
2. Order cancellation workflow
3. Partial fill handling
4. Risk validation integration
5. Pattern trade workflow
6. Concurrent order processing
7. Error recovery scenarios

### Performance Tests

**Benchmarks:**
- Order placement latency
- Validation pipeline performance
- Database operation speed
- Concurrent throughput

### Error Scenarios

**Test Cases:**
- Invalid order parameters
- Insufficient buying power
- Market closed
- Risk violations
- Pattern disabled
- Broker rejection
- Network failures
- Database errors

---

## Event Integration

### Event Types

All order events will be published to the event bus:

```rust
use crate::events::types::MarketEvent;

// Publish order events as MarketEvent variants
pub async fn publish_order_placed_event(
    &self,
    order: &ManagedOrder,
) {
    let event = MarketEvent::OrderPlaced {
        order: Arc::new(order.clone()),
    };
    
    if let Err(e) = self.event_bus.publish(event).await {
        tracing::error!("Failed to publish OrderPlaced event: {}", e);
    }
}
```

### Event Subscribers

The order module will subscribe to:
- Broker order status updates
- Fill notifications
- Cancellation confirmations
- Rejection notifications

---

## Database Schema Usage

### Orders Table

The existing [`orders`](../src/db/schema.sql:50) table will be used directly:

```sql
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    broker_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    order_type TEXT NOT NULL CHECK(order_type IN ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop')),
    quantity INTEGER NOT NULL,
    filled_quantity INTEGER DEFAULT 0,
    limit_price REAL,
    stop_price REAL,
    trailing_percent REAL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'filled', 'partial', 'cancelled', 'rejected')),
    time_in_force TEXT DEFAULT 'day' CHECK(time_in_force IN ('day', 'gtc', 'ioc', 'fok')),
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMP,
    filled_avg_price REAL,
    commission REAL DEFAULT 0.0,
    error_message TEXT,
    is_pattern_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);
```

**Key Indexes:**
- `idx_orders_account_time`: Efficient historical queries
- `idx_orders_status`: Fast open order lookups
- `idx_orders_symbol`: Symbol-based filtering

---

## Risk Mitigation

### Technical Risks

| Risk | Mitigation |
|------|-----------|
| Order placement latency | Comprehensive benchmarking, optimization focus |
| Concurrent access bugs | Thread-safe collections, extensive testing |
| Database write performance | Connection pooling, batch operations where possible |
| Broker API failures | Robust error handling, retry logic, fallback mechanisms |

### Business Risks

| Risk | Mitigation |
|------|-----------|
| Risk validation bypass | Multiple validation layers, comprehensive auditing |
| Order submission errors | Comprehensive error handling, manual review for critical errors |
| Database corruption | Transaction-based operations, backup strategies |
| Pattern trade violations | Strict pattern limits, pre-trade validation |

---

## Integration Points

### Phase 1 Integration
- ✅ Use `OrderExecutionProvider` trait for broker interaction
- ✅ Leverage event bus for order events
- ✅ Use database connection pooling
- ✅ Utilize type-safe newtypes

### Phase 2 Integration
- ✅ Call `RiskEngine.validate_order()` before placement
- ✅ Respect pattern-specific limits
- ✅ Integrate with daily loss tracking
- ✅ Respect position sizing limits

### Phase 3 Integration
- ✅ Accept orders from signal generation
- ✅ Propagate pattern metadata
- ✅ Support signal tracking
- ✅ Enable pattern performance feedback

---

## Next Phase Handoff

### To Phase 4B (Position Tracking)

**Deliverables:**
- Order fill events published
- Fill information structured data
- Order-to-position mapping support
- Commission tracking

**APIs:**
```rust
// Phase 4B will subscribe to these events
OrderEvent::OrderFilled { order_id, fill_info }
OrderEvent::OrderPartiallyFilled { order_id, fill_info, remaining_quantity }
```

### To Phase 4C (Automated Execution)

**Deliverables:**
- Order placement API ready
- Risk validation integrated
- Pattern trade support
- Comprehensive error handling

**APIs:**
```rust
// Phase 4C will use this API
pub async fn place_order(
    &self,
    request: OrderRequest,
) -> Result<OrderId, OrderError>;
```

---

## Documentation Requirements

### API Documentation

- All public types documented
- Code examples for common use cases
- Error handling patterns
- Integration examples

### User Guides

1. **Order Placement Guide**
   - Order types and when to use them
   - Parameter requirements
   - Error handling
   - Best practices

2. **Order Management Guide**
   - Order status tracking
   - Cancellation procedures
   - Historical queries
   - Troubleshooting

### Developer Documentation

1. **Architecture Overview**
   - Component interaction
   - Data flow
   - Event publishing

2. **Extension Guide**
   - Adding new order types
   - Custom validation
   - Integration patterns

---

## Success Metrics

### Functional Metrics
- ✅ All 5 order types supported
- ✅ 100% risk validation coverage
- ✅ Pattern trade support operational
- ✅ Real-time status tracking

### Performance Metrics
- ✅ <100ms order placement latency
- ✅ <50ms validation pipeline
- ✅ >50 orders/minute throughput
- ✅ <100MB memory footprint

### Quality Metrics
- ✅ >85% test coverage
- ✅ Zero compiler warnings
- ✅ Zero critical bugs
- ✅ Comprehensive error handling

### Integration Metrics
- ✅ 100% event bus integration
- ✅ Full risk engine integration
- ✅ Complete database persistence
- ✅ Seamless broker integration

---

## Timeline and Milestones

### Week 1 (Days 1-7)

**Milestone 1:** Order Types & Engine Core
- Day 1-3: Order types implementation
- Day 4-7: Order engine core logic

**Deliverables:**
- Order types module complete
- Order engine operational
- Validation pipeline working
- Unit tests passing

### Week 2 (Days 8-14)

**Milestone 2:** Tracking, Persistence, Integration
- Day 8-9: Order tracker implementation
- Day 10-11: Database integration
- Day 12-14: Broker adapter

**Deliverables:**
- Order tracking operational
- Database persistence working
- Broker integration complete
- Integration tests passing

---

## Appendix A: Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("Order validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Insufficient buying power: required ${required}, available ${available}")]
    InsufficientBuyingPower {
        required: Decimal,
        available: Decimal,
    },
    
    #[error("Risk violation: {0}")]
    RiskViolation(#[from] RiskViolation),
    
    #[error("Market closed")]
    MarketClosed,
    
    #[error("Pattern disabled: {0}")]
    PatternDisabled(String),
    
    #[error("Pattern daily limit reached: {pattern} (limit: {limit})")]
    PatternDailyLimitReached {
        pattern: String,
        limit: u32,
    },
    
    #[error("Order not found: {0}")]
    NotFound(OrderId),
    
    #[error("Order not cancellable: {0}")]
    NotCancellable(OrderId),
    
    #[error("Broker rejected order: {0}")]
    BrokerRejection(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    
    #[error("Broker error: {0}")]
    BrokerError(#[from] BrokerError),
}
```

---

## Appendix B: Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfig {
    /// Maximum concurrent orders
    pub max_concurrent_orders: usize,
    
    /// Allow trading during extended hours
    pub allow_extended_hours: bool,
    
    /// Default time in force
    pub default_time_in_force: TimeInForce,
    
    /// Enable automatic order retries
    pub enable_retries: bool,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    
    /// Market hours configuration
    pub market_hours: MarketHoursConfig,
}

impl Default for OrderConfig {
    fn default() -> Self {
        Self {
            max_concurrent_orders: 10,
            allow_extended_hours: false,
            default_time_in_force: TimeInForce::Day,
            enable_retries: true,
            max_retries: 3,
            retry_delay_ms: 1000,
            market_hours: MarketHoursConfig::default(),
        }
    }
}
```

---

**Document Version:** 1.0  
**Last Updated:** January 25, 2026  
**Next Review:** After Sub-Phase 4A-2 completion  
**Status:** Ready for Implementation
