//! Automated execution engine
//!
//! This module provides the core execution engine that connects pattern signals
//! to order placement, implementing the automated trading pipeline.

use crate::brokers::traits::{OrderSide, OrderType};
use crate::brokers::types::{OrderId, PatternName, Price};
use crate::events::{EventBus, MarketEvent};
use crate::execution::strategy::{ExecutionContext, ExecutionStrategy};
use crate::execution::{
    AutomationDecision, ExecutionError, ExecutionEvent, PatternPerformance, PerformanceThresholds,
};
use crate::orders::engine::OrderEngine;
use crate::orders::types::OrderRequest;
use crate::patterns::signals::TradingSignal;
use crate::positions::tracker::PositionTracker;
use crate::risk::engine::RiskEngine;
use chrono::Utc;
use dashmap::DashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ============================================================================
// Execution Engine
// ============================================================================

/// Core automated execution engine
///
/// Coordinates signal-to-order conversion with pattern automation controls
/// and performance tracking.
///
/// # Examples
///
/// ```no_run
/// use daytrader::execution::{ExecutionEngine, ExecutionConfig};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let order_engine = todo!();
/// # let position_tracker = todo!();
/// # let risk_engine = todo!();
/// # let event_bus = todo!();
/// # let db_pool = todo!();
/// # let strategy = todo!();
/// let config = ExecutionConfig::default();
///
/// let engine = ExecutionEngine::new(
///     order_engine,
///     position_tracker,
///     risk_engine,
///     event_bus,
///     db_pool,
///     strategy,
///     config,
/// ).await?;
///
/// // Start automated execution
/// engine.start().await?;
/// # Ok(())
/// # }
/// ```
pub struct ExecutionEngine {
    /// Order engine for placing orders
    order_engine: Arc<OrderEngine>,

    /// Position tracker for monitoring positions
    position_tracker: Arc<PositionTracker>,

    /// Risk engine for validation
    risk_engine: Arc<RiskEngine>,

    /// Event bus for publishing/subscribing
    event_bus: Arc<EventBus>,

    /// Database pool for persistence
    db_pool: Arc<SqlitePool>,

    /// Execution strategy
    strategy: Arc<dyn ExecutionStrategy>,

    /// Configuration
    config: ExecutionConfig,

    /// Global automation toggle
    automation_enabled: Arc<AtomicBool>,

    /// Pattern-specific automation flags
    pattern_automation: Arc<DashMap<PatternName, bool>>,

    /// Pattern performance tracking
    pattern_performance: Arc<DashMap<PatternName, PatternPerformance>>,

    /// Execution metrics
    metrics: Arc<ExecutionMetrics>,

    /// Time source for execution context (injectable for backtesting)
    clock: Arc<dyn crate::utils::clock::Clock>,
}

/// Configuration for the execution engine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Global automation enabled
    pub auto_execution_enabled: bool,

    /// Minimum confidence for auto-execution
    pub min_auto_confidence: Decimal,

    /// Require manual confirmation below this threshold
    pub manual_confirm_below: Decimal,

    /// Pattern-specific automation enabled flags
    pub pattern_auto_enabled: HashMap<String, bool>,

    /// Maximum orders per day
    pub max_orders_per_day: u32,

    /// Maximum auto-execution value per day
    pub max_auto_value_per_day: Decimal,

    /// Performance thresholds for automation decisions
    pub performance_thresholds: PerformanceThresholds,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            auto_execution_enabled: false,             // Start disabled for safety
            min_auto_confidence: Decimal::new(75, 2),  // 0.75
            manual_confirm_below: Decimal::new(65, 2), // 0.65
            pattern_auto_enabled: HashMap::new(),
            max_orders_per_day: 20,
            max_auto_value_per_day: Decimal::new(5000000, 2), // $50,000
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

/// Execution metrics tracking
#[derive(Debug, Default)]
pub struct ExecutionMetrics {
    /// Total signals processed
    pub total_signals: AtomicU64,

    /// Signals auto-executed
    pub auto_executed: AtomicU64,

    /// Signals requiring manual review
    pub manual_review: AtomicU64,

    /// Signals rejected
    pub rejected: AtomicU64,

    /// Total execution time (milliseconds)
    pub total_exec_time_ms: AtomicU64,
}

impl ExecutionMetrics {
    /// Creates new execution metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Increments auto-executed counter
    pub fn increment_auto_orders(&self) {
        self.auto_executed.fetch_add(1, Ordering::SeqCst);
    }

    /// Increments manual review counter
    pub fn increment_manual_review(&self) {
        self.manual_review.fetch_add(1, Ordering::SeqCst);
    }

    /// Increments rejected counter
    pub fn increment_rejected(&self) {
        self.rejected.fetch_add(1, Ordering::SeqCst);
    }

    /// Records execution time
    pub fn record_exec_time(&self, duration_ms: u64) {
        self.total_exec_time_ms
            .fetch_add(duration_ms, Ordering::SeqCst);
        self.total_signals.fetch_add(1, Ordering::SeqCst);
    }

    /// Gets average execution time
    pub fn avg_exec_time_ms(&self) -> u64 {
        let total = self.total_signals.load(Ordering::SeqCst);
        if total == 0 {
            0
        } else {
            self.total_exec_time_ms.load(Ordering::SeqCst) / total
        }
    }

    /// Gets stats snapshot
    pub fn snapshot(&self) -> ExecutionMetricsSnapshot {
        ExecutionMetricsSnapshot {
            total_signals: self.total_signals.load(Ordering::SeqCst),
            auto_executed: self.auto_executed.load(Ordering::SeqCst),
            manual_review: self.manual_review.load(Ordering::SeqCst),
            rejected: self.rejected.load(Ordering::SeqCst),
            avg_exec_time_ms: self.avg_exec_time_ms(),
        }
    }
}

/// Snapshot of execution metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionMetricsSnapshot {
    pub total_signals: u64,
    pub auto_executed: u64,
    pub manual_review: u64,
    pub rejected: u64,
    pub avg_exec_time_ms: u64,
}

impl ExecutionEngine {
    /// Creates a new execution engine
    pub async fn new(
        order_engine: Arc<OrderEngine>,
        position_tracker: Arc<PositionTracker>,
        risk_engine: Arc<RiskEngine>,
        event_bus: Arc<EventBus>,
        db_pool: Arc<SqlitePool>,
        strategy: Arc<dyn ExecutionStrategy>,
        config: ExecutionConfig,
    ) -> Result<Self, ExecutionError> {
        let automation_enabled = Arc::new(AtomicBool::new(config.auto_execution_enabled));
        let pattern_automation = Arc::new(DashMap::new());
        let pattern_performance = Arc::new(DashMap::new());

        // Initialize pattern automation flags
        for (pattern, enabled) in &config.pattern_auto_enabled {
            let pattern_name = PatternName::new(pattern);
            pattern_automation.insert(pattern_name.clone(), *enabled);

            // Initialize performance tracking
            pattern_performance.insert(pattern_name.clone(), PatternPerformance::new(pattern_name));
        }

        Ok(Self {
            order_engine,
            position_tracker,
            risk_engine,
            event_bus,
            db_pool,
            strategy,
            config,
            automation_enabled,
            pattern_automation,
            pattern_performance,
            metrics: Arc::new(ExecutionMetrics::new()),
            clock: Arc::new(crate::utils::clock::SystemClock),
        })
    }

    /// Replaces the engine's time source
    ///
    /// Production uses the default [`crate::utils::clock::SystemClock`]; the
    /// backtester injects a [`crate::utils::clock::SteppedClock`] so the
    /// execution context (market-hours rules, timestamps) follows replay
    /// time instead of wall-clock time.
    pub fn with_clock(mut self, clock: Arc<dyn crate::utils::clock::Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Starts the execution engine
    ///
    /// Subscribes to signal events and begins processing them.
    pub async fn start(&self) -> Result<(), ExecutionError> {
        let mut receiver = self.event_bus.subscribe();

        tracing::info!(
            "Execution engine started (automation: {})",
            self.is_automation_enabled()
        );

        // Clone Arc references for the async task
        let engine = Arc::new(self.clone_refs());

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                if let MarketEvent::SignalGenerated { signal } = event {
                    let start = std::time::Instant::now();

                    if let Err(e) = engine.handle_signal(signal).await {
                        tracing::error!("Failed to handle signal: {}", e);
                    }

                    let elapsed = start.elapsed().as_millis() as u64;
                    engine.metrics.record_exec_time(elapsed);
                }
            }
        });

        Ok(())
    }

    /// Stops automated execution
    pub async fn stop(&self) {
        self.automation_enabled.store(false, Ordering::SeqCst);
        tracing::info!("Execution engine stopped");
    }

    /// Processes a single trading signal synchronously
    ///
    /// Same pipeline as the event-bus path used by [`Self::start`], but
    /// callable directly so the backtester can replay history
    /// deterministically (no event-delivery races between steps).
    ///
    /// # Errors
    ///
    /// Returns an error if context building, sizing, or order placement
    /// fails. `Ok(None)` means the signal was consumed without an order
    /// (automation off, below confidence threshold, strategy rejection).
    pub async fn process_signal(
        &self,
        signal: TradingSignal,
    ) -> Result<Option<OrderId>, ExecutionError> {
        self.handle_signal(signal).await
    }

    /// Handles an incoming trading signal
    async fn handle_signal(
        &self,
        signal: TradingSignal,
    ) -> Result<Option<OrderId>, ExecutionError> {
        tracing::debug!(
            "Processing signal: {} {} (confidence: {:.2})",
            signal.symbol.as_str(),
            signal.signal_type.as_str(),
            signal.confidence.value()
        );

        // 1. Check if automation is globally enabled
        if !self.is_automation_enabled() {
            self.publish_manual_review_event(&signal, "Global automation disabled")
                .await;
            self.metrics.increment_manual_review();
            return Ok(None);
        }

        // 2. Check signal confidence threshold
        if Decimal::from_f64_retain(signal.confidence.value()).unwrap_or(Decimal::ZERO)
            < self.config.min_auto_confidence
        {
            self.publish_manual_review_event(&signal, "Confidence below auto threshold")
                .await;
            self.metrics.increment_manual_review();
            return Ok(None);
        }

        // 3. Check pattern-specific automation
        if let Some(pattern_name) = self.extract_pattern_name(&signal) {
            if !self.is_pattern_auto_enabled(&pattern_name) {
                self.publish_manual_review_event(&signal, "Pattern automation disabled")
                    .await;
                self.metrics.increment_manual_review();
                return Ok(None);
            }

            // Record signal for performance tracking
            if let Some(mut perf) = self.pattern_performance.get_mut(&pattern_name) {
                perf.record_signal();
            }
        }

        // 4. Build execution context
        let context = self.build_execution_context().await?;

        // 5. Check if strategy allows execution
        if let Err(e) = self.strategy.should_execute(&signal, &context) {
            tracing::info!("Strategy rejected signal: {}", e);
            self.publish_rejected_event(&signal, &e.to_string()).await;
            self.metrics.increment_rejected();
            return Ok(None);
        }

        // 6. Calculate position size
        let account = context.account.clone();
        let quantity =
            self.strategy
                .calculate_position_size(&signal, &account, self.risk_engine.config())?;

        if quantity.value() == 0 {
            tracing::warn!("Position size calculated as 0, skipping execution");
            self.metrics.increment_rejected();
            return Ok(None);
        }

        // 7. Determine order type and parameters
        let order_type = self
            .strategy
            .determine_order_type(&signal, signal.price_level);
        let time_in_force = self.strategy.determine_time_in_force(&signal);
        let limit_price = if matches!(order_type, OrderType::Limit) {
            self.strategy
                .calculate_limit_price(&signal, signal.price_level)
        } else {
            None
        };

        // 8. Create order request
        let order_request = OrderRequest {
            symbol: signal.symbol.clone(),
            side: match signal.signal_type {
                crate::events::types::SignalType::Buy => OrderSide::Buy,
                crate::events::types::SignalType::Sell => OrderSide::Sell,
            },
            order_type,
            quantity,
            limit_price,
            stop_price: None,
            trailing_percent: None,
            time_in_force,
            pattern_name: self.extract_pattern_name(&signal),
            signal_id: Some(signal.id.clone()),
            signal_confidence: Decimal::from_f64_retain(signal.confidence.value()),
        };

        // 9. Place order through order engine
        let order_id = self.order_engine.place_order(order_request).await?;

        // 10. Publish execution event
        self.publish_auto_execution_event(&signal, &order_id).await;
        self.metrics.increment_auto_orders();

        tracing::info!(
            "Auto-executed signal: {} {} @ {} (order: {})",
            signal.symbol.as_str(),
            signal.signal_type.as_str(),
            signal.price_level.value(),
            order_id.as_str()
        );

        Ok(Some(order_id))
    }

    /// Builds execution context for strategy evaluation
    async fn build_execution_context(&self) -> Result<ExecutionContext, ExecutionError> {
        let account = self.order_engine.broker().get_account().await?;
        let current_positions = self.position_tracker.get_all_positions().await?;
        let daily_metrics = self.get_daily_metrics().await;

        // Build pattern stats map
        let pattern_stats: HashMap<_, _> = self
            .pattern_performance
            .iter()
            .map(|entry| {
                let pattern_name = entry.key().clone();
                let perf = entry.value();
                let stats = crate::risk::pattern_stats::PatternPerformanceStats {
                    win_rate: perf.win_rate.to_f64().unwrap_or(0.0),
                    total_trades: perf.total_trades,
                    consecutive_losses: perf.consecutive_losses,
                    total_profit: perf.total_profit,
                    avg_profit_per_trade: perf.avg_profit_per_trade,
                };
                (pattern_name, stats)
            })
            .collect();

        Ok(ExecutionContext {
            account,
            current_positions,
            daily_metrics,
            pattern_stats,
            current_time: self.clock.now(),
        })
    }

    /// Gets daily risk metrics
    async fn get_daily_metrics(&self) -> crate::risk::types::DailyRiskMetrics {
        let state = self.risk_engine.get_risk_state().await;
        crate::risk::types::DailyRiskMetrics {
            realized_pl: state.daily_pl,
            unrealized_pl: Decimal::ZERO, // Would be calculated from positions
            total_pl: state.daily_pl,
            day_trade_count: state.day_trade_count,
            trading_halted: state.is_trading_halted,
        }
    }

    /// Extracts pattern name from signal
    fn extract_pattern_name(&self, signal: &TradingSignal) -> Option<PatternName> {
        signal.components.first().and_then(|comp| {
            if matches!(
                comp.component_type,
                crate::patterns::signals::SignalComponentType::Pattern
            ) {
                Some(PatternName::new(&comp.name))
            } else {
                None
            }
        })
    }

    // ========================================================================
    // Automation Controls
    // ========================================================================

    /// Enables global automation
    pub fn enable_automation(&self) {
        self.automation_enabled.store(true, Ordering::SeqCst);
        tracing::info!("Global automation enabled");

        // Publish event
        tokio::spawn({
            let bus = Arc::clone(&self.event_bus);
            async move {
                let _ = bus.publish(MarketEvent::PatternEnabled {
                    pattern_name: PatternName::new("global"),
                    reason: "Manual enable".to_string(),
                });
            }
        });
    }

    /// Disables global automation
    pub fn disable_automation(&self) {
        self.automation_enabled.store(false, Ordering::SeqCst);
        tracing::info!("Global automation disabled");

        // Publish event
        tokio::spawn({
            let bus = Arc::clone(&self.event_bus);
            async move {
                let _ = bus.publish(MarketEvent::PatternDisabled {
                    pattern_name: PatternName::new("global"),
                    reason: "Manual disable".to_string(),
                });
            }
        });
    }

    /// Checks if automation is enabled
    pub fn is_automation_enabled(&self) -> bool {
        self.automation_enabled.load(Ordering::SeqCst)
    }

    /// Enables automation for a specific pattern
    pub fn enable_pattern_automation(&self, pattern_name: &PatternName) {
        self.pattern_automation.insert(pattern_name.clone(), true);
        tracing::info!("Enabled automation for pattern: {}", pattern_name.as_str());
    }

    /// Disables automation for a specific pattern
    pub fn disable_pattern_automation(&self, pattern_name: &PatternName) {
        self.pattern_automation.insert(pattern_name.clone(), false);
        tracing::info!("Disabled automation for pattern: {}", pattern_name.as_str());
    }

    /// Checks if pattern automation is enabled
    pub fn is_pattern_auto_enabled(&self, pattern_name: &PatternName) -> bool {
        self.pattern_automation
            .get(pattern_name)
            .map(|v| *v)
            .unwrap_or(false)
    }

    // ========================================================================
    // Performance Tracking
    // ========================================================================

    /// Records trade outcome for pattern performance
    pub async fn record_trade_outcome(
        &self,
        pattern_name: &PatternName,
        realized_pl: Decimal,
        holding_period_secs: u64,
    ) -> Result<(), ExecutionError> {
        if let Some(mut perf) = self.pattern_performance.get_mut(pattern_name) {
            perf.record_trade(realized_pl, holding_period_secs);

            // Evaluate automation based on performance
            let decision = self.evaluate_pattern_automation(&perf).await;

            match decision {
                AutomationDecision::Enable { reason } => {
                    self.enable_pattern_automation(pattern_name);
                    tracing::info!("Auto-enabled {}: {}", pattern_name.as_str(), reason);
                }
                AutomationDecision::Disable { reason } => {
                    self.disable_pattern_automation(pattern_name);
                    tracing::warn!("Auto-disabled {}: {}", pattern_name.as_str(), reason);
                }
                AutomationDecision::Maintain { .. } => {
                    // No change
                }
            }
        }

        Ok(())
    }

    /// Evaluates whether to enable/disable pattern automation based on performance
    async fn evaluate_pattern_automation(&self, perf: &PatternPerformance) -> AutomationDecision {
        let thresholds = &self.config.performance_thresholds;

        // Not enough data yet
        if perf.total_trades < thresholds.min_trades_for_automation {
            return AutomationDecision::Maintain {
                reason: format!(
                    "Insufficient trades: {}/{}",
                    perf.total_trades, thresholds.min_trades_for_automation
                ),
            };
        }

        // Check consecutive losses
        if perf.consecutive_losses >= thresholds.max_consecutive_losses {
            return AutomationDecision::Disable {
                reason: format!("Too many consecutive losses: {}", perf.consecutive_losses),
            };
        }

        // Check win rate
        if perf.win_rate < thresholds.min_win_rate {
            return AutomationDecision::Disable {
                reason: format!(
                    "Win rate too low: {:.1}% (min: {:.1}%)",
                    perf.win_rate * Decimal::from(100),
                    thresholds.min_win_rate * Decimal::from(100)
                ),
            };
        }

        // Check average profit
        if perf.avg_profit_per_trade < thresholds.min_avg_profit {
            return AutomationDecision::Disable {
                reason: format!(
                    "Average profit too low: ${:.2} (min: ${:.2})",
                    perf.avg_profit_per_trade, thresholds.min_avg_profit
                ),
            };
        }

        // All checks passed - enable automation
        AutomationDecision::Enable {
            reason: format!(
                "Pattern performing well: {:.1}% win rate, ${:.2} avg profit",
                perf.win_rate * Decimal::from(100),
                perf.avg_profit_per_trade
            ),
        }
    }

    /// Gets pattern performance
    pub fn get_pattern_performance(
        &self,
        pattern_name: &PatternName,
    ) -> Option<PatternPerformance> {
        self.pattern_performance
            .get(pattern_name)
            .map(|p| p.clone())
    }

    /// Gets all pattern performances
    pub fn get_all_performances(&self) -> Vec<PatternPerformance> {
        self.pattern_performance
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Gets execution metrics
    pub fn get_metrics(&self) -> ExecutionMetricsSnapshot {
        self.metrics.snapshot()
    }

    // ========================================================================
    // Event Publishing
    // ========================================================================

    async fn publish_manual_review_event(&self, signal: &TradingSignal, reason: &str) {
        let pattern_name = self.extract_pattern_name(signal);

        let _ = self.event_bus.publish(MarketEvent::PatternDetected {
            signal: crate::events::types::PatternSignal {
                symbol: signal.symbol.clone(),
                pattern_name: pattern_name
                    .clone()
                    .unwrap_or_else(|| PatternName::new("unknown")),
                signal_type: signal.signal_type.clone(),
                confidence: signal.confidence,
                detected_at: signal.generated_at,
                price: signal.price_level,
            },
        });

        tracing::info!(
            "Signal requires manual review: {} - {}",
            signal.symbol.as_str(),
            reason
        );
    }

    async fn publish_auto_execution_event(&self, signal: &TradingSignal, order_id: &OrderId) {
        let pattern_name = self.extract_pattern_name(signal);

        let _ = self.event_bus.publish(MarketEvent::PatternDetected {
            signal: crate::events::types::PatternSignal {
                symbol: signal.symbol.clone(),
                pattern_name: pattern_name
                    .clone()
                    .unwrap_or_else(|| PatternName::new("unknown")),
                signal_type: signal.signal_type.clone(),
                confidence: signal.confidence,
                detected_at: signal.generated_at,
                price: signal.price_level,
            },
        });
    }

    async fn publish_rejected_event(&self, signal: &TradingSignal, reason: &str) {
        tracing::debug!("Signal rejected: {} - {}", signal.symbol.as_str(), reason);
    }

    // Helper method to clone Arc references
    fn clone_refs(&self) -> Self {
        Self {
            order_engine: Arc::clone(&self.order_engine),
            position_tracker: Arc::clone(&self.position_tracker),
            risk_engine: Arc::clone(&self.risk_engine),
            event_bus: Arc::clone(&self.event_bus),
            db_pool: Arc::clone(&self.db_pool),
            strategy: Arc::clone(&self.strategy),
            config: self.config.clone(),
            automation_enabled: Arc::clone(&self.automation_enabled),
            clock: Arc::clone(&self.clock),
            pattern_automation: Arc::clone(&self.pattern_automation),
            pattern_performance: Arc::clone(&self.pattern_performance),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_metrics() {
        let metrics = ExecutionMetrics::new();

        metrics.increment_auto_orders();
        metrics.increment_manual_review();
        metrics.record_exec_time(50);

        assert_eq!(metrics.auto_executed.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.manual_review.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.avg_exec_time_ms(), 50);
    }

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();

        assert!(!config.auto_execution_enabled);
        assert_eq!(config.min_auto_confidence, Decimal::new(75, 2));
        assert_eq!(config.max_orders_per_day, 20);
    }
}
