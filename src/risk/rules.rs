//! Risk Rule Integration and Evaluation
//!
//! This module provides a composable risk rule system that allows individual
//! risk rules to be combined and evaluated together for trade validation.
//!
//! # Key Features
//!
//! - **Composable Rules**: Individual risk rules that can be combined
//! - **Rule Evaluation**: Evaluate all applicable rules for a trade
//! - **Violation Handling**: Structured violation responses with actions
//! - **Configuration-Driven**: Rules loaded from config
//! - **Async Evaluation**: All rule evaluation is async for performance
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::rules::{RiskRuleEngine, TradeContext};
//! use daytrader::config::RiskConfig;
//! use rust_decimal::Decimal;
//!
//! let config = RiskConfig::default();
//! let engine = RiskRuleEngine::new(config);
//!
//! let context = TradeContext {
//!     symbol: "AAPL".to_string(),
//!     quantity: Decimal::new(100, 0),
//!     price: Decimal::new(15000, 2),
//!     order_value: Decimal::new(1500000, 2),
//!     current_equity: Decimal::new(5000000, 2),
//!     is_day_trade: false,
//! };
//!
//! // Evaluate all risk rules
//! let results = engine.evaluate_all_rules(context).await;
//!
//! for (rule_id, evaluation) in results {
//!     if !evaluation.is_passed() {
//!         println!("Rule {} violated: {:?}", rule_id, evaluation);
//!     }
//! }
//! ```

use crate::brokers::types::PatternName;
use crate::config::RiskConfig;
use crate::risk::limits::{DailyLossLimits, DailyLossTracker};
use crate::risk::pattern_limits::{PatternRiskConfig, PatternRiskEvaluator};
use crate::risk::pdt::{PdtConfig, PdtEnforcer};
use crate::risk::position_sizing::{PositionSizeLimits, PositionSizer};
use crate::risk::types::{RiskEvaluation, RiskRuleId, ViolationSeverity};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

/// Context information for risk rule evaluation
///
/// Contains all the information needed to evaluate risk rules for a trade.
#[derive(Debug, Clone)]
pub struct TradeContext {
    /// Symbol being traded
    pub symbol: String,
    /// Quantity of shares/contracts
    pub quantity: Decimal,
    /// Price per share/contract
    pub price: Decimal,
    /// Total order value (quantity * price)
    pub order_value: Decimal,
    /// Current account equity
    pub current_equity: Decimal,
    /// Whether this is considered a day trade
    pub is_day_trade: bool,
    /// Pattern name if this is a pattern-based trade
    pub pattern_name: Option<PatternName>,
    /// Pattern confidence score (0.0 to 1.0)
    pub pattern_confidence: Option<f64>,
    /// Current timestamp
    pub timestamp: DateTime<Utc>,
}

impl TradeContext {
    /// Creates a new trade context
    pub fn new(
        symbol: String,
        quantity: Decimal,
        price: Decimal,
        current_equity: Decimal,
        is_day_trade: bool,
    ) -> Self {
        let order_value = quantity * price;
        Self {
            symbol,
            quantity,
            price,
            order_value,
            current_equity,
            is_day_trade,
            pattern_name: None,
            pattern_confidence: None,
            timestamp: Utc::now(),
        }
    }

    /// Creates a new trade context for pattern-based trades
    pub fn new_with_pattern(
        symbol: String,
        quantity: Decimal,
        price: Decimal,
        current_equity: Decimal,
        is_day_trade: bool,
        pattern_name: PatternName,
        pattern_confidence: f64,
    ) -> Self {
        let order_value = quantity * price;
        Self {
            symbol,
            quantity,
            price,
            order_value,
            current_equity,
            is_day_trade,
            pattern_name: Some(pattern_name),
            pattern_confidence: Some(pattern_confidence),
            timestamp: Utc::now(),
        }
    }
}

/// Violation action recommendations
///
/// Suggests what action to take when a risk rule is violated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationAction {
    /// Allow the trade (warning only)
    Allow,
    /// Block the trade temporarily
    Block,
    /// Halt all trading
    Halt,
    /// Reduce position size
    ReduceSize,
    /// Increase margin/equity requirement
    RequireMargin,
    /// Manual review required
    ManualReview,
}

impl ViolationAction {
    /// Returns true if this action blocks the trade
    pub fn blocks_trade(&self) -> bool {
        matches!(self, Self::Block | Self::Halt)
    }

    /// Returns true if this action halts trading
    pub fn halts_trading(&self) -> bool {
        matches!(self, Self::Halt)
    }
}

/// Enhanced risk evaluation with action recommendations
///
/// Extends the basic RiskEvaluation with suggested actions and metadata.
#[derive(Debug, Clone)]
pub struct RuleEvaluation {
    /// Basic risk evaluation result
    pub evaluation: RiskEvaluation,
    /// Recommended action if violated
    pub suggested_action: Option<ViolationAction>,
    /// Additional context or explanation
    pub message: Option<String>,
    /// Rule-specific metadata
    pub metadata: HashMap<String, String>,
}

impl RuleEvaluation {
    /// Creates a passed evaluation
    pub fn passed() -> Self {
        Self {
            evaluation: RiskEvaluation::Passed,
            suggested_action: None,
            message: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates a violated evaluation
    pub fn violated(severity: ViolationSeverity, action: ViolationAction, message: impl Into<String>) -> Self {
        let message_str = message.into();
        Self {
            evaluation: RiskEvaluation::Violated {
                severity,
                suggested_action: Some(message_str.clone()),
            },
            suggested_action: Some(action),
            message: Some(message_str),
            metadata: HashMap::new(),
        }
    }

    /// Adds metadata to the evaluation
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns true if the evaluation passed
    pub fn is_passed(&self) -> bool {
        self.evaluation.is_passed()
    }

    /// Returns true if the evaluation resulted in a violation
    pub fn is_violated(&self) -> bool {
        self.evaluation.is_violated()
    }

    /// Gets the violation severity if violated
    pub fn severity(&self) -> Option<&ViolationSeverity> {
        self.evaluation.severity()
    }
}

/// Trait for composable risk rules
///
/// Defines the interface that all risk rules must implement.
/// Rules can be combined and evaluated together.
#[async_trait::async_trait]
pub trait RiskRule: Send + Sync {
    /// Returns the unique identifier for this rule
    fn id(&self) -> RiskRuleId;

    /// Evaluates the rule against the given trade context
    ///
    /// # Arguments
    ///
    /// * `context` - The trade context to evaluate
    ///
    /// # Returns
    ///
    /// Rule evaluation result with action recommendations
    async fn evaluate(&self, context: &TradeContext) -> RuleEvaluation;

    /// Returns the name of this rule for display purposes
    fn name(&self) -> &'static str;

    /// Returns a description of what this rule checks
    fn description(&self) -> &'static str;
}

/// Daily loss limit rule implementation
pub struct DailyLossLimitRule {
    tracker: Arc<DailyLossTracker>,
}

impl DailyLossLimitRule {
    /// Creates a new daily loss limit rule
    pub fn new(tracker: Arc<DailyLossTracker>) -> Self {
        Self { tracker }
    }
}

#[async_trait::async_trait]
impl RiskRule for DailyLossLimitRule {
    fn id(&self) -> RiskRuleId {
        RiskRuleId::DailyLossLimit
    }

    async fn evaluate(&self, context: &TradeContext) -> RuleEvaluation {
        // Check if the potential trade would violate daily loss limits
        match self.tracker.check_daily_loss_limit(context.order_value).await {
            Ok(true) => RuleEvaluation::passed(),
            Ok(false) => RuleEvaluation::violated(
                ViolationSeverity::Critical,
                ViolationAction::Halt,
                "Trade would exceed daily loss limits",
            ),
            Err(msg) => RuleEvaluation::violated(
                ViolationSeverity::Critical,
                ViolationAction::Halt,
                msg,
            ),
        }
    }

    fn name(&self) -> &'static str {
        "Daily Loss Limit"
    }

    fn description(&self) -> &'static str {
        "Prevents losses exceeding configured daily limits"
    }
}

/// Position size limit rule implementation
pub struct PositionSizeLimitRule {
    sizer: Arc<PositionSizer>,
}

impl PositionSizeLimitRule {
    /// Creates a new position size limit rule
    pub fn new(sizer: Arc<PositionSizer>) -> Self {
        Self { sizer }
    }
}

#[async_trait::async_trait]
impl RiskRule for PositionSizeLimitRule {
    fn id(&self) -> RiskRuleId {
        RiskRuleId::PositionSizeLimit
    }

    async fn evaluate(&self, context: &TradeContext) -> RuleEvaluation {
        match self.sizer.validate_position_size(context.order_value).await {
            RiskEvaluation::Passed => RuleEvaluation::passed(),
            RiskEvaluation::Violated { severity, suggested_action } => {
                let action = match severity {
                    ViolationSeverity::Critical => ViolationAction::Block,
                    ViolationSeverity::Moderate => ViolationAction::ReduceSize,
                    ViolationSeverity::Warning => ViolationAction::Allow,
                };

                RuleEvaluation::violated(
                    severity,
                    action,
                    suggested_action.unwrap_or_else(|| "Position size violation".to_string()),
                )
            }
        }
    }

    fn name(&self) -> &'static str {
        "Position Size Limit"
    }

    fn description(&self) -> &'static str {
        "Enforces maximum position sizes and portfolio allocation limits"
    }
}

/// PDT rule implementation
pub struct PdtLimitRule {
    enforcer: Arc<PdtEnforcer>,
}

impl PdtLimitRule {
    /// Creates a new PDT limit rule
    pub fn new(enforcer: Arc<PdtEnforcer>) -> Self {
        Self { enforcer }
    }
}

#[async_trait::async_trait]
impl RiskRule for PdtLimitRule {
    fn id(&self) -> RiskRuleId {
        RiskRuleId::PdtLimit
    }

    async fn evaluate(&self, context: &TradeContext) -> RuleEvaluation {
        // Only check PDT rules for day trades
        if !context.is_day_trade {
            return RuleEvaluation::passed();
        }

        match self.enforcer.enforce_pdt_limits().await {
            RiskEvaluation::Passed => RuleEvaluation::passed(),
            RiskEvaluation::Violated { severity, suggested_action } => {
                let action = match severity {
                    ViolationSeverity::Critical => ViolationAction::Halt,
                    ViolationSeverity::Moderate => ViolationAction::Block,
                    ViolationSeverity::Warning => ViolationAction::ManualReview,
                };

                RuleEvaluation::violated(
                    severity,
                    action,
                    suggested_action.unwrap_or_else(|| "PDT rule violation".to_string()),
                )
            }
        }
    }

    fn name(&self) -> &'static str {
        "PDT Limit"
    }

    fn description(&self) -> &'static str {
        "Enforces Pattern Day Trading rules and equity requirements"
    }
}

/// Pattern-specific limit rule implementation
pub struct PatternSpecificLimitRule {
    evaluator: Arc<PatternRiskEvaluator>,
}

impl PatternSpecificLimitRule {
    /// Creates a new pattern-specific limit rule
    pub fn new(evaluator: Arc<PatternRiskEvaluator>) -> Self {
        Self { evaluator }
    }
}

#[async_trait::async_trait]
impl RiskRule for PatternSpecificLimitRule {
    fn id(&self) -> RiskRuleId {
        RiskRuleId::PatternSpecificLimit
    }

    async fn evaluate(&self, context: &TradeContext) -> RuleEvaluation {
        // Only evaluate if this is a pattern-based trade
        let (pattern_name, confidence) = match (&context.pattern_name, context.pattern_confidence) {
            (Some(name), Some(conf)) => (name, conf),
            _ => return RuleEvaluation::passed(), // Not a pattern trade, skip evaluation
        };

        match self.evaluator.evaluate_pattern_trade(pattern_name, confidence, context.order_value) {
            RiskEvaluation::Passed => RuleEvaluation::passed(),
            RiskEvaluation::Violated { severity, suggested_action } => {
                let action = match severity {
                    ViolationSeverity::Critical => ViolationAction::Block,
                    ViolationSeverity::Moderate => ViolationAction::ReduceSize,
                    ViolationSeverity::Warning => ViolationAction::Allow,
                };

                RuleEvaluation::violated(
                    severity,
                    action,
                    suggested_action.unwrap_or_else(|| "Pattern-specific limit violation".to_string()),
                )
            }
        }
    }

    fn name(&self) -> &'static str {
        "Pattern-Specific Limit"
    }

    fn description(&self) -> &'static str {
        "Enforces pattern-specific risk limits based on pattern type and confidence"
    }
}

/// Integrated risk rule engine
///
/// Manages all risk rules and provides unified evaluation interface.
/// Rules are evaluated in parallel for performance.
pub struct RiskRuleEngine {
    rules: Vec<Box<dyn RiskRule>>,
    config: RiskConfig,
}

impl RiskRuleEngine {
    /// Creates a new risk rule engine with all configured rules
    pub fn new(config: RiskConfig) -> Self {
        let mut rules: Vec<Box<dyn RiskRule>> = Vec::new();

        // Initialize daily loss limits
        let daily_limits = DailyLossLimits::from_config(&config, Decimal::ZERO);
        let daily_tracker = Arc::new(DailyLossTracker::new(daily_limits));
        rules.push(Box::new(DailyLossLimitRule::new(Arc::clone(&daily_tracker))));

        // Initialize position sizing
        let position_limits = PositionSizeLimits::from_config(&config);
        let position_sizer = Arc::new(PositionSizer::new(position_limits, Decimal::ZERO));
        rules.push(Box::new(PositionSizeLimitRule::new(Arc::clone(&position_sizer))));

        // Initialize PDT enforcement
        let pdt_config = PdtConfig::from_config(&config);
        let pdt_enforcer = Arc::new(PdtEnforcer::new(pdt_config, Decimal::ZERO));
        rules.push(Box::new(PdtLimitRule::new(Arc::clone(&pdt_enforcer))));

        // Initialize pattern-specific limits
        let pattern_config = PatternRiskConfig::from_config(&config);
        let pattern_evaluator = Arc::new(PatternRiskEvaluator::new(pattern_config));
        rules.push(Box::new(PatternSpecificLimitRule::new(Arc::clone(&pattern_evaluator))));

        Self { rules, config }
    }

    /// Evaluates all risk rules for a trade context
    ///
    /// # Arguments
    ///
    /// * `context` - The trade context to evaluate
    ///
    /// # Returns
    ///
    /// Map of rule IDs to their evaluation results
    ///
    /// # Examples
    ///
    /// ```
    /// # use daytrader::risk::rules::{RiskRuleEngine, TradeContext};
    /// # use daytrader::config::RiskConfig;
    /// # use rust_decimal::Decimal;
    /// # tokio_test::block_on(async {
    /// let config = RiskConfig::default();
    /// let engine = RiskRuleEngine::new(config);
    ///
    /// let context = TradeContext::new(
    ///     "AAPL".to_string(),
    ///     Decimal::new(100, 0),
    ///     Decimal::new(15000, 2),
    ///     Decimal::new(5000000, 2),
    ///     false,
    /// );
    ///
    /// let results = engine.evaluate_all_rules(context).await;
    /// assert!(!results.is_empty());
    /// # });
    /// ```
    pub async fn evaluate_all_rules(&self, context: TradeContext) -> HashMap<RiskRuleId, RuleEvaluation> {
        let mut results = HashMap::new();

        // Evaluate all rules in parallel
        let futures = self.rules.iter().map(|rule| {
            let rule_id = rule.id();
            let context_ref = &context;
            async move {
                let evaluation = rule.evaluate(context_ref).await;
                (rule_id, evaluation)
            }
        });

        let evaluations = futures_util::future::join_all(futures).await;

        for (rule_id, evaluation) in evaluations {
            results.insert(rule_id, evaluation);
        }

        results
    }

    /// Evaluates rules and returns only violations
    ///
    /// # Arguments
    ///
    /// * `context` - The trade context to evaluate
    ///
    /// # Returns
    ///
    /// Map of rule IDs to violations (empty if all rules pass)
    pub async fn evaluate_violations_only(&self, context: TradeContext) -> HashMap<RiskRuleId, RuleEvaluation> {
        let all_results = self.evaluate_all_rules(context).await;

        all_results
            .into_iter()
            .filter(|(_, evaluation)| evaluation.is_violated())
            .collect()
    }

    /// Checks if any critical violations exist
    ///
    /// # Arguments
    ///
    /// * `context` - The trade context to evaluate
    ///
    /// # Returns
    ///
    /// `true` if any critical violations are found
    pub async fn has_critical_violations(&self, context: TradeContext) -> bool {
        let violations = self.evaluate_violations_only(context).await;

        violations.values().any(|evaluation| {
            matches!(evaluation.severity(), Some(ViolationSeverity::Critical))
        })
    }

    /// Gets all configured rules
    pub fn rules(&self) -> &[Box<dyn RiskRule>] {
        &self.rules
    }

    /// Gets the risk configuration
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn create_test_context() -> TradeContext {
        TradeContext::new(
            "AAPL".to_string(),
            Decimal::new(100, 0),     // 100 shares
            Decimal::new(15000, 2),   // $150.00
            Decimal::new(5000000, 2), // $50,000 equity
            false,                    // not a day trade
        )
    }

    #[tokio::test]
    async fn test_rule_evaluation_structure() {
        let config = RiskConfig::default();
        let engine = RiskRuleEngine::new(config);
        let context = create_test_context();

        let results = engine.evaluate_all_rules(context).await;

        // Should have evaluations for all rule types
        assert!(results.contains_key(&RiskRuleId::DailyLossLimit));
        assert!(results.contains_key(&RiskRuleId::PositionSizeLimit));
        assert!(results.contains_key(&RiskRuleId::PdtLimit));
        assert!(results.contains_key(&RiskRuleId::PatternSpecificLimit));

        // All should pass for a basic trade
        for (rule_id, evaluation) in results {
            assert!(
                evaluation.is_passed(),
                "Rule {:?} should pass for basic trade",
                rule_id
            );
        }
    }

    #[tokio::test]
    async fn test_violation_evaluation() {
        let passed = RuleEvaluation::passed();
        assert!(passed.is_passed());
        assert!(!passed.is_violated());

        let violated = RuleEvaluation::violated(
            ViolationSeverity::Critical,
            ViolationAction::Halt,
            "Test violation",
        );
        assert!(!violated.is_passed());
        assert!(violated.is_violated());
        assert_eq!(violated.severity(), Some(&ViolationSeverity::Critical));
    }

    #[tokio::test]
    async fn test_violation_actions() {
        assert!(ViolationAction::Halt.blocks_trade());
        assert!(ViolationAction::Halt.halts_trading());
        assert!(ViolationAction::Block.blocks_trade());
        assert!(!ViolationAction::Block.halts_trading());
        assert!(!ViolationAction::Allow.blocks_trade());
        assert!(!ViolationAction::Allow.halts_trading());
    }

    #[tokio::test]
    async fn test_critical_violation_detection() {
        let config = RiskConfig::default();
        let engine = RiskRuleEngine::new(config);

        // Create a context that should pass
        let context = create_test_context();
        let has_critical = engine.has_critical_violations(context).await;
        assert!(!has_critical);
    }

    #[test]
    fn test_rule_metadata() {
        let evaluation = RuleEvaluation::passed()
            .with_metadata("test_key", "test_value");

        assert_eq!(evaluation.metadata.get("test_key"), Some(&"test_value".to_string()));
    }
}