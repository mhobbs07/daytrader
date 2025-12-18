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

use crate::config::RiskConfig;
use crate::events::bus::EventBus;
use crate::events::types::{MarketEvent, RiskViolation, RiskViolationType};
use crate::risk::limits::{DailyLossTracker, DailyLossLimits};
use crate::risk::pdt::{PdtEnforcer, PdtConfig};
use crate::risk::position_sizing::{PositionSizer, PositionSizeLimits};
use crate::risk::rules::{RiskRuleEngine, TradeContext};
use crate::risk::types::*;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
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
    // Pattern-specific risk management
    pattern_stats_tracker: Arc<RwLock<crate::risk::pattern_stats::PatternStatsTracker>>,
    pattern_risk_evaluator: Arc<RwLock<crate::risk::pattern_limits::PatternRiskEvaluator>>,
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
    pub fn new(config: RiskConfig, event_bus: Arc<EventBus>, account_id: AccountId, initial_equity: Decimal) -> Self {
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

        let daily_limits = DailyLossLimits::from_config(&config, initial_equity);
        let daily_loss_tracker = Arc::new(DailyLossTracker::new(daily_limits));

        let position_limits = PositionSizeLimits::from_config(&config);
        let position_sizer = Arc::new(PositionSizer::new(position_limits, initial_equity));

        let pdt_config = PdtConfig::from_config(&config);
        let pdt_enforcer = Arc::new(PdtEnforcer::new(pdt_config, initial_equity));

        // Initialize pattern statistics tracker
        let pattern_stats_tracker = Arc::new(RwLock::new(crate::risk::pattern_stats::PatternStatsTracker::new()));

        // Initialize pattern risk evaluator with stats tracker
        let pattern_config = crate::risk::pattern_limits::PatternRiskConfig::from_config(&config);
        let mut pattern_risk_evaluator = crate::risk::pattern_limits::PatternRiskEvaluator::new(pattern_config);
        pattern_risk_evaluator.set_stats_tracker(crate::risk::pattern_stats::PatternStatsTracker::new());
        let pattern_risk_evaluator = Arc::new(RwLock::new(pattern_risk_evaluator));

        Self {
            config,
            event_bus,
            state: Arc::new(RwLock::new(state)),
            rule_engine,
            daily_loss_tracker,
            position_sizer,
            pdt_enforcer,
            pattern_stats_tracker,
            pattern_risk_evaluator,
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
        let pattern_stats_tracker = Arc::clone(&self.pattern_stats_tracker);
        let pattern_risk_evaluator = Arc::clone(&self.pattern_risk_evaluator);

        // Create a new rule engine for the async task (can't clone the existing one)
        let rule_engine = crate::risk::rules::RiskRuleEngine::new(config.clone());

        let engine = Arc::new(RiskEngine {
            config,
            event_bus: Arc::clone(&self.event_bus),
            state,
            rule_engine,
            daily_loss_tracker,
            position_sizer,
            pdt_enforcer,
            pattern_stats_tracker,
            pattern_risk_evaluator,
        });

        // Spawn event processing task
        tokio::spawn(async move {
            let mut receiver = event_bus.subscribe();

            while let Ok(event) = receiver.recv().await {
                if let Err(e) = engine.process_event(event).await {
                    tracing::error!("Risk engine event processing error: {}", e);
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
            }
            MarketEvent::OrderPlaced { order } => {
                // Pre-trade risk validation
                self.validate_order_placement(&order).await?;
            }
            MarketEvent::OrderFilled { order_id, fill } => {
                // Update risk metrics after fill
                self.update_on_fill(order_id, &fill).await?;
                // Record pattern trade outcome if applicable
                self.record_pattern_trade_outcome(order_id, &fill).await?;
            }
            MarketEvent::PositionUpdated { position } => {
                // Update P&L tracking
                self.update_position_pl(&position).await?;
            }
            MarketEvent::PositionClosed { symbol, realized_pl } => {
                // Update daily P&L
                self.update_daily_pl(realized_pl).await?;
            }
            _ => {
                // Other events may be relevant in future phases
            }
        }

        Ok(())
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

        // Create trade context for risk evaluation
        let context = TradeContext::new(
            order.symbol.to_string(),
            Decimal::from(order.quantity.value()),
            order.price.map(|p| p.value()).unwrap_or(Decimal::ZERO), // Use limit price or assume market order
            current_state.daily_pl, // Use current equity approximation
            false, // TODO: Implement day trade detection
        );

        // Evaluate all risk rules
        let violations = self.rule_engine.evaluate_violations_only(context).await;

        // Check for critical violations that should block the trade
        for (rule_id, evaluation) in violations {
            if let Some(severity) = evaluation.severity() {
                if matches!(severity, ViolationSeverity::Critical) {
                    let violation = RiskViolation {
                        violation_type: match rule_id {
                            RiskRuleId::DailyLossLimit => RiskViolationType::Other, // TODO: Add specific types
                            RiskRuleId::PositionSizeLimit => RiskViolationType::PositionSizeLimit,
                            RiskRuleId::PdtLimit => RiskViolationType::PdtLimit,
                            _ => RiskViolationType::Other,
                        },
                        message: crate::brokers::types::Message::new(
                            evaluation.message.unwrap_or_else(|| format!("{:?} violation", rule_id))
                        ),
                        timestamp: Utc::now(),
                    };

                    // Publish risk violation event
                    let event = MarketEvent::RiskViolation { violation };
                    self.event_bus.publish(event);

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
        let mut current_state = self.state.write().await;

        // Update day trade count if applicable (simplified - assume all fills are day trades for now)
        // TODO: Implement proper day trade detection based on position lifecycle
        self.pdt_enforcer.record_day_trade(Utc::now().date_naive()).await;

        // Update daily P&L (realized P&L from commissions)
        let pl_change = fill.commission; // Negative for losses, positive for profits
        self.daily_loss_tracker.update_daily_pnl(pl_change).await;

        current_state.daily_pl += pl_change;
        current_state.last_updated = Utc::now();

        Ok(())
    }

    /// Updates position P&L tracking
    async fn update_position_pl(
        &self,
        position: &crate::events::types::Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut current_state = self.state.write().await;

        // Update position in position sizer for portfolio allocation tracking
        self.position_sizer.update_position(
            position.symbol.to_string(),
            Decimal::from(position.quantity.value()),
            position.average_entry_price.value(),
            position.current_price.value(),
        ).await;

        // Update unrealized P&L in daily loss tracker
        let unrealized_change = position.unrealized_pl;
        self.daily_loss_tracker.update_unrealized_pnl(unrealized_change).await;

        current_state.last_updated = Utc::now();

        Ok(())
    }

    /// Updates daily realized P&L
    async fn update_daily_pl(
        &self,
        realized_pl: Decimal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut current_state = self.state.write().await;

        // Update daily P&L in loss tracker
        self.daily_loss_tracker.update_daily_pnl(realized_pl).await;

        current_state.daily_pl += realized_pl;
        current_state.last_updated = Utc::now();

        Ok(())
    }

    /// Records pattern signal detection for statistics tracking
    async fn record_pattern_signal(
        &self,
        signal: &crate::events::types::PatternSignal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut stats_tracker = self.pattern_stats_tracker.write().await;
        stats_tracker.record_signal(&signal.pattern_name);
        Ok(())
    }

    /// Records pattern trade outcome for performance tracking
    async fn record_pattern_trade_outcome(
        &self,
        order_id: crate::brokers::types::OrderId,
        fill: &crate::events::types::Fill,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: We need to determine if this is a pattern trade and get the pattern name and confidence
        // For now, we'll assume we can look this up from the order or have it passed in the event
        // This would require extending the OrderFilled event to include pattern information

        // For Phase 2B-2 implementation, we'll assume pattern trades are identified by some means
        // and implement the basic structure

        // Placeholder implementation - in real system, this would look up pattern info from order metadata
        // let pattern_name = ...;
        // let confidence = ...;
        // let profit = calculate_profit_from_fill(fill);

        // if let (Some(pattern_name), Some(confidence)) = (pattern_name, confidence) {
        //     let mut stats_tracker = self.pattern_stats_tracker.write().await;
        //     stats_tracker.record_trade_outcome(&pattern_name, confidence, profit);
        //
        //     let mut risk_evaluator = self.pattern_risk_evaluator.write().await;
        //     risk_evaluator.record_pattern_trade(&pattern_name, profit > Decimal::ZERO);
        // }

        Ok(())
    }

    /// Gets current risk state (read-only)
    pub async fn get_risk_state(&self) -> RiskState {
        self.state.read().await.clone()
    }

    /// Checks if trading is currently allowed
    pub async fn is_trading_allowed(&self) -> bool {
        let state = self.state.read().await;
        !state.is_trading_halted
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
            pattern_performance_thresholds: crate::risk::types::PatternPerformanceThresholds::default(),
        }
    }
}