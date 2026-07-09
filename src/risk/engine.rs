//! Risk Engine - Central risk evaluation and enforcement component
//!
//! The RiskEngine coordinates all risk management activities:
//! - Subscribes to relevant market events
//! - Evaluates risk rules against current state
//! - Publishes risk violations and warnings
//! - Maintains risk state persistence
//!
//! # Architecture
//!
//! The engine follows an event-driven, functional design:
//! - Immutable state updates
//! - Async-first operations
//! - Zero-copy event handling where possible
//! - Type-safe rule evaluation
//! - Composable risk rules with real-time monitoring

use crate::brokers::types::{Message, OrderId, Symbol};
use crate::config::RiskConfig;
use crate::db::{Database, PatternRepository, PerformanceRepository, SignalRepository};
use crate::events::bus::EventBus;
use crate::events::types::{MarketEvent, OrderSide, RiskViolation, RiskViolationType};
use crate::risk::limits::{DailyLossLimits, DailyLossTracker};
use crate::risk::pdt::{PdtConfig, PdtEnforcer};
use crate::risk::position_sizing::{PositionSizeLimits, PositionSizer};
use crate::risk::position_tracker::PositionTracker;
use crate::risk::rules::{RiskRuleEngine, TradeContext};
use crate::risk::types::*;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Central risk management engine
pub struct RiskEngine {
    config: RiskConfig,
    event_bus: Arc<EventBus>,
    state: Arc<RwLock<RiskState>>,
    // New risk rule components
    rule_engine: RiskRuleEngine,
    daily_loss_tracker: Arc<DailyLossTracker>,
    position_sizer: Arc<PositionSizer>,
    pdt_enforcer: Arc<PdtEnforcer>,
    // Position lifecycle tracking for day-trade detection (FIFO lots per symbol)
    position_tracker: Arc<PositionTracker>,
    /// Account equity at engine construction; combined with daily P&L for
    /// balance-based checks (emergency liquidation threshold)
    initial_equity: Decimal,
    // Metadata for orders awaiting fill, so `update_on_fill` can recover the
    // symbol/side needed to feed the position tracker (OrderFilled events
    // only carry the order id and fill, not the originating order).
    pending_orders: Arc<DashMap<OrderId, (Symbol, OrderSide)>>,
    // Pattern-specific risk management
    pattern_stats_tracker: Arc<RwLock<crate::risk::pattern_stats::PatternStatsTracker>>,
    pattern_risk_evaluator: Arc<RwLock<crate::risk::pattern_limits::PatternRiskEvaluator>>,
    // Database integration
    database: Option<Arc<Database>>,
    pattern_repo: Option<PatternRepository>,
    signal_repo: Option<SignalRepository>,
    performance_repo: Option<PerformanceRepository>,
}

/// Internal risk state tracking
#[derive(Debug, Clone)]
pub struct RiskState {
    pub account_id: AccountId,
    pub trading_date: chrono::NaiveDate,
    pub daily_pl: Decimal,
    pub day_trade_count: u32,
    pub is_trading_halted: bool,
    pub last_updated: DateTime<Utc>,
}

impl RiskEngine {
    /// Creates a new risk engine with the given configuration
    pub fn new(
        config: RiskConfig,
        event_bus: Arc<EventBus>,
        account_id: AccountId,
        initial_equity: Decimal,
    ) -> Self {
        Self::with_database(config, event_bus, account_id, initial_equity, None)
    }

    /// Creates a new risk engine with database integration
    pub fn with_database(
        config: RiskConfig,
        event_bus: Arc<EventBus>,
        account_id: AccountId,
        initial_equity: Decimal,
        database: Option<Arc<Database>>,
    ) -> Self {
        let state = RiskState {
            account_id,
            trading_date: Utc::now().date_naive(),
            daily_pl: Decimal::ZERO,
            day_trade_count: 0,
            is_trading_halted: false,
            last_updated: Utc::now(),
        };

        // Initialize risk rule components
        let rule_engine = RiskRuleEngine::new(config.clone());

        let daily_loss_tracker = Arc::new(DailyLossTracker::from_config(&config, initial_equity));

        let position_limits = PositionSizeLimits::from_config(&config);
        let position_sizer = Arc::new(PositionSizer::new(position_limits, initial_equity));

        let pdt_config = PdtConfig::from_config(&config);
        let pdt_enforcer = Arc::new(PdtEnforcer::new(pdt_config, initial_equity));

        // Position lifecycle tracker for real day-trade detection (FIFO lots per symbol)
        let position_tracker = Arc::new(PositionTracker::new());
        let pending_orders = Arc::new(DashMap::new());

        // Initialize pattern statistics tracker
        let pattern_stats_tracker = Arc::new(RwLock::new(
            crate::risk::pattern_stats::PatternStatsTracker::new(),
        ));

        // Initialize pattern risk evaluator with stats tracker
        let pattern_config = crate::risk::pattern_limits::PatternRiskConfig::from_config(&config);
        let mut pattern_risk_evaluator =
            crate::risk::pattern_limits::PatternRiskEvaluator::new(pattern_config);
        pattern_risk_evaluator.set_stats_tracker(Arc::clone(&pattern_stats_tracker));
        let pattern_risk_evaluator = Arc::new(RwLock::new(pattern_risk_evaluator));

        // Initialize database repositories if database is provided
        let (pattern_repo, signal_repo, performance_repo) = if let Some(ref db) = database {
            (
                Some(PatternRepository::new(db.pool.clone())),
                Some(SignalRepository::new(db.pool.clone())),
                Some(PerformanceRepository::new(db.pool.clone())),
            )
        } else {
            (None, None, None)
        };

        Self {
            config,
            event_bus,
            state: Arc::new(RwLock::new(state)),
            initial_equity,
            rule_engine,
            daily_loss_tracker,
            position_sizer,
            pdt_enforcer,
            position_tracker,
            pending_orders,
            pattern_stats_tracker,
            pattern_risk_evaluator,
            database,
            pattern_repo,
            signal_repo,
            performance_repo,
        }
    }

    /// Starts the risk engine event processing
    ///
    /// This spawns async tasks to monitor events and enforce risk rules
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Subscribe to relevant events
        let event_bus = Arc::clone(&self.event_bus);
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let daily_loss_tracker = Arc::clone(&self.daily_loss_tracker);
        let position_sizer = Arc::clone(&self.position_sizer);
        let pdt_enforcer = Arc::clone(&self.pdt_enforcer);
        let position_tracker = Arc::clone(&self.position_tracker);
        let pending_orders = Arc::clone(&self.pending_orders);
        let pattern_stats_tracker = Arc::clone(&self.pattern_stats_tracker);
        let pattern_risk_evaluator = Arc::clone(&self.pattern_risk_evaluator);

        // Create a new rule engine for the async task (can't clone the existing one)
        let rule_engine = crate::risk::rules::RiskRuleEngine::new(config.clone());

        let engine = Arc::new(RiskEngine {
            config,
            event_bus: Arc::clone(&self.event_bus),
            state,
            initial_equity: self.initial_equity,
            rule_engine,
            daily_loss_tracker,
            position_sizer,
            pdt_enforcer,
            position_tracker,
            pending_orders,
            pattern_stats_tracker,
            pattern_risk_evaluator,
            database: self.database.clone(),
            pattern_repo: self.pattern_repo.clone(),
            signal_repo: self.signal_repo.clone(),
            performance_repo: self.performance_repo.clone(),
        });

        // Subscribe BEFORE spawning: a receiver created inside the task only
        // sees events published after the task gets scheduled, so anything
        // published immediately after start() returned was silently missed.
        let mut receiver = event_bus.subscribe();

        // Spawn event processing task
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Err(e) = engine.process_event(event).await {
                            tracing::error!("Risk engine event processing error: {}", e);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                        // Dropped messages are expected under load; skip ahead
                        // instead of killing risk processing
                        tracing::warn!("Risk engine receiver lagged, skipped {missed} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    /// Processes a market event and applies risk rules
    async fn process_event(
        &self,
        event: MarketEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match event {
            MarketEvent::PatternDetected { signal } => {
                // Record pattern signal detection for statistics
                self.record_pattern_signal(&signal).await?;

                // Real-time risk evaluation on pattern detection
                self.evaluate_pattern_risk_on_detection(&signal).await?;
            }
            MarketEvent::OrderPlaced { order } => {
                // Remember the symbol/side for this order so `update_on_fill`
                // can recover them later — `OrderFilled` only carries the
                // order id and fill, not the originating order.
                self.pending_orders
                    .insert(order.id.clone(), (order.symbol.clone(), order.side.clone()));

                // Pre-trade risk validation
                self.validate_order_placement(&order).await?;
            }
            MarketEvent::OrderFilled { order_id, fill } => {
                // Update risk metrics after fill
                self.update_on_fill(order_id.clone(), &fill).await?;
                // Record pattern trade outcome if applicable
                self.record_pattern_trade_outcome(order_id, &fill).await?;
            }
            MarketEvent::OrderCancelled { order_id, .. }
            | MarketEvent::OrderRejected { order_id, .. } => {
                // Terminal non-fill states: reclaim the placement metadata,
                // otherwise every cancelled/rejected order leaks an entry
                // and pending_orders grows without bound in a long-running
                // process.
                self.pending_orders.remove(&order_id);
            }
            MarketEvent::PositionUpdated { position } => {
                // Update P&L tracking
                self.update_position_pl(&position).await?;
            }
            MarketEvent::PositionClosed {
                symbol,
                realized_pl,
            } => {
                // Update daily P&L
                self.update_daily_pl(realized_pl).await?;
                // Update pattern statistics from position close (assuming pattern association via symbol or metadata)
                // For now, general update; pattern-specific requires trade-pattern linking
                self.update_pattern_stats_from_close(symbol, realized_pl)
                    .await?;
            }
            _ => {
                // Other events may be relevant in future phases
            }
        }

        Ok(())
    }

    /// Predicts whether filling `order_side` for `symbol` right now would
    /// complete a day trade, without mutating any tracked position state.
    ///
    /// This mirrors the FIFO same-day-open/close logic in
    /// [`crate::risk::types::TrackedPosition::update_with_fill`] but only
    /// reads the tracker: a fill in the same direction as the current
    /// position (or into a flat position) always opens a new lot and can
    /// never be a day trade; a fill in the opposite direction closes
    /// existing lots FIFO and is a day trade if any lot it would consume
    /// was opened today.
    ///
    /// # Errors
    ///
    /// Returns an error only if the underlying position lookup fails.
    async fn predicts_day_trade(
        &self,
        symbol: &Symbol,
        order_side: &OrderSide,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let Some(position) = self.position_tracker.get_position(symbol).await else {
            // No tracked position: this fill would open a new position, never a day trade.
            return Ok(false);
        };

        let opening_side = match order_side {
            OrderSide::Buy => crate::risk::types::PositionSide::Long,
            OrderSide::Sell => crate::risk::types::PositionSide::Short,
        };

        // Same direction as the current position (or flat): this fill opens
        // or adds to a lot, it does not close anything.
        if position.side == crate::risk::types::PositionSide::Flat || position.side == opening_side
        {
            return Ok(false);
        }

        // Opposite direction: this fill would close existing FIFO lots. It is
        // a day trade if any lot opened today would be consumed. Since the
        // FIFO queue is consumed oldest-first and we don't know the fill
        // quantity here (this runs pre-trade, before the exact fill is
        // known), a same-day lot anywhere in the position is a sufficient
        // signal — any close touches the front of the queue first, and if a
        // same-day lot exists it will be reached unless the order is smaller
        // than the older, cross-day lots ahead of it. Position sizing risk
        // rules already validate order quantity against the position, so
        // conservatively predicting a day trade here (rather than assuming
        // none) is the safe direction for PDT compliance.
        let today = Utc::now().date_naive();
        Ok(position.has_components_opened_on_date(today))
    }

    /// Validates order placement against risk rules
    async fn validate_order_placement(
        &self,
        order: &Arc<crate::events::types::Order>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_state = self.state.read().await;

        // Check if trading is halted
        if current_state.is_trading_halted {
            return Err("Trading is currently halted due to risk violation".into());
        }

        // Predict whether this order would complete a day trade if filled now.
        // A closing (or reversing) fill against a position with any lot opened
        // today counts as a day trade under the PDT rule; this is a read-only
        // lookahead against the position tracker (no state mutation — the
        // authoritative update happens in `update_on_fill` once the fill is real).
        let would_be_day_trade = self.predicts_day_trade(&order.symbol, &order.side).await?;

        // Create trade context for risk evaluation
        let context = TradeContext::new(
            order.symbol.to_string(),
            Decimal::from(order.quantity.value()),
            order.price.map(|p| p.value()).unwrap_or(Decimal::ZERO), // Use limit price or assume market order
            current_state.daily_pl, // Use current equity approximation
            would_be_day_trade,
        );

        // Evaluate all risk rules
        let violations = self.rule_engine.evaluate_violations_only(context).await;

        // Check for critical violations that should block the trade
        for (rule_id, evaluation) in violations {
            if let Some(severity) = evaluation.severity() {
                if matches!(severity, ViolationSeverity::Critical) {
                    let violation = RiskViolation {
                        violation_type: match rule_id {
                            RiskRuleId::DailyLossLimit => RiskViolationType::DailyLossLimit,
                            RiskRuleId::PositionSizeLimit => RiskViolationType::PositionSizeLimit,
                            RiskRuleId::PdtLimit => RiskViolationType::PdtLimit,
                            RiskRuleId::PatternSpecificLimit => {
                                RiskViolationType::PatternPerformanceThreshold
                            }
                            RiskRuleId::AccountSafeguard => RiskViolationType::InsufficientFunds,
                        },
                        message: crate::brokers::types::Message::new(
                            evaluation
                                .message
                                .unwrap_or_else(|| format!("{:?} violation", rule_id)),
                        ),
                        timestamp: Utc::now(),
                        pattern_name: None,
                    };

                    // Publish risk violation event
                    let event = MarketEvent::RiskViolation {
                        violation: violation.clone(),
                    };
                    self.event_bus.publish(event);

                    // Store risk violation in database if available
                    if let Some(ref performance_repo) = self.performance_repo {
                        if let Err(e) = performance_repo.store_risk_event(&violation).await {
                            tracing::warn!("Failed to persist risk violation to database: {}", e);
                        }
                    }

                    return Err(format!("Critical risk violation: {:?}", rule_id).into());
                }
            }
        }

        Ok(())
    }

    /// Updates risk metrics after order fill
    async fn update_on_fill(
        &self,
        order_id: crate::brokers::types::OrderId,
        fill: &crate::events::types::Fill,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Recover the symbol/side this fill belongs to. `OrderFilled` events
        // only carry the fill itself, so the placement-time metadata cached
        // in `pending_orders` is the only place this is available. Read
        // without removing: `process_fill` is fallible, and reclaiming the
        // entry before it succeeds would strand a retried fill in the
        // metadata-less fallback below.
        let metadata = self
            .pending_orders
            .get(&order_id)
            .map(|entry| entry.value().clone());
        if let Some((symbol, side)) = metadata {
            // Authoritative day-trade detection: feed the real fill into the
            // FIFO position tracker and record a day trade only when it
            // actually completes one (same-day open + close). This replaces
            // the pre-trade `predicts_day_trade` lookahead with the ground
            // truth, and is the single source of truth fed to the PDT
            // enforcer so the two counters never diverge.
            let is_day_trade = self
                .position_tracker
                .process_fill(
                    &symbol,
                    side,
                    u64::from(fill.filled_quantity.value()),
                    fill.filled_price.value(),
                    fill.filled_at,
                )
                .await?;

            // Fill accepted by the tracker — now the metadata can go.
            self.pending_orders.remove(&order_id);

            if is_day_trade {
                self.pdt_enforcer
                    .record_day_trade(fill.filled_at.date_naive())
                    .await?;
            }
        } else {
            // We should always have placement metadata for a fill we
            // process, but if it's missing (e.g. engine restart mid-order)
            // fail safe: without position lifecycle context we cannot rule
            // out a day trade, and PDT compliance must never be bypassed.
            // The position tracker cannot be updated without symbol/side,
            // so its view of this symbol is now stale — log every detail
            // needed for manual reconciliation.
            tracing::warn!(
                order_id = %order_id,
                filled_quantity = fill.filled_quantity.value(),
                filled_price = %fill.filled_price.value(),
                filled_at = %fill.filled_at,
                "No pending order metadata for fill; conservatively recording \
                 a day trade. Position tracker NOT updated for this fill — \
                 its state for the affected symbol is stale until reconciled"
            );
            self.pdt_enforcer
                .record_day_trade(fill.filled_at.date_naive())
                .await?;
        }

        // Fills carry realized P&L via the commission field (negative for
        // losses, positive for profits — established event convention).
        // Route it through the same path as position closes so loss-limit
        // halts propagate to engine state and the bus.
        self.update_daily_pl(fill.commission).await?;

        Ok(())
    }

    /// Updates position P&L tracking
    async fn update_position_pl(
        &self,
        position: &crate::events::types::Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut current_state = self.state.write().await;

        // Update position in position sizer for portfolio allocation tracking
        self.position_sizer
            .update_position(
                position.symbol.to_string(),
                Decimal::from(position.quantity.value()),
                position.average_entry_price.value(),
                position.current_price.value(),
            )
            .await;

        // Update unrealized P&L in daily loss tracker
        let unrealized_change = position.unrealized_pl;
        self.daily_loss_tracker
            .update_unrealized_pnl(unrealized_change)
            .await;

        current_state.last_updated = Utc::now();

        // Unrealized losses count toward the daily limit too; propagate a
        // tracker halt to engine state just like the realized path does
        if self.config.halt_trading_on_limit
            && !current_state.is_trading_halted
            && self.daily_loss_tracker.should_halt_trading().await
        {
            current_state.is_trading_halted = true;
            tracing::error!("Trading halted: daily loss limit exceeded (unrealized)");
            self.event_bus.publish(MarketEvent::TradingHalted {
                reason: crate::brokers::types::Reason::new(
                    "Daily loss limit exceeded (unrealized)",
                ),
            });
        }

        Ok(())
    }

    /// Updates daily realized P&L
    async fn update_daily_pl(
        &self,
        realized_pl: Decimal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update daily P&L in loss tracker; it reports when limits trip
        let evaluation = self.daily_loss_tracker.update_daily_pnl(realized_pl).await;

        // Scope the state guard: check_emergency_liquidation below reaches
        // halt_trading(), which takes this same write lock — holding the
        // guard across that call self-deadlocks the risk engine.
        let daily_pl = {
            let mut current_state = self.state.write().await;

            current_state.daily_pl += realized_pl;
            current_state.last_updated = Utc::now();

            // Propagate loss-limit halts to the engine's own state so
            // is_trading_allowed() and pre-trade order checks observe them.
            // Without this the tracker's halt was invisible to callers.
            if self.config.halt_trading_on_limit
                && !current_state.is_trading_halted
                && matches!(
                    evaluation,
                    crate::risk::types::RiskEvaluation::Violated { .. }
                )
            {
                current_state.is_trading_halted = true;
                tracing::error!(
                    daily_pl = %current_state.daily_pl,
                    "Trading halted: daily loss limit exceeded"
                );
                self.event_bus.publish(MarketEvent::TradingHalted {
                    reason: crate::brokers::types::Reason::new("Daily loss limit exceeded"),
                });
            }

            current_state.daily_pl
        };

        // Check for emergency liquidation (may halt; takes the lock itself).
        // The threshold is a BALANCE floor: compare equity + daily P&L, not
        // the day's P&L alone — passing daily_pl false-triggered a halt on
        // any loss past the threshold.
        self.check_emergency_liquidation(self.initial_equity + daily_pl)
            .await?;

        Ok(())
    }

    /// Records pattern signal detection for statistics tracking
    async fn record_pattern_signal(
        &self,
        signal: &crate::events::types::PatternSignal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut stats_tracker = self.pattern_stats_tracker.write().await;
        stats_tracker.record_signal(&signal.pattern_name);

        // Persist the signal detection to database if available. This stores
        // exactly the fields the risk engine actually observes on the event
        // (see `PatternSignal` in events::types) rather than fabricating a
        // full patterns::signals::TradingSignal, which would require values
        // (signal direction, market context) this layer doesn't have.
        if let Some(ref signal_repo) = self.signal_repo {
            if let Err(e) = signal_repo
                .store_signal_detection(
                    signal.symbol.as_str(),
                    signal.pattern_name.as_str(),
                    signal.signal_type.as_str(),
                    signal.confidence.value(),
                    signal.detected_at,
                    signal.price.value().to_f64().unwrap_or(0.0),
                )
                .await
            {
                tracing::warn!("Failed to persist signal to database: {}", e);
            }
        }

        Ok(())
    }

    /// Real-time risk evaluation on pattern detection
    async fn evaluate_pattern_risk_on_detection(
        &self,
        signal: &crate::events::types::PatternSignal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut evaluator = self.pattern_risk_evaluator.write().await;
        let mut stats_tracker = self.pattern_stats_tracker.write().await;

        // Evaluate if pattern is tradable
        // Estimate order value based on typical position size (e.g., from config)
        let estimated_order_value = Decimal::new(5000i64, 2); // $50.00 placeholder

        let evaluation = evaluator
            .evaluate_pattern_trade(
                &signal.pattern_name,
                signal.confidence.value(),
                estimated_order_value,
            )
            .await;

        match evaluation {
            RiskEvaluation::Passed => {
                // Pattern is good to trade, no action needed
            }
            RiskEvaluation::Violated {
                severity,
                suggested_action,
            } => {
                // Publish pattern-specific violation
                let violation = RiskViolation {
                    violation_type: RiskViolationType::PatternDisabled, // or appropriate type
                    message: Message::new(format!(
                        "Pattern risk violation: {} - {}",
                        signal.pattern_name.as_str(),
                        suggested_action
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or_default()
                    )),
                    timestamp: Utc::now(),
                    pattern_name: Some(signal.pattern_name.clone()),
                };

                let event = MarketEvent::RiskViolation { violation };
                self.event_bus.publish(event);

                // If critical, disable pattern
                if matches!(severity, ViolationSeverity::Critical) {
                    evaluator.update_pattern_status(
                        &signal.pattern_name,
                        0.0, // Force disable
                        0,
                    );
                    stats_tracker.update_pattern_status(&signal.pattern_name, 0.0, 0);

                    // Publish PatternDisabled event
                    let disable_event = MarketEvent::PatternDisabled {
                        pattern_name: signal.pattern_name.clone(),
                        reason: format!("Risk violation: {}", suggested_action.unwrap_or_default()),
                    };
                    self.event_bus.publish(disable_event);
                }
            }
        }

        // Update performance-based multipliers periodically or on detection
        evaluator.update_performance_based_multipliers().await;

        // Check for status changes and publish events
        if let Some(state) = evaluator.get_pattern_state(&signal.pattern_name) {
            if !state.is_enabled {
                let disable_event = MarketEvent::PatternDisabled {
                    pattern_name: signal.pattern_name.clone(),
                    reason: "Performance-based disable".to_string(),
                };
                self.event_bus.publish(disable_event);
            } else {
                // Check if recently enabled
                // (Would need previous state tracking, simplified here)
            }

            // Publish performance update if significant change
            let stats = stats_tracker
                .get_pattern_stats(&signal.pattern_name)
                .cloned();
            if let Some(s) = stats {
                if s.total_trades > 0 {
                    let perf_event = MarketEvent::PatternPerformanceUpdate {
                        pattern_name: signal.pattern_name.clone(),
                        win_rate: s.win_rate,
                        total_trades: s.total_trades,
                        recent_pl: s.total_profit,
                    };
                    self.event_bus.publish(perf_event);
                }

                // Persist pattern statistics to database
                if let Some(ref pattern_repo) = self.pattern_repo {
                    if let Err(e) = pattern_repo.store_pattern_stats(&s).await {
                        tracing::warn!("Failed to persist pattern stats to database: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Records pattern trade outcome for performance tracking
    async fn record_pattern_trade_outcome(
        &self,
        order_id: crate::brokers::types::OrderId,
        fill: &crate::events::types::Fill,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Determine pattern association (e.g., via order metadata or pending trades map)
        // For Phase 2B-3, implement basic update assuming pattern lookup
        // Placeholder: assume a default pattern or skip pattern-specific for non-linked trades

        // Example implementation assuming pattern lookup function (to be implemented)
        // let (pattern_name, confidence) = self.lookup_pattern_for_order(&order_id).await?;
        // if let (Some(pattern_name), Some(confidence)) = (pattern_name, confidence) {
        //     let profit = fill.filled_price.value() * Decimal::from(fill.filled_quantity.value()) - fill.commission;
        //
        //     let mut stats_tracker = self.pattern_stats_tracker.write().await;
        //     stats_tracker.record_trade_outcome(&pattern_name, confidence, profit);
        //
        //     let mut evaluator = self.pattern_risk_evaluator.write().await;
        //     evaluator.record_pattern_trade(&pattern_name, profit > Decimal::ZERO);
        //
        //     // Check for multiplier adjustment and publish
        //     evaluator.update_performance_based_multipliers();
        //     if let Some(state) = evaluator.get_pattern_state(&pattern_name) {
        //         if state.current_risk_multiplier != 1.0 {  // Assuming previous was 1.0
        //             let adjust_event = MarketEvent::RiskMultiplierAdjusted {
        //                 pattern_name: pattern_name.clone(),
        //                 old_multiplier: Decimal::ONE,
        //                 new_multiplier: Decimal::from_f64_retain(state.current_risk_multiplier).unwrap_or(Decimal::ONE),
        //                 reason: "Trade outcome adjustment".to_string(),
        //             };
        //             self.event_bus.publish(adjust_event);
        //         }
        //     }
        // }

        // For now, update general statistics
        Ok(())
    }

    /// Updates pattern statistics from position close
    async fn update_pattern_stats_from_close(
        &self,
        symbol: Symbol,
        realized_pl: Decimal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Associate closed position with originating pattern
        // Placeholder: general PL update to risk state, pattern-specific requires linking

        // Example: if pattern known for symbol/position
        // let (pattern_name, confidence) = self.lookup_pattern_for_position(&symbol).await?;
        // if let (Some(p), Some(c)) = (pattern_name, confidence) {
        //     let mut stats = self.pattern_stats_tracker.write().await;
        //     stats.record_trade_outcome(&p, c, realized_pl);
        // }

        Ok(())
    }

    /// Checks for emergency liquidation conditions
    async fn check_emergency_liquidation(
        &self,
        current_balance: Decimal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let threshold = Decimal::from_f64_retain(self.config.emergency_liquidate_threshold)
            .unwrap_or(Decimal::ZERO);

        if current_balance <= threshold {
            tracing::error!(
                "Emergency liquidation triggered: balance {} below threshold {}",
                current_balance,
                threshold
            );

            // Halt trading immediately
            self.halt_trading("Emergency liquidation: account balance below threshold")
                .await;

            // TODO: Implement actual liquidation logic
            // This would involve:
            // 1. Getting all open positions
            // 2. Submitting market sell orders for all positions
            // 3. Monitoring order fills
            // 4. Updating risk state

            // For now, just log and halt
            let event = MarketEvent::EmergencyLiquidation {
                reason: format!(
                    "Account balance {} below emergency threshold {}",
                    current_balance, threshold
                ),
                timestamp: Utc::now(),
            };
            self.event_bus.publish(event);
        }

        Ok(())
    }

    /// Gets current risk state (read-only)
    pub async fn get_risk_state(&self) -> RiskState {
        self.state.read().await.clone()
    }

    /// Gets a reference to the risk configuration
    pub fn get_config(&self) -> &RiskConfig {
        &self.config
    }

    /// Checks if trading is currently allowed
    pub async fn is_trading_allowed(&self) -> bool {
        let state = self.state.read().await;
        !state.is_trading_halted
    }

    /// Gets access to the pattern risk evaluator (for internal use)
    pub fn get_pattern_risk_evaluator(
        &self,
    ) -> &Arc<RwLock<crate::risk::pattern_limits::PatternRiskEvaluator>> {
        &self.pattern_risk_evaluator
    }

    /// Gets access to the position sizer (for internal use)
    pub fn get_position_sizer(&self) -> &Arc<PositionSizer> {
        &self.position_sizer
    }

    /// Gets the engine's position tracker
    ///
    /// The risk engine feeds this tracker from `OrderFilled` events; consumers
    /// (execution engine, UI state) must share this instance rather than
    /// constructing their own, or they will never observe fills.
    pub fn position_tracker(&self) -> &Arc<PositionTracker> {
        &self.position_tracker
    }

    /// Gets access to the risk configuration
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }

    /// Manually halts trading (emergency stop)
    pub async fn halt_trading(&self, reason: impl Into<String>) {
        let mut state = self.state.write().await;
        state.is_trading_halted = true;
        state.last_updated = Utc::now();

        // Publish trading halted event
        let event = MarketEvent::TradingHalted {
            reason: crate::brokers::types::Reason::new(reason),
        };

        self.event_bus.publish(event);
    }

    /// Resumes trading after halt
    pub async fn resume_trading(&self) {
        let mut state = self.state.write().await;
        state.is_trading_halted = false;
        state.last_updated = Utc::now();

        // Publish trading resumed event
        let event = MarketEvent::TradingResumed;

        self.event_bus.publish(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::types::AccountId as BrokerAccountId;

    #[tokio::test]
    async fn test_risk_engine_creation() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let initial_equity = Decimal::new(5000000, 2); // $50,000.00

        let engine = RiskEngine::new(config, event_bus, account_id, initial_equity);

        let state = engine.get_risk_state().await;
        assert_eq!(state.daily_pl, Decimal::ZERO);
        assert_eq!(state.day_trade_count, 0);
        assert!(!state.is_trading_halted);
    }

    #[tokio::test]
    async fn test_position_tracker_accessor_returns_shared_instance() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let engine = RiskEngine::new(
            config,
            event_bus,
            AccountId::new("test-account"),
            Decimal::new(5000000, 2),
        );

        let symbol = crate::brokers::types::Symbol::new("AAPL").unwrap();
        let tracker = Arc::clone(engine.position_tracker());
        tracker
            .process_fill(
                &symbol,
                crate::events::types::OrderSide::Buy,
                100,
                Decimal::new(15000, 2),
                Utc::now(),
            )
            .await
            .unwrap();

        // A fill applied through the returned handle must be visible via the
        // engine's own tracker — same instance, not a copy
        let position = engine.position_tracker().get_position(&symbol).await;
        assert!(position.is_some(), "fill must land in the shared tracker");
    }

    #[tokio::test]
    async fn test_trading_halt_resume() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let initial_equity = Decimal::new(5000000, 2); // $50,000.00

        let engine = RiskEngine::new(config, event_bus, account_id, initial_equity);

        // Initially trading should be allowed
        assert!(engine.is_trading_allowed().await);

        // Halt trading
        engine.halt_trading("Test halt").await;
        assert!(!engine.is_trading_allowed().await);

        // Resume trading
        engine.resume_trading().await;
        assert!(engine.is_trading_allowed().await);
    }

    /// Helper function to create a test risk configuration
    fn create_test_risk_config() -> crate::config::RiskConfig {
        crate::config::RiskConfig {
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
            pattern_profiles: std::collections::HashMap::new(),
            confidence_multipliers: vec![],
            pattern_performance_thresholds:
                crate::risk::types::PatternPerformanceThresholds::default(),
        }
    }

    /// Builds a minimal `Order` for placement/fill tests
    fn create_test_order(
        id: &str,
        symbol: &str,
        side: OrderSide,
    ) -> Arc<crate::events::types::Order> {
        Arc::new(crate::events::types::Order {
            id: crate::brokers::types::OrderId::new(id),
            symbol: Symbol::new(symbol).unwrap(),
            side,
            order_type: crate::events::types::OrderType::Market,
            quantity: crate::brokers::types::Quantity::new(100).unwrap(),
            price: None,
            status: crate::events::types::OrderStatus::Submitted,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Builds a `Fill` for a given order at a given fill time
    fn create_test_fill(
        order_id: &str,
        price: Decimal,
        filled_at: DateTime<Utc>,
    ) -> crate::events::types::Fill {
        crate::events::types::Fill {
            order_id: crate::brokers::types::OrderId::new(order_id),
            filled_quantity: crate::brokers::types::Quantity::new(100).unwrap(),
            filled_price: crate::brokers::types::Price::new(price).unwrap(),
            filled_at,
            commission: Decimal::ZERO,
        }
    }

    /// Places an order (registering it in `pending_orders`, mirroring what
    /// `process_event` does for `MarketEvent::OrderPlaced`) without running
    /// full risk-rule evaluation, so tests can drive `update_on_fill`
    /// directly against a known order/side.
    fn register_pending_order(engine: &RiskEngine, order: &crate::events::types::Order) {
        engine
            .pending_orders
            .insert(order.id.clone(), (order.symbol.clone(), order.side.clone()));
    }

    #[tokio::test]
    async fn test_predicts_day_trade_flat_position_returns_false() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(5000000, 2));

        let symbol = Symbol::new("AAPL").unwrap();
        let result = engine
            .predicts_day_trade(&symbol, &OrderSide::Buy)
            .await
            .unwrap();
        assert!(
            !result,
            "no tracked position means opening, never a day trade"
        );
    }

    #[tokio::test]
    async fn test_predicts_day_trade_same_day_open_returns_true() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(5000000, 2));

        let symbol = Symbol::new("AAPL").unwrap();
        // Open a long position today via the tracker directly.
        engine
            .position_tracker
            .process_fill(
                &symbol,
                OrderSide::Buy,
                100,
                Decimal::new(15000, 2),
                Utc::now(),
            )
            .await
            .unwrap();

        // A sell (closing) order against a same-day lot should predict a day trade.
        let result = engine
            .predicts_day_trade(&symbol, &OrderSide::Sell)
            .await
            .unwrap();
        assert!(result, "closing a same-day lot should predict a day trade");

        // A further buy (adding to the position) should not.
        let result = engine
            .predicts_day_trade(&symbol, &OrderSide::Buy)
            .await
            .unwrap();
        assert!(!result, "adding to an existing position is not a day trade");
    }

    #[tokio::test]
    async fn test_predicts_day_trade_different_day_returns_false() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(5000000, 2));

        let symbol = Symbol::new("AAPL").unwrap();
        let yesterday = Utc::now() - chrono::Duration::days(1);
        engine
            .position_tracker
            .process_fill(
                &symbol,
                OrderSide::Buy,
                100,
                Decimal::new(15000, 2),
                yesterday,
            )
            .await
            .unwrap();

        let result = engine
            .predicts_day_trade(&symbol, &OrderSide::Sell)
            .await
            .unwrap();
        assert!(
            !result,
            "closing a lot opened on a prior day is not a day trade"
        );
    }

    #[tokio::test]
    async fn test_same_day_open_close_increments_pdt_count() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(2000000, 2));

        let now = Utc::now();

        // Open long today.
        let open_order = create_test_order("order-1", "AAPL", OrderSide::Buy);
        register_pending_order(&engine, &open_order);
        let open_fill = create_test_fill("order-1", Decimal::new(15000, 2), now);
        engine
            .update_on_fill(open_order.id.clone(), &open_fill)
            .await
            .unwrap();

        assert_eq!(
            engine.pdt_enforcer.count_day_trades().await,
            0,
            "opening a position is never a day trade"
        );

        // Close it same day.
        let close_order = create_test_order("order-2", "AAPL", OrderSide::Sell);
        register_pending_order(&engine, &close_order);
        let close_fill = create_test_fill("order-2", Decimal::new(15100, 2), now);
        engine
            .update_on_fill(close_order.id.clone(), &close_fill)
            .await
            .unwrap();

        assert_eq!(
            engine.pdt_enforcer.count_day_trades().await,
            1,
            "same-day open+close must be recorded as one day trade"
        );
    }

    #[tokio::test]
    async fn test_different_day_close_does_not_increment_pdt_count() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(2000000, 2));

        let yesterday = Utc::now() - chrono::Duration::days(1);
        let today = Utc::now();

        // Open long yesterday.
        let open_order = create_test_order("order-1", "AAPL", OrderSide::Buy);
        register_pending_order(&engine, &open_order);
        let open_fill = create_test_fill("order-1", Decimal::new(15000, 2), yesterday);
        engine
            .update_on_fill(open_order.id.clone(), &open_fill)
            .await
            .unwrap();

        // Close it today.
        let close_order = create_test_order("order-2", "AAPL", OrderSide::Sell);
        register_pending_order(&engine, &close_order);
        let close_fill = create_test_fill("order-2", Decimal::new(15100, 2), today);
        engine
            .update_on_fill(close_order.id.clone(), &close_fill)
            .await
            .unwrap();

        assert_eq!(
            engine.pdt_enforcer.count_day_trades().await,
            0,
            "closing a position opened on a prior day must not count as a day trade"
        );
    }

    #[tokio::test]
    async fn test_short_round_trip_counts_as_day_trade() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(2000000, 2));

        let now = Utc::now();

        // Sell to open a short position today.
        let open_order = create_test_order("order-1", "TSLA", OrderSide::Sell);
        register_pending_order(&engine, &open_order);
        let open_fill = create_test_fill("order-1", Decimal::new(25000, 2), now);
        engine
            .update_on_fill(open_order.id.clone(), &open_fill)
            .await
            .unwrap();

        assert_eq!(engine.pdt_enforcer.count_day_trades().await, 0);

        // Buy to cover same day.
        let cover_order = create_test_order("order-2", "TSLA", OrderSide::Buy);
        register_pending_order(&engine, &cover_order);
        let cover_fill = create_test_fill("order-2", Decimal::new(24900, 2), now);
        engine
            .update_on_fill(cover_order.id.clone(), &cover_fill)
            .await
            .unwrap();

        assert_eq!(
            engine.pdt_enforcer.count_day_trades().await,
            1,
            "same-day short open + cover must be recorded as one day trade"
        );
    }

    #[tokio::test]
    async fn test_update_on_fill_removes_pending_order_metadata() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(2000000, 2));

        let order = create_test_order("order-1", "AAPL", OrderSide::Buy);
        register_pending_order(&engine, &order);
        assert!(engine.pending_orders.contains_key(&order.id));

        let fill = create_test_fill("order-1", Decimal::new(15000, 2), Utc::now());
        engine
            .update_on_fill(order.id.clone(), &fill)
            .await
            .unwrap();

        assert!(
            !engine.pending_orders.contains_key(&order.id),
            "pending order metadata should be consumed once the fill is processed"
        );
    }

    #[tokio::test]
    async fn test_cancelled_and_rejected_orders_reclaim_pending_metadata() {
        let config = create_test_risk_config();
        let event_bus = Arc::new(EventBus::new(1000));
        let account_id = AccountId::new("test-account");
        let engine = RiskEngine::new(config, event_bus, account_id, Decimal::new(5000000, 2));

        let cancelled = create_test_order("order-cancel", "AAPL", OrderSide::Buy);
        let rejected = create_test_order("order-reject", "MSFT", OrderSide::Sell);
        register_pending_order(&engine, &cancelled);
        register_pending_order(&engine, &rejected);

        engine
            .process_event(crate::events::types::MarketEvent::OrderCancelled {
                order_id: cancelled.id.clone(),
                reason: crate::brokers::types::Reason::new("user cancelled"),
            })
            .await
            .unwrap();
        engine
            .process_event(crate::events::types::MarketEvent::OrderRejected {
                order_id: rejected.id.clone(),
                reason: crate::brokers::types::Reason::new("insufficient funds"),
            })
            .await
            .unwrap();

        assert!(
            !engine.pending_orders.contains_key(&cancelled.id),
            "cancelled order must not leak pending metadata"
        );
        assert!(
            !engine.pending_orders.contains_key(&rejected.id),
            "rejected order must not leak pending metadata"
        );
    }
}
