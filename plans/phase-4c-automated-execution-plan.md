# Phase 4C: Automated Execution Engine - Planning Document

**Date:** January 25, 2026  
**Status:** Planning  
**Version:** 1.0  
**Timeline:** Weeks 19-20 (2 weeks)  
**Dependencies:** Phase 4A (Order Management) ✅, Phase 4B (Position Tracking) - Planning

---

## Executive Summary

Phase 4C implements the automated execution engine that transforms pattern signals into actual trades, completing the core automated trading pipeline. This phase builds on the completed Phase 4A order management and planned Phase 4B position tracking to create a production-ready automated trading system with pattern-specific controls and performance monitoring.

**Note:** This document exceeds the typical 600-line target for planning documents. Per project rules: "Exceptions: Tightly coupled concepts may exceed limits if well-documented." The execution strategies, engine core, performance tracking, and integration testing form a tightly coupled system that benefits from unified documentation.

### Core Objectives

- Implement execution strategies (conservative, aggressive)
- Build automated execution engine with signal processing
- Create pattern performance tracking system
- Establish pattern automation controls
- Integrate comprehensive testing framework
- Achieve <200ms signal-to-order latency

### Success Criteria

- ✅ Multiple execution strategies implemented
- ✅ Signal-to-order pipeline operational
- ✅ Pattern-specific automation controls functional
- ✅ Performance tracking and auto-enable/disable logic working
- ✅ Manual review workflow supported
- ✅ >85% test coverage
- ✅ Performance targets met (<200ms latency)

---

## Context

### Completed Phases

**Phase 4A: Order Management ✅ COMPLETE**
- Complete order lifecycle management
- Order validation and risk checking
- Order status tracking with events
- Database persistence
- Broker integration

**Key Deliverables:**
- [`src/orders/engine.rs`](../src/orders/engine.rs) - Order placement API
- [`src/orders/tracker.rs`](../src/orders/tracker.rs) - Order status tracking
- Order fill event notifications

**Phase 4B: Position Tracking - Planning**
- Real-time position monitoring
- P&L calculations
- Position management

### Phase 4C Scope

Phase 4C focuses exclusively on automated execution:
- Signal-to-order conversion strategies
- Automated execution engine
- Pattern performance tracking
- Automation enable/disable logic
- Manual review workflows

**Out of Scope for Phase 4C:**
- Manual order placement (Phase 4A)
- Position display UI (Phase 5)
- Backtesting (Phase 6)

---

## Architecture Design

### Module Structure

```
src/execution/
├── mod.rs                  # Module exports and public API
├── strategies.rs           # Execution strategies (~500 lines)
├── engine.rs               # Execution engine core (~700 lines)
└── performance.rs          # Performance tracker (~450 lines)
```

### Execution Strategy Design

```rust
pub trait ExecutionStrategy: Send + Sync {
    /// Determine if signal should trigger order
    fn should_execute(&self, signal: &TradingSignal, context: &ExecutionContext) -> bool;
    
    /// Calculate position size
    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_params: &RiskConfig,
    ) -> Result<Quantity, ExecutionError>;
    
    /// Determine order type
    fn determine_order_type(
        &self,
        signal: &TradingSignal,
        market_context: &MarketContext,
    ) -> OrderType;
    
    /// Generate order request from signal
    fn create_order_request(
        &self,
        signal: &TradingSignal,
        position_size: Quantity,
        order_type: OrderType,
    ) -> OrderRequest;
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub account: Account,
    pub current_positions: Vec<Position>,
    pub daily_metrics: DailyRiskMetrics,
    pub pattern_stats: HashMap<String, PatternStats>,
    pub market_hours: MarketHours,
}
```

---

## Sub-Phase 4C-1: Signal-to-Order Conversion (Week 19, Days 1-2)

### 9. Execution Strategies Implementation

**File:** `src/execution/strategies.rs` (~500 lines)

#### Conservative Strategy

```rust
/// Conservative strategy: Market orders with fixed position sizing
pub struct ConservativeStrategy {
    config: ConservativeConfig,
}

#[derive(Debug, Clone)]
pub struct ConservativeConfig {
    pub min_confidence: Decimal,  // e.g., 0.75
    pub position_size_percent: Decimal,  // e.g., 0.05 (5% of portfolio)
    pub max_positions: usize,  // e.g., 5
}

impl ExecutionStrategy for ConservativeStrategy {
    fn should_execute(&self, signal: &TradingSignal, context: &ExecutionContext) -> bool {
        // Check confidence threshold
        if signal.confidence < self.config.min_confidence {
            return false;
        }
        
        // Check max positions limit
        if context.current_positions.len() >= self.config.max_positions {
            return false;
        }
        
        // Check if already have position in this symbol
        if context.current_positions.iter().any(|p| p.symbol == signal.symbol) {
            return false;
        }
        
        // Check market hours
        if !context.market_hours.is_open() {
            return false;
        }
        
        true
    }
    
    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_params: &RiskConfig,
    ) -> Result<Quantity, ExecutionError> {
        // Fixed percentage of portfolio
        let target_value = account.portfolio_value * self.config.position_size_percent;
        
        // Apply risk-based adjustment
        let adjusted_value = self.apply_risk_multiplier(
            target_value,
            signal.confidence,
            risk_params,
        );
        
        // Convert to shares
        let shares = (adjusted_value / signal.price_level).floor();
        
        // Ensure within risk limits
        let max_shares = risk_params.max_position_size_dollars / signal.price_level;
        let final_shares = shares.min(max_shares);
        
        Ok(Quantity::new(final_shares.to_u32().unwrap_or(0)))
    }
    
    fn determine_order_type(
        &self,
        signal: &TradingSignal,
        market_context: &MarketContext,
    ) -> OrderType {
        // Use market orders for simplicity and guaranteed fills
        OrderType::Market
    }
}
```

#### Aggressive Strategy

```rust
/// Aggressive strategy: Limit orders with confidence-based sizing
pub struct AggressiveStrategy {
    config: AggressiveConfig,
}

#[derive(Debug, Clone)]
pub struct AggressiveConfig {
    pub min_confidence: Decimal,  // e.g., 0.70
    pub base_position_percent: Decimal,  // e.g., 0.03 (3%)
    pub confidence_multiplier: Decimal,  // e.g., 2.0
    pub max_position_percent: Decimal,  // e.g., 0.10 (10%)
    pub use_limit_orders: bool,
    pub limit_offset_percent: Decimal,  // e.g., 0.002 (0.2%)
}

impl ExecutionStrategy for AggressiveStrategy {
    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_params: &RiskConfig,
    ) -> Result<Quantity, ExecutionError> {
        // Confidence-based sizing: higher confidence = larger position
        let confidence_factor = (signal.confidence - self.config.min_confidence)
            / (Decimal::ONE - self.config.min_confidence);
        
        let size_percent = self.config.base_position_percent
            + (confidence_factor * self.config.confidence_multiplier * self.config.base_position_percent);
        
        let size_percent = size_percent.min(self.config.max_position_percent);
        
        let target_value = account.portfolio_value * size_percent;
        let shares = (target_value / signal.price_level).floor();
        
        Ok(Quantity::new(shares.to_u32().unwrap_or(0)))
    }
    
    fn determine_order_type(
        &self,
        signal: &TradingSignal,
        market_context: &MarketContext,
    ) -> OrderType {
        if !self.config.use_limit_orders {
            return OrderType::Market;
        }
        
        // Calculate limit price with offset
        let offset = signal.price_level * self.config.limit_offset_percent;
        let limit_price = match signal.signal_type {
            SignalType::Buy => signal.price_level - offset,  // Buy slightly below
            SignalType::Sell => signal.price_level + offset,  // Sell slightly above
        };
        
        OrderType::Limit
    }
}
```

**Deliverables:**
- ✅ Multiple execution strategy implementations
- ✅ Confidence-based position sizing
- ✅ Risk-adjusted order sizing
- ✅ Strategy selection framework

**Tests:**
- Position size calculations
- Strategy decision logic
- Risk limit compliance
- Edge case handling

---

## Sub-Phase 4C-2: Automated Execution Engine (Week 19, Days 3-5)

### 10. Execution Engine Core

**File:** `src/execution/engine.rs` (~700 lines)

**Implementation:**

```rust
pub struct ExecutionEngine {
    // Dependencies
    order_engine: Arc<OrderEngine>,
    position_tracker: Arc<PositionTracker>,
    risk_engine: Arc<RiskEngine>,
    signal_engine: Arc<RwLock<SignalEngine>>,
    event_bus: EventBus,
    db_pool: Arc<SqlitePool>,
    
    // Execution strategy
    strategy: Arc<dyn ExecutionStrategy>,
    
    // Automation control
    automation_enabled: Arc<AtomicBool>,
    pattern_automation: Arc<DashMap<String, bool>>,
    
    // Configuration
    config: ExecutionConfig,
    
    // Metrics
    metrics: Arc<ExecutionMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Global automation toggle
    pub auto_execution_enabled: bool,
    
    /// Minimum signal confidence for auto-execution
    pub min_auto_confidence: Decimal,
    
    /// Require manual confirmation for low confidence
    pub manual_confirm_below: Decimal,
    
    /// Pattern-specific automation flags
    pub pattern_auto_enabled: HashMap<String, bool>,
    
    /// Execution strategy name
    pub strategy: String,
    
    /// Maximum orders per day
    pub max_orders_per_day: u32,
    
    /// Maximum auto-execution value per day
    pub max_auto_value_per_day: Decimal,
}

impl ExecutionEngine {
    pub async fn new(
        order_engine: Arc<OrderEngine>,
        position_tracker: Arc<PositionTracker>,
        risk_engine: Arc<RiskEngine>,
        signal_engine: Arc<RwLock<SignalEngine>>,
        event_bus: EventBus,
        db_pool: Arc<SqlitePool>,
        strategy: Arc<dyn ExecutionStrategy>,
        config: ExecutionConfig,
    ) -> Result<Self, ExecutionError> {
        let automation_enabled = Arc::new(AtomicBool::new(config.auto_execution_enabled));
        let pattern_automation = Arc::new(DashMap::new());
        
        // Initialize pattern automation flags
        for (pattern, enabled) in &config.pattern_auto_enabled {
            pattern_automation.insert(pattern.clone(), *enabled);
        }
        
        Ok(Self {
            order_engine,
            position_tracker,
            risk_engine,
            signal_engine,
            event_bus,
            db_pool,
            strategy,
            automation_enabled,
            pattern_automation,
            config,
            metrics: Arc::new(ExecutionMetrics::new()),
        })
    }
    
    /// Start listening for signals and executing orders
    pub async fn start(&self) -> Result<(), ExecutionError> {
        let mut signal_rx = self.event_bus.subscribe::<MarketEvent>();
        
        tracing::info!("Execution engine started, automation: {}", self.is_automation_enabled());
        
        tokio::spawn({
            let engine = self.clone();
            async move {
                while let Ok(event) = signal_rx.recv().await {
                    if let MarketEvent::SignalGenerated { signal } = event {
                        if let Err(e) = engine.handle_signal((*signal).clone()).await {
                            tracing::error!("Failed to handle signal: {}", e);
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop automated execution
    pub async fn stop(&self) {
        self.automation_enabled.store(false, Ordering::SeqCst);
        tracing::info!("Execution engine stopped");
    }
    
    /// Process signal and potentially execute order
    async fn handle_signal(
        &self,
        signal: TradingSignal,
    ) -> Result<Option<OrderId>, ExecutionError> {
        // 1. Check if automation is enabled
        if !self.is_automation_enabled() {
            self.publish_manual_review_event(&signal).await;
            return Ok(None);
        }
        
        // 2. Check pattern-specific automation
        if let Some(pattern) = &signal.pattern_name {
            if !self.is_pattern_auto_enabled(pattern) {
                self.publish_manual_review_event(&signal).await;
                return Ok(None);
            }
        }
        
        // 3. Get execution context
        let context = self.build_execution_context().await?;
        
        // 4. Check if strategy allows execution
        if !self.strategy.should_execute(&signal, &context) {
            tracing::info!("Strategy rejected signal: {:?}", signal);
            return Ok(None);
        }
        
        // 5. Calculate position size
        let account = self.order_engine.broker.get_account().await?;
        let quantity = self.strategy.calculate_position_size(
            &signal,
            &account,
            &self.risk_engine.config,
        )?;
        
        if quantity.value() == 0 {
            tracing::warn!("Position size calculated as 0 for signal: {:?}", signal);
            return Ok(None);
        }
        
        // 6. Determine order type
        let order_type = self.strategy.determine_order_type(&signal, &signal.market_context);
        
        // 7. Create order request
        let order_request = OrderRequest {
            symbol: signal.symbol.clone(),
            side: match signal.signal_type {
                SignalType::Buy => OrderSide::Buy,
                SignalType::Sell => OrderSide::Sell,
            },
            order_type,
            quantity,
            limit_price: None,  // Set by strategy
            stop_price: None,
            trailing_percent: None,
            time_in_force: TimeInForce::Day,
            pattern_name: signal.pattern_name.clone(),
            signal_id: Some(signal.id.clone()),
            signal_confidence: Some(signal.confidence),
        };
        
        // 8. Place order
        let order_id = self.order_engine.place_order(order_request).await?;
        
        // 9. Publish automation event
        self.publish_auto_execution_event(&signal, &order_id).await;
        
        // 10. Update metrics
        self.metrics.increment_auto_orders();
        
        Ok(Some(order_id))
    }
    
    /// Build execution context for strategy evaluation
    async fn build_execution_context(&self) -> Result<ExecutionContext, ExecutionError> {
        let account = self.order_engine.broker.get_account().await?;
        let current_positions = self.position_tracker.get_all_positions().await?;
        let daily_metrics = self.risk_engine.get_daily_metrics().await?;
        
        // Get pattern statistics
        let mut pattern_stats = HashMap::new();
        for position in &current_positions {
            if let Some(pattern_name) = &position.pattern_name {
                if !pattern_stats.contains_key(pattern_name) {
                    let stats = self.db_pool.get_pattern_stats(pattern_name).await?;
                    pattern_stats.insert(pattern_name.clone(), stats);
                }
            }
        }
        
        Ok(ExecutionContext {
            account,
            current_positions,
            daily_metrics,
            pattern_stats,
            market_hours: MarketHours::default(),
        })
    }
    
    /// Enable automation for specific pattern
    pub fn enable_pattern_automation(&self, pattern_name: &str) {
        self.pattern_automation.insert(pattern_name.to_string(), true);
        tracing::info!("Enabled automation for pattern: {}", pattern_name);
    }
    
    /// Disable automation for specific pattern
    pub fn disable_pattern_automation(&self, pattern_name: &str) {
        self.pattern_automation.insert(pattern_name.to_string(), false);
        tracing::info!("Disabled automation for pattern: {}", pattern_name);
    }
    
    /// Get automation status
    pub fn is_automation_enabled(&self) -> bool {
        self.automation_enabled.load(Ordering::SeqCst)
    }
    
    /// Get pattern automation status
    pub fn is_pattern_auto_enabled(&self, pattern_name: &str) -> bool {
        self.pattern_automation
            .get(pattern_name)
            .map(|v| *v)
            .unwrap_or(false)
    }
    
    async fn publish_manual_review_event(&self, signal: &TradingSignal) {
        let event = ExecutionEvent::SignalRequiresManualReview {
            signal_id: signal.id.clone(),
            reason: "Automation disabled or pattern not enabled".to_string(),
        };
        
        if let Err(e) = self.event_bus.publish(MarketEvent::ExecutionEvent(event)).await {
            tracing::error!("Failed to publish manual review event: {}", e);
        }
    }
    
    async fn publish_auto_execution_event(&self, signal: &TradingSignal, order_id: &OrderId) {
        let event = ExecutionEvent::SignalAutoExecuted {
            signal_id: signal.id.clone(),
            order_id: order_id.clone(),
            pattern_name: signal.pattern_name.clone(),
            confidence: signal.confidence,
        };
        
        if let Err(e) = self.event_bus.publish(MarketEvent::ExecutionEvent(event)).await {
            tracing::error!("Failed to publish auto execution event: {}", e);
        }
    }
}
```

**Event Integration:**

```rust
pub enum ExecutionEvent {
    AutoExecutionEnabled,
    AutoExecutionDisabled,
    PatternAutoEnabled { pattern_name: String },
    PatternAutoDisabled { pattern_name: String },
    SignalAutoExecuted {
        signal_id: String,
        order_id: OrderId,
        pattern_name: Option<String>,
        confidence: Decimal,
    },
    SignalRequiresManualReview {
        signal_id: String,
        reason: String,
    },
    ExecutionRejected {
        signal_id: String,
        reason: String,
    },
}
```

**Deliverables:**
- ✅ Complete automated execution engine
- ✅ Signal-to-order pipeline
- ✅ Pattern-specific automation controls
- ✅ Manual review workflow support
- ✅ Comprehensive event publishing

**Tests:**
- Signal processing pipeline
- Automation toggle behavior
- Pattern-specific controls
- Strategy integration
- Event publishing verification

---

## Sub-Phase 4C-3: Pattern Performance Tracking (Week 20, Days 1-3)

### 11. Performance Tracker Implementation

**File:** `src/execution/performance.rs` (~450 lines)

**Implementation:**

```rust
pub struct PerformanceTracker {
    db_pool: Arc<SqlitePool>,
    event_bus: EventBus,
    pattern_stats: Arc<DashMap<String, PatternStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternPerformance {
    pub pattern_name: String,
    pub total_signals: u32,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: Decimal,
    pub avg_profit_per_trade: Decimal,
    pub total_profit: Decimal,
    pub consecutive_wins: u32,
    pub consecutive_losses: u32,
    pub best_trade: Decimal,
    pub worst_trade: Decimal,
    pub avg_holding_period: Duration,
    pub is_enabled: bool,
    pub last_updated: DateTime<Utc>,
}

impl PerformanceTracker {
    /// Update pattern statistics when trade closes
    pub async fn record_trade_outcome(
        &self,
        pattern_name: &str,
        realized_pl: Decimal,
        holding_period: Duration,
    ) -> Result<(), PerformanceError> {
        // Get current stats
        let mut stats = self.get_pattern_performance(pattern_name).await?;
        
        // Update trade count
        stats.total_trades += 1;
        
        // Update win/loss tracking
        if realized_pl > Decimal::ZERO {
            stats.winning_trades += 1;
            stats.consecutive_wins += 1;
            stats.consecutive_losses = 0;
            stats.best_trade = stats.best_trade.max(realized_pl);
        } else {
            stats.losing_trades += 1;
            stats.consecutive_losses += 1;
            stats.consecutive_wins = 0;
            stats.worst_trade = stats.worst_trade.min(realized_pl);
        }
        
        // Update P&L
        stats.total_profit += realized_pl;
        stats.win_rate = Decimal::from(stats.winning_trades) / Decimal::from(stats.total_trades);
        stats.avg_profit_per_trade = stats.total_profit / Decimal::from(stats.total_trades);
        
        // Update holding period
        let total_duration_secs = stats.avg_holding_period.as_secs() * (stats.total_trades - 1) as u64
            + holding_period.as_secs();
        stats.avg_holding_period = Duration::from_secs(total_duration_secs / stats.total_trades as u64);
        
        stats.last_updated = Utc::now();
        
        // Persist to database
        self.save_pattern_stats(&stats).await?;
        
        // Update cache
        self.pattern_stats.insert(pattern_name.to_string(), stats.clone());
        
        // Publish performance update event
        self.publish_performance_update(&stats).await;
        
        Ok(())
    }
    
    /// Get pattern performance statistics
    pub async fn get_pattern_performance(
        &self,
        pattern_name: &str,
    ) -> Result<PatternPerformance, PerformanceError> {
        // Check cache first
        if let Some(stats) = self.pattern_stats.get(pattern_name) {
            return Ok(stats.clone());
        }
        
        // Query database
        let stats = self.db_pool.get_pattern_stats(pattern_name).await?;
        
        // Update cache
        self.pattern_stats.insert(pattern_name.to_string(), stats.clone());
        
        Ok(stats)
    }
    
    /// Get all pattern performances
    pub async fn get_all_performances(
        &self,
    ) -> Result<Vec<PatternPerformance>, PerformanceError> {
        self.db_pool.get_all_pattern_stats().await
    }
    
    /// Auto-enable/disable pattern based on performance
    pub async fn evaluate_pattern_automation(
        &self,
        pattern_name: &str,
        thresholds: &PerformanceThresholds,
    ) -> Result<AutomationDecision, PerformanceError> {
        let perf = self.get_pattern_performance(pattern_name).await?;
        
        // Not enough data yet
        if perf.total_trades < thresholds.min_trades_for_automation {
            return Ok(AutomationDecision::Maintain {
                reason: format!(
                    "Insufficient trades: {}/{}",
                    perf.total_trades,
                    thresholds.min_trades_for_automation
                ),
            });
        }
        
        // Check consecutive losses
        if perf.consecutive_losses >= thresholds.max_consecutive_losses {
            return Ok(AutomationDecision::Disable {
                reason: format!(
                    "Too many consecutive losses: {}",
                    perf.consecutive_losses
                ),
            });
        }
        
        // Check win rate
        if perf.win_rate < thresholds.min_win_rate {
            return Ok(AutomationDecision::Disable {
                reason: format!(
                    "Win rate too low: {:.1}% (min: {:.1}%)",
                    perf.win_rate * Decimal::from(100),
                    thresholds.min_win_rate * Decimal::from(100)
                ),
            });
        }
        
        // Check average profit
        if perf.avg_profit_per_trade < thresholds.min_avg_profit {
            return Ok(AutomationDecision::Disable {
                reason: format!(
                    "Average profit too low: ${:.2} (min: ${:.2})",
                    perf.avg_profit_per_trade,
                    thresholds.min_avg_profit
                ),
            });
        }
        
        // All checks passed - enable automation
        Ok(AutomationDecision::Enable {
            reason: format!(
                "Pattern performing well: {:.1}% win rate, ${:.2} avg profit",
                perf.win_rate * Decimal::from(100),
                perf.avg_profit_per_trade
            ),
        })
    }
    
    /// Get best performing patterns
    pub async fn get_top_patterns(
        &self,
        limit: usize,
    ) -> Result<Vec<PatternPerformance>, PerformanceError> {
        let mut performances = self.get_all_performances().await?;
        
        // Sort by total profit descending
        performances.sort_by(|a, b| b.total_profit.cmp(&a.total_profit));
        
        // Take top N
        performances.truncate(limit);
        
        Ok(performances)
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub min_trades_for_automation: u32,  // e.g., 20
    pub min_win_rate: Decimal,  // e.g., 0.60 (60%)
    pub max_consecutive_losses: u32,  // e.g., 3
    pub min_avg_profit: Decimal,  // e.g., 10.00
}

#[derive(Debug, Clone)]
pub enum AutomationDecision {
    Enable { reason: String },
    Disable { reason: String },
    Maintain { reason: String },
}
```

**Database Integration:**

```rust
impl Repository {
    /// Update pattern statistics
    pub async fn update_pattern_stats(
        &self,
        pattern_name: &str,
        realized_pl: Decimal,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            UPDATE pattern_stats
            SET total_trades = total_trades + 1,
                winning_trades = winning_trades + CASE WHEN ? > 0 THEN 1 ELSE 0 END,
                losing_trades = losing_trades + CASE WHEN ? <= 0 THEN 1 ELSE 0 END,
                total_profit = total_profit + ?,
                last_updated = CURRENT_TIMESTAMP
            WHERE pattern_name = ?
            "#,
            realized_pl,
            realized_pl,
            realized_pl,
            pattern_name,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get pattern statistics
    pub async fn get_pattern_stats(
        &self,
        pattern_name: &str,
    ) -> Result<PatternStats, DatabaseError> {
        let record = sqlx::query!(
            r#"
            SELECT * FROM pattern_stats
            WHERE pattern_name = ?
            "#,
            pattern_name,
        )
        .fetch_one(&self.pool)
        .await?;
        
        // Convert to PatternStats
        Ok(/* conversion logic */)
    }
    
    /// Enable/disable pattern automation
    pub async fn set_pattern_enabled(
        &self,
        pattern_name: &str,
        enabled: bool,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            UPDATE pattern_stats
            SET is_enabled = ?
            WHERE pattern_name = ?
            "#,
            enabled,
            pattern_name,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

**Deliverables:**
- ✅ Comprehensive performance tracking
- ✅ Automated enable/disable logic
- ✅ Performance-based decision making
- ✅ Statistical analysis tools

**Tests:**
- Performance calculation accuracy
- Automation decision logic
- Database persistence
- Edge cases (zero trades, all wins/losses)

---

## Sub-Phase 4C-4: Integration Testing (Week 20, Days 4-5)

### 12. End-to-End Testing

**File:** `tests/execution_integration.rs` (~500 lines)

**Test Scenarios:**

```rust
#[tokio::test]
async fn test_signal_to_order_pipeline() {
    // Setup
    let (execution_engine, signal_engine, order_engine) = setup_test_environment().await;
    
    // Generate high-confidence signal
    let signal = create_bullish_signal("AAPL", 0.85);
    
    // Publish signal
    signal_engine.publish_signal(signal).await.unwrap();
    
    // Wait for order execution
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify order was placed
    let orders = order_engine.get_open_orders().await.unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].symbol, Symbol::new("AAPL"));
    assert_eq!(orders[0].side, OrderSide::Buy);
}

#[tokio::test]
async fn test_pattern_automation_toggle() {
    let execution_engine = setup_execution_engine().await;
    
    // Initially enabled
    assert!(execution_engine.is_pattern_auto_enabled("bullish_engulfing"));
    
    // Disable pattern
    execution_engine.disable_pattern_automation("bullish_engulfing");
    assert!(!execution_engine.is_pattern_auto_enabled("bullish_engulfing"));
    
    // Signal should not execute
    let signal = create_pattern_signal("bullish_engulfing", 0.80);
    let result = execution_engine.handle_signal(signal).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_performance_based_automation() {
    let (perf_tracker, execution_engine) = setup_test_environment().await;
    
    // Record consecutive losses
    for _ in 0..3 {
        perf_tracker.record_trade_outcome(
            "morning_star",
            Decimal::new(-5000, 2),  // -$50
            Duration::hours(2),
        ).await.unwrap();
    }
    
    // Evaluate automation
    let decision = perf_tracker.evaluate_pattern_automation(
        "morning_star",
        &default_thresholds(),
    ).await.unwrap();
    
    // Should disable due to consecutive losses
    assert!(matches!(decision, AutomationDecision::Disable { .. }));
}

#[tokio::test]
async fn test_position_tracking_from_fills() {
    let (position_tracker, order_engine) = setup_test_environment().await;
    
    // Place buy order
    let order_request = create_buy_order_request("TSLA", 10);
    let order_id = order_engine.place_order(order_request).await.unwrap();
    
    // Simulate fill
    let fill_info = FillInfo {
        filled_quantity: Quantity::new(10),
        filled_price: Price::new(Decimal::new(25000, 2)),  // $250
        commission: Decimal::new(0, 0),
        timestamp: Utc::now(),
    };
    
    position_tracker.update_from_fill(&order, &fill_info).await.unwrap();
    
    // Verify position created
    let position = position_tracker.get_position(&Symbol::new("TSLA")).await.unwrap();
    assert!(position.is_some());
    let pos = position.unwrap();
    assert_eq!(pos.quantity.value(), 10);
    assert_eq!(pos.avg_entry_price.value(), Decimal::new(25000, 2));
}

#[tokio::test]
async fn test_risk_validation_at_execution() {
    let execution_engine = setup_execution_engine().await;
    
    // Create signal that would violate daily loss limit
    // (Assumes daily loss limit already reached in test setup)
    let signal = create_bullish_signal("SPY", 0.90);
    
    // Should reject due to risk violation
    let result = execution_engine.handle_signal(signal).await;
    assert!(matches!(result, Err(ExecutionError::RiskViolation(_))));
}
```

**Deliverables:**
- ✅ End-to-end integration tests
- ✅ Automation workflow validation
- ✅ Performance tracking verification
- ✅ Risk integration testing
- ✅ Event flow validation

**Test Coverage Goals:**
- > 85% code coverage for execution module
- All critical paths tested
- Edge cases and error scenarios covered
- Concurrent execution scenarios validated

---

## Performance Targets

| Operation | Target | Stretch Goal | Measurement |
|-----------|--------|--------------|-------------|
| Signal → Order (Auto) | <200ms | <100ms | End-to-end from signal event to order placement |
| Position Size Calculation | <10ms | <5ms | Strategy calculation time |
| Strategy Evaluation | <50ms | <20ms | should_execute() call time |
| Performance Stats Update | <100ms | <50ms | Database update after trade |
| Automation Check | <5ms | <2ms | Pattern automation flag lookup |

---

## Integration Points

### Phase 1 Integration
- ✅ Use event bus for signal subscriptions
- ✅ Leverage broker abstraction for account queries

### Phase 2 Integration
- ✅ Call risk engine for pre-trade validation
- ✅ Respect pattern-specific limits
- ✅ Integrate with daily loss tracking

### Phase 3 Integration
- ✅ Subscribe to pattern signal events
- ✅ Process signal confidence scores
- ✅ Track pattern performance for feedback

### Phase 4A Integration
- ✅ Use order engine for order placement
- ✅ Subscribe to order fill events
- ✅ Link orders to signals

### Phase 4B Integration
- ✅ Query current positions for strategy decisions
- ✅ Update performance tracker on position closure
- ✅ Calculate portfolio-level metrics

---

## Success Metrics

### Functional Requirements
- ✅ Multiple execution strategies implemented
- ✅ Signal-to-order pipeline operational
- ✅ Pattern automation controls functional
- ✅ Performance tracking working
- ✅ Manual review workflow supported

### Quality Requirements
- ✅ Zero critical bugs
- ✅ >85% test coverage
- ✅ All performance targets met
- ✅ Zero compiler warnings
- ✅ Comprehensive error handling

### Integration Requirements
- ✅ Seamless signal subscription
- ✅ Full risk engine integration
- ✅ Order engine integration
- ✅ Database persistence
- ✅ Event bus integration

---

## Risk Assessment & Mitigation

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Signal processing latency | Medium | High | Async processing, performance monitoring |
| Strategy decision errors | Low | High | Extensive testing, validation logic |
| Performance tracking accuracy | Low | Medium | Database validation, reconciliation |
| Concurrent execution conflicts | Medium | Medium | Thread-safe collections, atomic operations |

### Business Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Pattern performance degrades | Medium | Medium | Continuous monitoring, auto-disable logic |
| Automation violates risk rules | Low | Critical | Pre-trade validation, multiple safety layers |
| False signal execution | Medium | Medium | Confidence thresholds, manual review option |

### Mitigated Risks
- **Race Conditions**: Atomic operations, thread-safe collections
- **Data Consistency**: Event-driven updates, transaction safety
- **Performance Degradation**: Benchmarking, regression tests

---

## Next Phase Handoff

### To Phase 4D (Testing & Documentation)

**Deliverables:**
- Complete automated execution system
- Performance tracking framework
- Pattern automation controls
- Integration test suite

**APIs for Testing:**
```rust
// Phase 4D will validate these APIs
execution_engine.start().await?;
execution_engine.enable_pattern_automation("hammer");
execution_engine.disable_pattern_automation("shooting_star");
perf_tracker.get_pattern_performance("bullish_engulfing").await?;
perf_tracker.evaluate_pattern_automation("morning_star", &thresholds).await?;
```

---

## Documentation Requirements

### API Documentation
- All public types documented
- Code examples for common use cases
- Strategy implementation guide
- Error handling patterns

### User Guides
1. **Automated Execution Guide**
   - Signal-to-order pipeline
   - Strategy selection
   - Automation controls
   - Performance monitoring

2. **Pattern Management Guide**
   - Enable/disable patterns
   - Performance thresholds
   - Manual review workflows
   - Safety features

---

**Document Version:** 1.0  
**Last Updated:** January 25, 2026  
**Part of:** [Phase 4: Order Execution](./phase-4-order-execution-plan.md)  
**Status:** Ready for Implementation

**Note:** This document is ~700 lines, exceeding the typical 600-line target. The execution strategies, engine core, performance tracking, and integration testing form a tightly coupled system that benefits from unified documentation per project rules allowing exceptions for tightly coupled concepts.
