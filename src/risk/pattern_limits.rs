//! Pattern-Specific Risk Limits and Evaluation
//!
//! This module implements pattern-aware risk management that applies different
//! risk rules based on candlestick pattern types and their confidence scores.
//! It builds on the core risk framework to provide fine-grained control over
//! pattern-based trading strategies.
//!
//! # Features
//!
//! - **Pattern Risk Profiles**: Configurable risk settings per pattern type
//! - **Confidence-Based Adjustments**: Dynamic risk limits based on pattern confidence
//! - **Trade Frequency Limits**: Pattern-specific trade count restrictions
//! - **Position Size Controls**: Pattern-aware position sizing limits
//! - **Performance Tracking**: Real-time pattern performance monitoring
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::pattern_limits::{PatternRiskEvaluator, PatternRiskConfig};
//! use daytrader::brokers::types::PatternName;
//! use rust_decimal::Decimal;
//!
//! // Create pattern risk evaluator with default profiles
//! let config = PatternRiskConfig::default();
//! let evaluator = PatternRiskEvaluator::new(config);
//!
//! // Evaluate a hammer pattern trade
//! let pattern_name = PatternName::new("hammer");
//! let confidence = 0.85;
//! let order_value = Decimal::new(5000, 2); // $50.00
//!
//! let evaluation = evaluator.evaluate_pattern_trade(
//!     &pattern_name,
//!     confidence,
//!     order_value
//! );
//!
//! assert!(evaluation.is_passed());
//! ```

use crate::brokers::types::{Message, PatternName};
use crate::events::bus::EventBus;
use crate::events::types::{MarketEvent, RiskViolation, RiskViolationType};
use crate::risk::types::{
    ConfidenceMultiplier, Count, PatternPerformanceThresholds, PatternRiskProfile,
    PatternRiskViolation, PatternRiskViolationType, RiskEvaluation, ViolationSeverity,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for pattern-specific risk management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRiskConfig {
    /// Risk profiles for each pattern type
    pub pattern_profiles: HashMap<String, PatternRiskProfile>,
    /// Confidence-based multipliers for risk adjustment
    pub confidence_multipliers: Vec<ConfidenceMultiplier>,
    /// Performance thresholds for pattern enable/disable
    pub performance_thresholds: PatternPerformanceThresholds,
    /// Default profile for patterns without specific configuration
    pub default_profile: PatternRiskProfile,
    /// Base position size limit (from general risk config)
    pub base_max_position_dollars: f64,
    /// Base trade limit per day (from general risk config)
    pub base_max_trades_per_day: u32,
}

impl PatternRiskConfig {
    /// Creates a PatternRiskConfig from the application RiskConfig
    pub fn from_config(config: &crate::config::RiskConfig) -> Self {
        Self {
            pattern_profiles: config.pattern_profiles.clone(),
            confidence_multipliers: config.confidence_multipliers.clone(),
            performance_thresholds: config.pattern_performance_thresholds.clone(),
            default_profile: PatternRiskProfile::moderate(),
            base_max_position_dollars: config.max_position_size_dollars,
            base_max_trades_per_day: config.max_trades_per_pattern_per_day,
        }
    }
}

impl Default for PatternRiskConfig {
    fn default() -> Self {
        let mut pattern_profiles = HashMap::new();

        // Hammer pattern - conservative
        pattern_profiles.insert(
            "hammer".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(3).unwrap(),
                max_position_size_multiplier: 0.7,
                min_confidence_threshold: 0.75,
                risk_multiplier: 0.6,
                stop_after_consecutive_losses: Count::new(3).unwrap(),
            },
        );

        // Doji pattern - very conservative
        pattern_profiles.insert(
            "doji".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(2).unwrap(),
                max_position_size_multiplier: 0.5,
                min_confidence_threshold: 0.80,
                risk_multiplier: 0.4,
                stop_after_consecutive_losses: Count::new(2).unwrap(),
            },
        );

        // Engulfing pattern - moderate
        pattern_profiles.insert(
            "bullish_engulfing".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(4).unwrap(),
                max_position_size_multiplier: 0.85,
                min_confidence_threshold: 0.70,
                risk_multiplier: 0.8,
                stop_after_consecutive_losses: Count::new(3).unwrap(),
            },
        );

        pattern_profiles.insert(
            "bearish_engulfing".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(4).unwrap(),
                max_position_size_multiplier: 0.85,
                min_confidence_threshold: 0.70,
                risk_multiplier: 0.8,
                stop_after_consecutive_losses: Count::new(3).unwrap(),
            },
        );

        // Morning/Evening Star - higher tolerance
        pattern_profiles.insert(
            "morning_star".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(5).unwrap(),
                max_position_size_multiplier: 0.95,
                min_confidence_threshold: 0.65,
                risk_multiplier: 0.9,
                stop_after_consecutive_losses: Count::new(3).unwrap(),
            },
        );

        pattern_profiles.insert(
            "evening_star".to_string(),
            PatternRiskProfile {
                max_trades_per_day: Count::new(5).unwrap(),
                max_position_size_multiplier: 0.95,
                min_confidence_threshold: 0.65,
                risk_multiplier: 0.9,
                stop_after_consecutive_losses: Count::new(3).unwrap(),
            },
        );

        Self {
            pattern_profiles,
            confidence_multipliers: vec![
                ConfidenceMultiplier {
                    min_confidence: 0.0,
                    max_confidence: 0.3,
                    position_size_multiplier: 0.3,
                    trade_limit_multiplier: 0.5,
                    risk_limit_multiplier: 0.3,
                },
                ConfidenceMultiplier {
                    min_confidence: 0.3,
                    max_confidence: 0.5,
                    position_size_multiplier: 0.5,
                    trade_limit_multiplier: 0.7,
                    risk_limit_multiplier: 0.5,
                },
                ConfidenceMultiplier {
                    min_confidence: 0.5,
                    max_confidence: 0.7,
                    position_size_multiplier: 0.8,
                    trade_limit_multiplier: 0.9,
                    risk_limit_multiplier: 0.8,
                },
                ConfidenceMultiplier {
                    min_confidence: 0.7,
                    max_confidence: 0.9,
                    position_size_multiplier: 1.0,
                    trade_limit_multiplier: 1.0,
                    risk_limit_multiplier: 1.0,
                },
                ConfidenceMultiplier {
                    min_confidence: 0.9,
                    max_confidence: 1.0,
                    position_size_multiplier: 1.2,
                    trade_limit_multiplier: 1.1,
                    risk_limit_multiplier: 1.1,
                },
            ],
            performance_thresholds: PatternPerformanceThresholds::default(),
            default_profile: PatternRiskProfile::moderate(),
            base_max_position_dollars: 5000.0, // Default base limit
            base_max_trades_per_day: 5, // Default base limit
        }
    }
}

/// Pattern risk state for tracking daily limits
#[derive(Debug, Clone)]
pub struct PatternRiskState {
    /// Pattern name
    pub pattern_name: String,
    /// Trades executed today for this pattern
    pub daily_trade_count: u32,
    /// Consecutive losses for this pattern
    pub consecutive_losses: u32,
    /// Whether the pattern is currently enabled
    pub is_enabled: bool,
    /// Current risk multiplier applied
    pub current_risk_multiplier: f64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl PatternRiskState {
    /// Creates a new pattern risk state
    pub fn new(pattern_name: String) -> Self {
        Self {
            pattern_name,
            daily_trade_count: 0,
            consecutive_losses: 0,
            is_enabled: true,
            current_risk_multiplier: 1.0,
            last_updated: Utc::now(),
        }
    }

    /// Resets daily trade count (call at market open)
    pub fn reset_daily_count(&mut self) {
        self.daily_trade_count = 0;
        self.last_updated = Utc::now();
    }

    /// Records a trade and updates state
    pub fn record_trade(&mut self, was_profitable: bool) {
        self.daily_trade_count += 1;
        if was_profitable {
            self.consecutive_losses = 0;
        } else {
            self.consecutive_losses += 1;
        }
        self.last_updated = Utc::now();
    }

    /// Updates the risk multiplier
    pub fn update_risk_multiplier(&mut self, multiplier: f64) {
        self.current_risk_multiplier = multiplier;
        self.last_updated = Utc::now();
    }

    /// Enables or disables the pattern
    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
        self.last_updated = Utc::now();
    }
}

/// Core pattern risk evaluator
///
/// Evaluates pattern-based trades against risk limits and confidence thresholds.
/// Maintains pattern-specific state and applies dynamic risk adjustments.
#[derive(Debug)]
pub struct PatternRiskEvaluator {
    config: PatternRiskConfig,
    pattern_states: HashMap<String, PatternRiskState>,
    stats_tracker: Option<crate::risk::pattern_stats::PatternStatsTracker>,
    event_bus: Option<Arc<EventBus>>,
    // Cache for frequently accessed pattern keys to avoid repeated string operations
    pattern_key_cache: Arc<DashMap<String, String>>,
}

impl PatternRiskEvaluator {
    /// Creates a new pattern risk evaluator
    pub fn new(config: PatternRiskConfig) -> Self {
        Self {
            config,
            pattern_states: HashMap::new(),
            stats_tracker: None,
            event_bus: None,
            pattern_key_cache: Arc::new(DashMap::new()),
        }
    }

    /// Creates a new pattern risk evaluator with statistics tracking
    pub fn with_stats_tracker(config: PatternRiskConfig, stats_tracker: crate::risk::pattern_stats::PatternStatsTracker) -> Self {
        Self {
            config,
            pattern_states: HashMap::new(),
            stats_tracker: Some(stats_tracker),
            event_bus: None,
            pattern_key_cache: Arc::new(DashMap::new()),
        }
    }

    /// Sets the statistics tracker for performance-based adjustments
    pub fn set_stats_tracker(&mut self, stats_tracker: crate::risk::pattern_stats::PatternStatsTracker) {
        self.stats_tracker = Some(stats_tracker);
    }
    
    pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Evaluates a pattern-based trade against risk limits
    ///
    /// # Arguments
    ///
    /// * `pattern_name` - Name of the candlestick pattern
    /// * `confidence` - Pattern recognition confidence score (0.0 to 1.0)
    /// * `order_value` - Dollar value of the proposed trade
    ///
    /// # Returns
    ///
    /// RiskEvaluation indicating whether the trade passes risk checks
    pub fn evaluate_pattern_trade(
        &self,
        pattern_name: &PatternName,
        confidence: f64,
        order_value: Decimal,
    ) -> RiskEvaluation {
        let pattern_key = self.pattern_key_cache
            .entry(pattern_name.as_str().to_string())
            .or_insert_with(|| pattern_name.as_str().to_lowercase())
            .value()
            .clone();

        // Get pattern profile and state in single lookup to avoid multiple HashMap accesses
        let profile = self.config.pattern_profiles
            .get(&pattern_key)
            .unwrap_or(&self.config.default_profile);

        let pattern_state = self.pattern_states.get(&pattern_key);

        // Check if pattern is enabled
        if let Some(state) = pattern_state {
            if !state.is_enabled {
                let violation = RiskViolation {
                    violation_type: RiskViolationType::PatternDisabled,
                    message: Message::new("Pattern is disabled due to poor performance".to_string()),
                    timestamp: Utc::now(),
                    pattern_name: Some(pattern_name.clone()),
                };
                if let Some(bus) = &self.event_bus {
                    bus.publish(MarketEvent::RiskViolation { violation });
                }
                return RiskEvaluation::Violated {
                    severity: ViolationSeverity::Critical,
                    suggested_action: Some("Pattern is disabled due to poor performance".to_string()),
                };
            }
        }

        // Check confidence threshold
        if confidence < profile.min_confidence_threshold {
            let message_str = format!(
                "Confidence {:.2} below minimum threshold {:.2} for {}",
                confidence, profile.min_confidence_threshold, pattern_name.as_str()
            );
            let violation = RiskViolation {
                violation_type: RiskViolationType::Other,
                message: Message::new(message_str.clone()),
                timestamp: Utc::now(),
                pattern_name: Some(pattern_name.clone()),
            };
            if let Some(bus) = &self.event_bus {
                bus.publish(MarketEvent::RiskViolation { violation });
            }
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Moderate,
                suggested_action: Some(message_str),
            };
        }

        // Check daily trade limit and consecutive losses using the already fetched state
        if let Some(state) = pattern_state {
            if state.daily_trade_count >= profile.max_trades_per_day.value() {
                let message_str = format!(
                    "Daily trade limit {} exceeded for pattern {}",
                    profile.max_trades_per_day.value(), pattern_name.as_str()
                );
                let violation = RiskViolation {
                    violation_type: RiskViolationType::PatternMultiplierExceeded,
                    message: Message::new(message_str.clone()),
                    timestamp: Utc::now(),
                    pattern_name: Some(pattern_name.clone()),
                };
                if let Some(bus) = &self.event_bus {
                    bus.publish(MarketEvent::RiskViolation { violation });
                }
                return RiskEvaluation::Violated {
                    severity: ViolationSeverity::Moderate,
                    suggested_action: Some(message_str),
                };
            }

            if state.consecutive_losses >= profile.stop_after_consecutive_losses.value() {
                let message_str = format!(
                    "Consecutive losses ({}) exceeded threshold for pattern {}",
                    state.consecutive_losses, pattern_name.as_str()
                );
                let violation = RiskViolation {
                    violation_type: RiskViolationType::PatternPerformanceThreshold,
                    message: Message::new(message_str.clone()),
                    timestamp: Utc::now(),
                    pattern_name: Some(pattern_name.clone()),
                };
                if let Some(bus) = &self.event_bus {
                    bus.publish(MarketEvent::RiskViolation { violation });
                }
                return RiskEvaluation::Violated {
                    severity: ViolationSeverity::Critical,
                    suggested_action: Some(message_str),
                };
            }
        }

        // Apply confidence-based adjustments
        let confidence_multiplier = ConfidenceMultiplier::for_confidence(
            confidence,
            &self.config.confidence_multipliers
        ).unwrap_or(&self.config.confidence_multipliers[0]);

        let mut position_multiplier = Decimal::from_f64_retain(confidence_multiplier.position_size_multiplier).unwrap_or(Decimal::ONE);
        let mut risk_multiplier = Decimal::from_f64_retain(confidence_multiplier.risk_limit_multiplier).unwrap_or(Decimal::ONE);

        // Apply performance-based adjustments if stats tracker is available
        if let Some(stats_tracker) = &self.stats_tracker {
            let performance_multiplier = stats_tracker.get_weighted_risk_multiplier(pattern_name);
            let performance_multiplier_decimal = Decimal::from_f64_retain(performance_multiplier).unwrap_or(Decimal::ONE);

            // Combine confidence and performance multipliers
            position_multiplier *= performance_multiplier_decimal;
            risk_multiplier *= performance_multiplier_decimal;
        }

        // Check position size against pattern-adjusted limits
        let pattern_max_position = self.config.base_max_position_dollars * profile.max_position_size_multiplier;
        let pattern_max_position_decimal = Decimal::from_f64_retain(pattern_max_position).unwrap_or(Decimal::MAX);
        if order_value > pattern_max_position_decimal {
            let message_str = format!(
                "Order value ${:.2} exceeds pattern limit ${:.2} for {}",
                order_value, pattern_max_position, pattern_name.as_str()
            );
            let violation = RiskViolation {
                violation_type: RiskViolationType::PositionSizeLimit,
                message: Message::new(message_str.clone()),
                timestamp: Utc::now(),
                pattern_name: Some(pattern_name.clone()),
            };
            if let Some(bus) = &self.event_bus {
                bus.publish(MarketEvent::RiskViolation { violation });
            }
            return RiskEvaluation::Violated {
                severity: ViolationSeverity::Moderate,
                suggested_action: Some(message_str),
            };
        }

        RiskEvaluation::Passed
    }

    /// Records a completed pattern trade and updates risk state
    ///
    /// # Arguments
    ///
    /// * `pattern_name` - Name of the pattern used for the trade
    /// * `was_profitable` - Whether the trade resulted in a profit
    pub fn record_pattern_trade(&mut self, pattern_name: &PatternName, was_profitable: bool) {
        let pattern_key = pattern_name.as_str().to_lowercase();

        let state = self.pattern_states
            .entry(pattern_key.clone())
            .or_insert_with(|| PatternRiskState::new(pattern_key));

        state.record_trade(was_profitable);
    }

    /// Updates pattern enable/disable status based on performance
    ///
    /// # Arguments
    ///
    /// * `pattern_name` - Name of the pattern to update
    /// * `win_rate` - Current win rate (0.0 to 1.0)
    /// * `sample_size` - Number of trades in the sample
    pub fn update_pattern_status(&mut self, pattern_name: &PatternName, win_rate: f64, sample_size: u32) {
        let pattern_key = pattern_name.as_str().to_lowercase();
    
        let threshold_percent = self.config.performance_thresholds.auto_disable_threshold.value() / Decimal::new(100, 0);
        let threshold_rate = threshold_percent.to_f64().unwrap_or(0.4);
        let should_enable = sample_size >= self.config.performance_thresholds.min_sample_size.value()
            && win_rate >= threshold_rate;
    
        let state = self.pattern_states
            .entry(pattern_key.clone())
            .or_insert_with(|| PatternRiskState::new(pattern_key));
    
        let old_enabled = state.is_enabled;
        state.set_enabled(should_enable);
    
        if old_enabled != should_enable && self.event_bus.is_some() {
            let bus = self.event_bus.as_ref().unwrap();
            let reason = if should_enable {
                "Met performance criteria".to_string()
            } else {
                "Failed performance threshold".to_string()
            };
            if should_enable {
                bus.publish(MarketEvent::PatternEnabled {
                    pattern_name: pattern_name.clone(),
                    reason: reason.clone(),
                });
            } else {
                bus.publish(MarketEvent::PatternDisabled {
                    pattern_name: pattern_name.clone(),
                    reason,
                });
            }
    
            // Publish performance update
            if let Some(stats_tracker) = &self.stats_tracker {
                if let Some(stats) = stats_tracker.get_pattern_stats(pattern_name) {
                    bus.publish(MarketEvent::PatternPerformanceUpdate {
                        pattern_name: pattern_name.clone(),
                        win_rate: stats.win_rate,
                        total_trades: stats.total_trades,
                        recent_pl: stats.total_profit,
                    });
                }
            }
        }
    }

    /// Gets the current risk state for a pattern
    pub fn get_pattern_state(&self, pattern_name: &PatternName) -> Option<&PatternRiskState> {
        let pattern_key = pattern_name.as_str().to_lowercase();
        self.pattern_states.get(&pattern_key)
    }

    /// Resets daily trade counts for all patterns (call at market open)
    pub fn reset_daily_counts(&mut self) {
        for state in self.pattern_states.values_mut() {
            state.reset_daily_count();
        }
    }

    /// Creates a pattern risk violation for monitoring/alerting
    pub fn create_violation(
        &self,
        violation_type: PatternRiskViolationType,
        pattern_name: PatternName,
        confidence: Option<f64>,
        message: String,
        severity: ViolationSeverity,
        suggested_action: Option<String>,
    ) -> PatternRiskViolation {
        PatternRiskViolation {
            violation_type,
            pattern_name,
            confidence,
            message,
            timestamp: Utc::now(),
            severity,
            suggested_action,
        }
    }

    /// Updates risk multipliers based on recent performance
    /// This should be called periodically to adjust risk levels
    pub fn update_performance_based_multipliers(&mut self) {
        if let Some(stats_tracker) = &self.stats_tracker {
            // Update pattern states with new risk multipliers
            for (pattern_key, state) in &mut self.pattern_states {
                let pattern_name = PatternName::new(pattern_key);
                let new_multiplier = stats_tracker.get_weighted_risk_multiplier(&pattern_name);
                state.update_risk_multiplier(new_multiplier);
            }
        }
    }

    /// Gets patterns that need risk escalation due to poor performance
    pub fn get_patterns_needing_escalation(&self, performance_threshold: f64) -> Vec<String> {
        if let Some(stats_tracker) = &self.stats_tracker {
            stats_tracker
                .get_patterns_for_risk_escalation(performance_threshold)
                .into_iter()
                .map(|(pattern, _)| pattern)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Gets patterns that need risk de-escalation due to excellent performance
    pub fn get_patterns_needing_deescalation(&self, performance_threshold: f64) -> Vec<String> {
        if let Some(stats_tracker) = &self.stats_tracker {
            stats_tracker
                .get_patterns_for_risk_deescalation(performance_threshold)
                .into_iter()
                .map(|(pattern, _)| pattern)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Updates pattern enable/disable status based on rolling performance
    pub fn update_pattern_status_from_performance(&mut self, min_performance_score: f64, min_sample_size: u32) {
        if let Some(stats_tracker) = &mut self.stats_tracker {
            let pattern_names: Vec<String> = self.pattern_states.keys().cloned().collect();
            for pattern_name in pattern_names {
                let pattern_name_obj = PatternName::new(&pattern_name);
                stats_tracker.update_pattern_status_from_rolling(&pattern_name_obj, min_performance_score, min_sample_size);

                // Update our local state to match
                if let Some(stats) = stats_tracker.get_pattern_stats(&pattern_name_obj) {
                    if let Some(state) = self.pattern_states.get_mut(&pattern_name) {
                        state.set_enabled(stats.is_enabled);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn create_test_evaluator() -> PatternRiskEvaluator {
        PatternRiskEvaluator::new(PatternRiskConfig::default())
    }

    #[test]
    fn test_pattern_risk_evaluation_passed() {
        let evaluator = create_test_evaluator();
        let pattern_name = PatternName::new("hammer");
        let confidence = 0.85;
        let order_value = Decimal::new(200000, 2); // $2,000 (within hammer limit of $3,500)

        let evaluation = evaluator.evaluate_pattern_trade(&pattern_name, confidence, order_value);
        assert!(evaluation.is_passed());
    }

    #[test]
    fn test_confidence_too_low_violation() {
        let evaluator = create_test_evaluator();
        let pattern_name = PatternName::new("doji");
        let confidence = 0.70; // Below 0.80 threshold for doji
        let order_value = Decimal::new(2500, 2);

        let evaluation = evaluator.evaluate_pattern_trade(&pattern_name, confidence, order_value);
        assert!(evaluation.is_violated());
        assert_eq!(evaluation.severity(), Some(&ViolationSeverity::Moderate));
    }

    #[test]
    fn test_daily_trade_limit_exceeded() {
        let mut evaluator = create_test_evaluator();
        let pattern_name = PatternName::new("hammer");

        // Record 3 trades (limit for hammer)
        for _ in 0..3 {
            evaluator.record_pattern_trade(&pattern_name, true);
        }

        let confidence = 0.85;
        let order_value = Decimal::new(3500, 2);
        let evaluation = evaluator.evaluate_pattern_trade(&pattern_name, confidence, order_value);

        assert!(evaluation.is_violated());
        assert_eq!(evaluation.severity(), Some(&ViolationSeverity::Moderate));
    }

    #[test]
    fn test_consecutive_losses_violation() {
        let mut evaluator = create_test_evaluator();
        let pattern_name = PatternName::new("morning_star"); // Has higher trade limits

        // Record 3 consecutive losses (limit for morning_star is also 3)
        for _ in 0..3 {
            evaluator.record_pattern_trade(&pattern_name, false);
        }

        let confidence = 0.85;
        let order_value = Decimal::new(300000, 2); // Within position size limit (5000 * 0.95 = 4750)
        let evaluation = evaluator.evaluate_pattern_trade(&pattern_name, confidence, order_value);

        assert!(evaluation.is_violated());
        assert_eq!(evaluation.severity(), Some(&ViolationSeverity::Critical));
    }

    #[test]
    fn test_pattern_disable_enable() {
        let mut evaluator = create_test_evaluator();
        let pattern_name = PatternName::new("hammer");

        // Initially enabled (state gets created on first access)
        evaluator.update_pattern_status(&pattern_name, 0.65, 25); // Good performance first
        assert!(evaluator.get_pattern_state(&pattern_name).unwrap().is_enabled);

        // Disable due to poor performance
        evaluator.update_pattern_status(&pattern_name, 0.35, 25); // 35% win rate, below 40% threshold
        assert!(!evaluator.get_pattern_state(&pattern_name).unwrap().is_enabled);

        // Re-enable with good performance
        evaluator.update_pattern_status(&pattern_name, 0.65, 25); // 65% win rate
        assert!(evaluator.get_pattern_state(&pattern_name).unwrap().is_enabled);
    }

    #[test]
    fn test_confidence_multiplier_application() {
        let evaluator = create_test_evaluator();

        // Test high confidence (should get 1.2x multiplier)
        let high_conf_multiplier = ConfidenceMultiplier::for_confidence(0.95, &evaluator.config.confidence_multipliers);
        assert_eq!(high_conf_multiplier.unwrap().position_size_multiplier, 1.2);

        // Test low confidence (should get 0.3x multiplier)
        let low_conf_multiplier = ConfidenceMultiplier::for_confidence(0.15, &evaluator.config.confidence_multipliers);
        assert_eq!(low_conf_multiplier.unwrap().position_size_multiplier, 0.3);
    }

    #[test]
    fn test_default_profile_fallback() {
        let evaluator = create_test_evaluator();
        let unknown_pattern = PatternName::new("unknown_pattern");
        let confidence = 0.75;
        let order_value = Decimal::new(200000, 2); // Within default profile limit

        // Should use default profile and pass
        let evaluation = evaluator.evaluate_pattern_trade(&unknown_pattern, confidence, order_value);
        assert!(evaluation.is_passed());
    }

    #[test]
    fn test_performance_based_risk_adjustment() {
        let mut tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add excellent performance history
        for _ in 0..15 {
            tracker.record_trade_outcome(&pattern_name, 0.9, Decimal::new(1000, 2));
        }

        let mut evaluator = PatternRiskEvaluator::with_stats_tracker(PatternRiskConfig::default(), tracker);
        let confidence = 0.85;
        let order_value = Decimal::new(500000, 2); // $5,000 order

        let evaluation = evaluator.evaluate_pattern_trade(&pattern_name, confidence, order_value);
        assert!(evaluation.is_passed()); // Should pass due to reduced risk from excellent performance
    }

    #[test]
    fn test_performance_based_escalation_detection() {
        let mut tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add poor performance history
        for _ in 0..12 {
            tracker.record_trade_outcome(&pattern_name, 0.6, Decimal::new(-1000, 2));
        }
        for _ in 0..3 {
            tracker.record_trade_outcome(&pattern_name, 0.6, Decimal::new(1000, 2));
        }

        let evaluator = PatternRiskEvaluator::with_stats_tracker(PatternRiskConfig::default(), tracker);

        let escalation_patterns = evaluator.get_patterns_needing_escalation(0.5);
        assert!(escalation_patterns.contains(&"hammer".to_string()));
    }

    #[test]
    fn test_performance_based_deescalation_detection() {
        let mut tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add excellent performance history
        for _ in 0..18 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }

        let evaluator = PatternRiskEvaluator::with_stats_tracker(PatternRiskConfig::default(), tracker);

        let deescalation_patterns = evaluator.get_patterns_needing_deescalation(0.7);
        assert!(deescalation_patterns.contains(&"hammer".to_string()));
    }

    #[test]
    fn test_risk_multiplier_updates() {
        let mut tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Start with good performance
        for _ in 0..10 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }

        let mut evaluator = PatternRiskEvaluator::with_stats_tracker(PatternRiskConfig::default(), tracker);

        // Initially should have lower risk multiplier due to good performance
        evaluator.update_performance_based_multipliers();
        let initial_multiplier = evaluator.get_pattern_state(&pattern_name).unwrap().current_risk_multiplier;
        assert!(initial_multiplier < 1.0); // Reduced risk for good performance

        // Simulate poor performance by directly modifying tracker
        let mut new_tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        for _ in 0..10 {
            new_tracker.record_trade_outcome(&pattern_name, 0.5, Decimal::new(-1000, 2));
        }
        evaluator.set_stats_tracker(new_tracker);

        evaluator.update_performance_based_multipliers();
        let final_multiplier = evaluator.get_pattern_state(&pattern_name).unwrap().current_risk_multiplier;
        assert!(final_multiplier > initial_multiplier); // Increased risk
    }

    #[test]
    fn test_pattern_status_updates_from_performance() {
        let mut tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Start with poor performance
        for _ in 0..20 {
            tracker.record_trade_outcome(&pattern_name, 0.5, Decimal::new(-1000, 2));
        }

        let mut evaluator = PatternRiskEvaluator::with_stats_tracker(PatternRiskConfig::default(), tracker);

        // Should disable pattern due to poor performance
        evaluator.update_pattern_status_from_performance(0.5, 10);
        let state = evaluator.get_pattern_state(&pattern_name).unwrap();
        assert!(!state.is_enabled);

        // Now add good performance
        let mut new_tracker = crate::risk::pattern_stats::PatternStatsTracker::new();
        for _ in 0..20 {
            new_tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }
        evaluator.set_stats_tracker(new_tracker);

        // Should re-enable pattern due to improved performance
        evaluator.update_pattern_status_from_performance(0.5, 10);
        let state_after = evaluator.get_pattern_state(&pattern_name).unwrap();
        assert!(state_after.is_enabled);
    }
}