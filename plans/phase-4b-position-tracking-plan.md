# Phase 4B: Position Tracking & P&L - Planning Document

**Date:** January 25, 2026  
**Status:** Planning  
**Version:** 1.0  
**Timeline:** Week 18 (5 days)  
**Dependencies:** Phase 4A (Order Management) ✅ COMPLETE

---

## Executive Summary

Phase 4B implements real-time position tracking with accurate profit/loss calculations, completing the foundation for portfolio management and automated trading. This phase builds on the completed Phase 4A order management system to provide comprehensive position monitoring, P&L tracking, and position lifecycle management.

###

 Core Objectives

- Implement real-time position tracking system
- Build accurate P&L calculation engine
- Create position management workflows
- Integrate with database for persistence
- Establish event-driven position updates
- Achieve <50ms position update latency

### Success Criteria

- ✅ Real-time position tracking operational
- ✅ Accurate P&L calculations (unrealized and realized)
- ✅ Position closure workflows functional
- ✅ Database persistence for all positions
- ✅ Event integration with order fills
- ✅ >85% test coverage
- ✅ Performance targets met (<50ms updates)

---

## Context

### Phase 4A Completion Status ✅

Phase 4A (Order Management) has been successfully completed, providing:
- Complete order lifecycle management
- Order validation and risk checking
- Order status tracking with event bus integration
- Database persistence for all orders
- Broker integration for order submission
- Fill event notifications

**Key Deliverables from Phase 4A:**
- [`src/orders/types.rs`](../src/orders/types.rs) - Order type definitions
- [`src/orders/engine.rs`](../src/orders/engine.rs) - Order engine core
- [`src/orders/tracker.rs`](../src/orders/tracker.rs) - Order status tracking
- [`src/orders/broker_adapter.rs`](../src/orders/broker_adapter.rs) - Broker integration

### Phase 4B Scope

Phase 4B focuses exclusively on position tracking and P&L management:
- Position creation from order fills
- Real-time position updates
- P&L calculations (unrealized and realized)
- Position closure workflows
- Historical position tracking

**Out of Scope for Phase 4B:**
- Automated execution (Phase 4C)
- Advanced order types (Phase 4A)
- UI components (Phase 5)

---

## Architecture Design

### Module Structure

```
src/positions/
├── mod.rs                  # Module exports and public API
├── tracker.rs              # Position tracking (~550 lines)
└── pnl.rs                  # P&L calculator (~400 lines)
```

### Position Tracker Design

```rust
pub struct PositionTracker {
    // Active positions (thread-safe concurrent access)
    positions: Arc<DashMap<Symbol, Position>>,
    
    // Dependencies
    broker: Arc<dyn OrderExecutionProvider>,
    event_bus: EventBus,
    db_pool: Arc<SqlitePool>,
    
    // P&L calculation
    pnl_calculator: Arc<PnLCalculator>,
    
    // Metrics and monitoring
    metrics: Arc<PositionMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: Symbol,
    pub quantity: Quantity,
    pub side: PositionSide,
    pub avg_entry_price: Price,
    pub current_price: Price,
    pub market_value: Decimal,
    pub cost_basis: Decimal,
    pub unrealized_pl: Decimal,
    pub unrealized_pl_percent: Decimal,
    pub realized_pl: Decimal,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Trade tracking
    pub entry_orders: Vec<OrderId>,
    pub exit_orders: Vec<OrderId>,
    
    // Pattern tracking (optional)
    pub pattern_name: Option<String>,
    pub signal_confidence: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PositionSide {
    Long,   // Bought shares
    Short,  // Sold short (future support)
}
```

### P&L Calculator Design

```rust
pub struct PnLCalculator {
    config: PnLConfig,
}

#[derive(Debug, Clone)]
pub struct PnLConfig {
    /// Include commissions in P&L calculation
    pub include_commissions: bool,
    
    /// Tax lot method (FIFO, LIFO, Average)
    pub tax_lot_method: TaxLotMethod,
}

#[derive(Debug, Clone, Copy)]
pub enum TaxLotMethod {
    FIFO,     // First In, First Out
    LIFO,     // Last In, First Out
    Average,  // Average cost basis
}

#[derive(Debug, Clone)]
pub struct PortfolioPnL {
    pub total_value: Decimal,
    pub total_cost: Decimal,
    pub unrealized_pl: Decimal,
    pub unrealized_pl_percent: Decimal,
    pub realized_pl_today: Decimal,
    pub position_count: usize,
}
```

---

## Sub-Phase 4B-1: Position Management (Week 18, Days 1-3)

### 6. Position Tracker Implementation

**File:** `src/positions/tracker.rs` (~550 lines)

**Core Functionality:**

```rust
impl PositionTracker {
    /// Update position based on filled order
    pub async fn update_from_fill(
        &self,
        order: &Order,
        fill_info: &FillInfo,
    ) -> Result<(), PositionError> {
        let symbol = &order.symbol;
        
        // Get or create position
        let mut position = self.positions
            .entry(symbol.clone())
            .or_insert_with(|| Position::new(symbol.clone()));
        
        match order.side {
            OrderSide::Buy => {
                // Increase position
                let new_quantity = position.quantity.value() + fill_info.filled_quantity.value();
                let new_cost = (position.avg_entry_price.value() * Decimal::from(position.quantity.value()))
                    + (fill_info.filled_price.value() * Decimal::from(fill_info.filled_quantity.value()));
                
                position.quantity = Quantity::new(new_quantity);
                position.avg_entry_price = Price::new(new_cost / Decimal::from(new_quantity));
                position.cost_basis = new_cost;
                position.side = PositionSide::Long;
                position.entry_orders.push(order.id.clone());
            }
            OrderSide::Sell => {
                // Decrease or close position
                let new_quantity = position.quantity.value().saturating_sub(fill_info.filled_quantity.value());
                
                if new_quantity == 0 {
                    // Position fully closed
                    let realized_pl = self.pnl_calculator.calculate_realized_pl(
                        position.avg_entry_price,
                        fill_info.filled_price,
                        fill_info.filled_quantity,
                        fill_info.commission,
                    );
                    
                    position.realized_pl = realized_pl;
                    
                    // Archive and remove from active positions
                    self.archive_position(position.clone()).await?;
                    self.positions.remove(symbol);
                    
                    // Publish position closed event
                    self.publish_position_closed_event(symbol, realized_pl).await;
                } else {
                    // Partial close
                    position.quantity = Quantity::new(new_quantity);
                    position.exit_orders.push(order.id.clone());
                }
            }
        }
        
        // Update timestamp
        position.updated_at = Utc::now();
        
        // Persist to database
        self.save_position(&position).await?;
        
        // Publish position updated event
        self.publish_position_updated_event(&position).await;
        
        Ok(())
    }
    
    /// Update position with current market price
    pub async fn update_market_price(
        &self,
        symbol: &Symbol,
        price: Price,
    ) -> Result<(), PositionError> {
        if let Some(mut position) = self.positions.get_mut(symbol) {
            position.current_price = price;
            
            // Recalculate P&L
            position.unrealized_pl = self.pnl_calculator.calculate_unrealized_pl(
                &position,
                price,
            );
            position.unrealized_pl_percent = self.pnl_calculator.calculate_unrealized_pl_percent(
                &position,
                price,
            );
            position.market_value = price.value() * Decimal::from(position.quantity.value());
            position.updated_at = Utc::now();
            
            // Persist to database
            self.save_position(&position).await?;
            
            // Publish price update event
            self.publish_price_updated_event(symbol, price, position.unrealized_pl).await;
        }
        
        Ok(())
    }
    
    /// Get current position for symbol
    pub async fn get_position(
        &self,
        symbol: &Symbol,
    ) -> Result<Option<Position>, PositionError> {
        Ok(self.positions.get(symbol).map(|p| p.clone()))
    }
    
    /// Get all open positions
    pub async fn get_all_positions(&self) -> Result<Vec<Position>, PositionError> {
        let positions: Vec<Position> = self.positions
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        
        Ok(positions)
    }
    
    /// Close position (create exit order)
    pub async fn close_position(
        &self,
        symbol: &Symbol,
        reason: CloseReason,
    ) -> Result<OrderId, PositionError> {
        let position = self.get_position(symbol).await?
            .ok_or_else(|| PositionError::PositionNotFound(symbol.clone()))?;
        
        // Create market order to close position
        let order_request = OrderRequest {
            symbol: symbol.clone(),
            side: OrderSide::Sell,  // Always sell to close long positions
            order_type: OrderType::Market,
            quantity: position.quantity,
            limit_price: None,
            stop_price: None,
            trailing_percent: None,
            time_in_force: TimeInForce::Day,
            pattern_name: position.pattern_name.clone(),
            signal_id: None,
            signal_confidence: None,
        };
        
        // Place order through order engine
        let order_id = self.order_engine.place_order(order_request).await?;
        
        tracing::info!(
            "Closing position for {} (reason: {:?}), order ID: {}",
            symbol,
            reason,
            order_id
        );
        
        Ok(order_id)
    }
    
    /// Sync positions from broker
    pub async fn sync_from_broker(&self) -> Result<(), PositionError> {
        let broker_positions = self.broker.get_positions().await?;
        
        for broker_pos in broker_positions {
            let mut position = Position {
                symbol: broker_pos.symbol,
                quantity: Quantity::new(broker_pos.quantity.abs() as u32),
                side: if broker_pos.quantity > 0 {
                    PositionSide::Long
                } else {
                    PositionSide::Short
                },
                avg_entry_price: Price::new(broker_pos.avg_entry_price),
                current_price: Price::new(broker_pos.current_price),
                market_value: broker_pos.market_value,
                cost_basis: broker_pos.avg_entry_price * Decimal::from(broker_pos.quantity.abs()),
                unrealized_pl: broker_pos.unrealized_pl,
                unrealized_pl_percent: broker_pos.unrealized_pl / broker_pos.market_value,
                realized_pl: Decimal::ZERO,
                opened_at: Utc::now(),  // Unknown from broker
                updated_at: Utc::now(),
                entry_orders: Vec::new(),
                exit_orders: Vec::new(),
                pattern_name: None,
                signal_confidence: None,
            };
            
            self.positions.insert(position.symbol.clone(), position.clone());
            self.save_position(&position).await?;
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloseReason {
    Manual,
    StopLoss,
    TakeProfit,
    RiskViolation,
    PatternExit,
    EndOfDay,
}
```

**Position Events:**

```rust
pub enum PositionEvent {
    PositionOpened {
        position: Arc<Position>,
    },
    PositionUpdated {
        symbol: Symbol,
        old_quantity: Quantity,
        new_quantity: Quantity,
        unrealized_pl: Decimal,
    },
    PositionClosed {
        symbol: Symbol,
        realized_pl: Decimal,
        holding_period: Duration,
    },
    PriceUpdated {
        symbol: Symbol,
        price: Price,
        unrealized_pl: Decimal,
    },
}
```

**Deliverables:**
- ✅ Real-time position tracking
- ✅ Position updates from order fills
- ✅ Market price updates integration
- ✅ Position closure logic
- ✅ Broker position synchronization

**Tests:**
- Position opening from buy order
- Position averaging (multiple entries)
- Position closing scenarios
- Concurrent price updates
- Broker sync validation

---

### 7. P&L Calculator Implementation

**File:** `src/positions/pnl.rs` (~400 lines)

**Implementation:**

```rust
impl PnLCalculator {
    /// Calculate unrealized P&L
    pub fn calculate_unrealized_pl(
        &self,
        position: &Position,
        current_price: Price,
    ) -> Decimal {
        let market_value = current_price.value() * Decimal::from(position.quantity.value());
        let unrealized_pl = market_value - position.cost_basis;
        
        unrealized_pl
    }
    
    /// Calculate unrealized P&L percent
    pub fn calculate_unrealized_pl_percent(
        &self,
        position: &Position,
        current_price: Price,
    ) -> Decimal {
        if position.cost_basis == Decimal::ZERO {
            return Decimal::ZERO;
        }
        
        let unrealized_pl = self.calculate_unrealized_pl(position, current_price);
        (unrealized_pl / position.cost_basis) * Decimal::from(100)
    }
    
    /// Calculate realized P&L from position close
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
    
    /// Calculate average entry price for position averaging
    pub fn calculate_avg_entry(
        &self,
        existing_position: &Position,
        new_fill: &FillInfo,
    ) -> Price {
        let existing_cost = existing_position.avg_entry_price.value() 
            * Decimal::from(existing_position.quantity.value());
        let new_cost = new_fill.filled_price.value() 
            * Decimal::from(new_fill.filled_quantity.value());
        let total_cost = existing_cost + new_cost;
        
        let total_quantity = existing_position.quantity.value() + new_fill.filled_quantity.value();
        
        Price::new(total_cost / Decimal::from(total_quantity))
    }
    
    /// Calculate portfolio-level P&L
    pub fn calculate_portfolio_pl(
        &self,
        positions: &[Position],
    ) -> PortfolioPnL {
        let mut total_value = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;
        let mut unrealized_pl = Decimal::ZERO;
        
        for position in positions {
            total_value += position.market_value;
            total_cost += position.cost_basis;
            unrealized_pl += position.unrealized_pl;
        }
        
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
            realized_pl_today: Decimal::ZERO,  // Calculated separately
            position_count: positions.len(),
        }
    }
}
```

**Deliverables:**
- ✅ Accurate P&L calculations
- ✅ Support for multiple tax lot methods
- ✅ Commission inclusion logic
- ✅ Portfolio-level aggregation

**Tests:**
- P&L calculation accuracy
- Tax lot method verification
- Position averaging scenarios
- Edge cases (zero quantity, negative prices)

---

## Sub-Phase 4B-2: Database & Event Integration (Week 18, Days 4-5)

### 8. Position Repository

**File:** `src/db/repositories.rs` (extend existing, ~180 lines added)

**Implementation:**

```rust
impl Repository {
    /// Save or update position
    pub async fn upsert_position(&self, position: &Position) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            INSERT INTO positions (
                account_id, symbol, quantity, avg_entry_price, current_price,
                market_value, unrealized_pl, unrealized_pl_percent, opened_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(account_id, symbol) DO UPDATE SET
                quantity = excluded.quantity,
                avg_entry_price = excluded.avg_entry_price,
                current_price = excluded.current_price,
                market_value = excluded.market_value,
                unrealized_pl = excluded.unrealized_pl,
                unrealized_pl_percent = excluded.unrealized_pl_percent,
                updated_at = excluded.updated_at
            "#,
            1,  // Default account ID
            position.symbol.as_str(),
            position.quantity.value(),
            position.avg_entry_price.value(),
            position.current_price.value(),
            position.market_value,
            position.unrealized_pl,
            position.unrealized_pl_percent,
            position.opened_at,
            position.updated_at,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get position by symbol
    pub async fn get_position(
        &self,
        symbol: &Symbol,
    ) -> Result<Option<Position>, DatabaseError> {
        let record = sqlx::query!(
            r#"
            SELECT * FROM positions
            WHERE account_id = ? AND symbol = ?
            "#,
            1,
            symbol.as_str(),
        )
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(record.map(|r| Position {
            symbol: Symbol::new(&r.symbol),
            quantity: Quantity::new(r.quantity as u32),
            side: PositionSide::Long,
            avg_entry_price: Price::new(Decimal::from_f64(r.avg_entry_price).unwrap()),
            current_price: Price::new(Decimal::from_f64(r.current_price.unwrap_or(0.0)).unwrap()),
            market_value: Decimal::from_f64(r.market_value.unwrap_or(0.0)).unwrap(),
            cost_basis: Decimal::from_f64(r.avg_entry_price).unwrap() * Decimal::from(r.quantity),
            unrealized_pl: Decimal::from_f64(r.unrealized_pl.unwrap_or(0.0)).unwrap(),
            unrealized_pl_percent: Decimal::from_f64(r.unrealized_pl_percent.unwrap_or(0.0)).unwrap(),
            realized_pl: Decimal::ZERO,
            opened_at: r.opened_at,
            updated_at: r.updated_at,
            entry_orders: Vec::new(),
            exit_orders: Vec::new(),
            pattern_name: None,
            signal_confidence: None,
        }))
    }
    
    /// Get all open positions
    pub async fn get_open_positions(&self) -> Result<Vec<Position>, DatabaseError> {
        let records = sqlx::query!(
            r#"
            SELECT * FROM positions
            WHERE account_id = ?
            ORDER BY symbol
            "#,
            1,
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(records.into_iter().map(|r| /* convert to Position */).collect())
    }
    
    /// Close position (archive to history)
    pub async fn close_position(
        &self,
        symbol: &Symbol,
        realized_pl: Decimal,
        closed_at: DateTime<Utc>,
    ) -> Result<(), DatabaseError> {
        // Archive to positions_history table
        sqlx::query!(
            r#"
            INSERT INTO positions_history
            SELECT *, ?, ? FROM positions
            WHERE account_id = ? AND symbol = ?
            "#,
            realized_pl,
            closed_at,
            1,
            symbol.as_str(),
        )
        .execute(&self.pool)
        .await?;
        
        // Delete from active positions
        sqlx::query!(
            r#"
            DELETE FROM positions
            WHERE account_id = ? AND symbol = ?
            "#,
            1,
            symbol.as_str(),
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

**Position Schema (already exists):**

```sql
CREATE TABLE positions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_entry_price REAL NOT NULL,
    current_price REAL,
    market_value REAL,
    unrealized_pl REAL,
    unrealized_pl_percent REAL,
    opened_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, symbol)
);
```

**Deliverables:**
- ✅ Position CRUD operations
- ✅ Historical position tracking
- ✅ Daily P&L queries
- ✅ Efficient indexing strategy

**Tests:**
- Position persistence
- Concurrent upserts
- Historical queries
- P&L aggregation accuracy

---

## Performance Targets

| Operation | Target | Stretch Goal | Measurement |
|-----------|--------|--------------|-------------|
| Position Update | <50ms | <20ms | From fill event to database update |
| P&L Calculation | <10ms | <5ms | Single position calculation |
| Portfolio P&L | <100ms | <50ms | All positions aggregation |
| Position Retrieval | <10ms | <5ms | Get position by symbol |
| Database Write | <50ms | <20ms | Insert/update position |

---

## Testing Strategy

### Unit Tests

**Coverage Target:** >90%

**Focus Areas:**
- Position calculations
- P&L accuracy
- Tax lot methods
- Edge cases

### Integration Tests

**Test Scenarios:**
1. Position creation from buy order
2. Position averaging (multiple buys)
3. Partial position close
4. Full position close
5. Real-time price updates
6. Broker position synchronization
7. Concurrent position updates

### Performance Tests

**Benchmarks:**
- Position update latency
- P&L calculation speed
- Database operation performance
- Concurrent update throughput

---

## Integration Points

### Phase 1 Integration
- ✅ Use event bus for position events
- ✅ Leverage broker abstraction for position queries
- ✅ Use database connection pooling

### Phase 2 Integration
- ✅ Provide position data for risk calculations
- ✅ Support position size limits
- ✅ Enable daily P&L tracking

### Phase 4A Integration
- ✅ Subscribe to order fill events
- ✅ Update positions on order completion
- ✅ Link positions to orders

### Phase 4C Preview
- Position tracking enables automated strategy performance
- P&L data drives strategy enable/disable decisions
- Real-time position monitoring for risk management

---

## Success Metrics

### Functional Requirements
- ✅ Real-time position tracking operational
- ✅ Accurate P&L calculations (within $0.01)
- ✅ Position closure workflows functional
- ✅ Database persistence complete
- ✅ Event integration working

### Quality Requirements
- ✅ Zero critical bugs
- ✅ >85% test coverage
- ✅ All performance targets met
- ✅ Zero compiler warnings
- ✅ Comprehensive error handling

### Integration Requirements
- ✅ Seamless order fill integration
- ✅ Event bus integration
- ✅ Database persistence
- ✅ Broker synchronization

---

## Risk Assessment & Mitigation

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| P&L calculation accuracy | Low | High | Extensive testing, validation against broker |
| Concurrent update conflicts | Medium | Medium | DashMap for thread-safe access, optimistic locking |
| Database write performance | Low | Medium | Connection pooling, batch operations |
| Broker sync discrepancies | Medium | Medium | Regular reconciliation, audit logging |

### Mitigated Risks
- **Data Consistency**: Transaction-based operations, atomic updates
- **Performance Degradation**: Benchmarking suite, regression tests
- **Calculation Errors**: Property-based testing, edge case coverage

---

## Next Phase Handoff

### To Phase 4C (Automated Execution)

**Deliverables:**
- Real-time position data API
- P&L calculation services
- Position risk assessment
- Portfolio-level metrics

**APIs:**
```rust
// Phase 4C will use these APIs
position_tracker.get_all_positions().await?;
position_tracker.get_position(&symbol).await?;
pnl_calculator.calculate_portfolio_pl(&positions);
```

---

## Documentation Requirements

### API Documentation
- All public types documented
- Code examples for common use cases
- Integration patterns
- Error handling examples

### User Guides
1. **Position Tracking Guide**
   - Position lifecycle
   - P&L methodology
   - Real-time monitoring

2. **Integration Guide**
   - Event subscriptions
   - Database queries
   - API usage patterns

---

**Document Version:** 1.0  
**Last Updated:** January 25, 2026  
**Part of:** [Phase 4: Order Execution](./phase-4-order-execution-plan.md)  
**Status:** Ready for Implementation
