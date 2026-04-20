//! Execution strategy implementations
//!
//! This module provides execution strategies that determine how signals are converted
//! into orders, including position sizing, order type selection, and entry/exit conditions.

use crate::brokers::traits::{Account, OrderSide, OrderType, TimeInForce};
use crate::brokers::types::{PatternName, Price, Quantity, Symbol};
use crate::config::RiskConfig;
use crate::execution::{ExecutionError, rules::*};
use crate::patterns::signals::TradingSignal;
use crate::positions::tracker::Position;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Execution Strategy Trait
// ============================================================================

/// Execution strategy trait defining how signals are converted to orders
pub trait ExecutionStrategy: Send + Sync {
    /// Determines if the signal should be executed
    fn should_execute(
        &self,
        signal: &TradingSignal,
        context: &ExecutionContext,
    ) -> Result<(), ExecutionError>;

    /// Calculates position size for the trade
    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_config: &RiskConfig,
    ) -> Result<Quantity, ExecutionError>;

    /// Determines the order type to use
    fn determine_order_type(&self, signal: &TradingSignal, current_price: Price) -> OrderType;

    /// Determines time in force for the order
    fn determine_time_in_force(&self, signal: &TradingSignal) -> TimeInForce;

    /// Calculates limit price if using limit orders
    fn calculate_limit_price(&self, signal: &TradingSignal, current_price: Price) -> Option<Price>;

    /// Returns strategy name for logging
    fn name(&self) -> &str;
}

/// Execution context for strategy evaluation
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    /// Account information
    pub account: Account,

    /// Current open positions
    pub current_positions: Vec<Position>,

    /// Daily risk metrics
    pub daily_metrics: crate::risk::types::DailyRiskMetrics,

    /// Pattern performance statistics
    pub pattern_stats: HashMap<PatternName, crate::risk::pattern_stats::PatternPerformanceStats>,

    /// Current timestamp
    pub current_time: chrono::DateTime<Utc>,
}

// ============================================================================
// Conservative Strategy
// ============================================================================

/// Conservative execution strategy
///
/// Uses market orders with fixed position sizing and strict entry/exit rules.
/// Designed for risk-averse trading with emphasis on capital preservation.
///
/// # Characteristics
///
/// - Market orders for guaranteed fills
/// - Fixed position sizing (percentage of portfolio)
/// - Strict confidence thresholds
/// - Limited position count
/// - No duplicate positions in same symbol
pub struct ConservativeStrategy {
    config: ConservativeConfig,
    rule_engine: RuleEngine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConservativeConfig {
    /// Minimum confidence threshold (0.0 to 1.0)
    pub min_confidence: Decimal,

    /// Position size as percentage of portfolio (0.0 to 1.0)
    pub position_size_percent: Decimal,

    /// Maximum concurrent positions
    pub max_positions: usize,

    /// Use market orders only
    pub use_market_orders: bool,
}

impl Default for ConservativeConfig {
    fn default() -> Self {
        Self {
            min_confidence: Decimal::new(75, 2),       // 0.75
            position_size_percent: Decimal::new(5, 2), // 0.05 = 5%
            max_positions: 5,
            use_market_orders: true,
        }
    }
}

impl ConservativeStrategy {
    /// Creates a new conservative strategy
    pub fn new(config: ConservativeConfig) -> Self {
        let rule_engine = RuleEngine::conservative();

        Self {
            config,
            rule_engine,
        }
    }
}

impl ExecutionStrategy for ConservativeStrategy {
    fn should_execute(
        &self,
        signal: &TradingSignal,
        context: &ExecutionContext,
    ) -> Result<(), ExecutionError> {
        // Build entry context from execution context
        let entry_context = EntryContext {
            account_balance: context.account.buying_power,
            current_positions: context.current_positions.clone(),
            daily_metrics: context.daily_metrics.clone(),
            pattern_stats: context
                .pattern_stats
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        PatternStats {
                            win_rate: v.win_rate,
                            total_trades: v.total_trades,
                            consecutive_losses: v.consecutive_losses,
                        },
                    )
                })
                .collect(),
            current_time: context.current_time,
        };

        // Evaluate all entry rules
        self.rule_engine.evaluate_entry(signal, &entry_context)?;

        Ok(())
    }

    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_config: &RiskConfig,
    ) -> Result<Quantity, ExecutionError> {
        // Fixed percentage of portfolio
        let target_value = account.portfolio_value * self.config.position_size_percent;

        // Apply risk-based adjustment (slightly reduce for lower confidence)
        let confidence_multiplier = if signal.confidence.value() >= 0.85 {
            Decimal::ONE
        } else if signal.confidence.value() >= 0.75 {
            Decimal::new(90, 2) // 0.90 = 90%
        } else {
            Decimal::new(80, 2) // 0.80 = 80%
        };

        let adjusted_value = target_value * confidence_multiplier;

        // Convert to shares based on current price
        let shares = if signal.price_level.value() > Decimal::ZERO {
            (adjusted_value / signal.price_level.value()).floor()
        } else {
            return Err(ExecutionError::PositionSizingError(
                "Invalid price level".to_string(),
            ));
        };

        // Ensure within risk limits
        let max_position_value = Decimal::from_f64_retain(risk_config.max_position_size_dollars)
            .unwrap_or(Decimal::ZERO);
        let max_shares = if signal.price_level.value() > Decimal::ZERO {
            (max_position_value / signal.price_level.value()).floor()
        } else {
            Decimal::ZERO
        };

        let final_shares = shares.min(max_shares);

        let quantity_value = final_shares
            .to_u32()
            .ok_or_else(|| ExecutionError::PositionSizingError("Invalid quantity".to_string()))?;

        Quantity::new(quantity_value)
            .map_err(|e| ExecutionError::PositionSizingError(e.to_string()))
    }

    fn determine_order_type(&self, _signal: &TradingSignal, _current_price: Price) -> OrderType {
        if self.config.use_market_orders {
            OrderType::Market
        } else {
            OrderType::Limit
        }
    }

    fn determine_time_in_force(&self, _signal: &TradingSignal) -> TimeInForce {
        TimeInForce::Day
    }

    fn calculate_limit_price(
        &self,
        _signal: &TradingSignal,
        _current_price: Price,
    ) -> Option<Price> {
        None // Conservative uses market orders
    }

    fn name(&self) -> &str {
        "Conservative"
    }
}

// ============================================================================
// Aggressive Strategy
// ============================================================================

/// Aggressive execution strategy
///
/// Uses limit orders with confidence-based position sizing for better fills.
/// Designed for active trading with larger position sizes and more trades.
///
/// # Characteristics
///
/// - Limit orders for better pricing
/// - Confidence-scaled position sizing
/// - Lower confidence threshold
/// - More concurrent positions allowed
/// - Larger position sizes for high-confidence signals
pub struct AggressiveStrategy {
    config: AggressiveConfig,
    rule_engine: RuleEngine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggressiveConfig {
    /// Minimum confidence threshold (0.0 to 1.0)
    pub min_confidence: Decimal,

    /// Base position size percentage (0.0 to 1.0)
    pub base_position_percent: Decimal,

    /// Confidence multiplier for sizing
    pub confidence_multiplier: Decimal,

    /// Maximum position size percentage
    pub max_position_percent: Decimal,

    /// Use limit orders
    pub use_limit_orders: bool,

    /// Limit order offset percentage (0.0 to 1.0)
    pub limit_offset_percent: Decimal,

    /// Maximum concurrent positions
    pub max_positions: usize,
}

impl Default for AggressiveConfig {
    fn default() -> Self {
        Self {
            min_confidence: Decimal::new(70, 2),       // 0.70
            base_position_percent: Decimal::new(3, 2), // 0.03 = 3%
            confidence_multiplier: Decimal::new(2, 0), // 2.0
            max_position_percent: Decimal::new(10, 2), // 0.10 = 10%
            use_limit_orders: true,
            limit_offset_percent: Decimal::new(2, 3), // 0.002 = 0.2%
            max_positions: 10,
        }
    }
}

impl AggressiveStrategy {
    /// Creates a new aggressive strategy
    pub fn new(config: AggressiveConfig) -> Self {
        let rule_engine = RuleEngine::aggressive();

        Self {
            config,
            rule_engine,
        }
    }
}

impl ExecutionStrategy for AggressiveStrategy {
    fn should_execute(
        &self,
        signal: &TradingSignal,
        context: &ExecutionContext,
    ) -> Result<(), ExecutionError> {
        // Build entry context
        let entry_context = EntryContext {
            account_balance: context.account.buying_power,
            current_positions: context.current_positions.clone(),
            daily_metrics: context.daily_metrics.clone(),
            pattern_stats: context
                .pattern_stats
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        PatternStats {
                            win_rate: v.win_rate,
                            total_trades: v.total_trades,
                            consecutive_losses: v.consecutive_losses,
                        },
                    )
                })
                .collect(),
            current_time: context.current_time,
        };

        // Evaluate all entry rules
        self.rule_engine.evaluate_entry(signal, &entry_context)?;

        Ok(())
    }

    fn calculate_position_size(
        &self,
        signal: &TradingSignal,
        account: &Account,
        risk_config: &RiskConfig,
    ) -> Result<Quantity, ExecutionError> {
        // Confidence-based sizing: higher confidence = larger position
        let confidence =
            Decimal::from_f64_retain(signal.confidence.value()).unwrap_or(Decimal::ZERO);
        let confidence_factor = if confidence >= self.config.min_confidence {
            (confidence - self.config.min_confidence) / (Decimal::ONE - self.config.min_confidence)
        } else {
            Decimal::ZERO
        };

        // Calculate position size percentage
        let size_percent = self.config.base_position_percent
            + (confidence_factor
                * self.config.confidence_multiplier
                * self.config.base_position_percent);

        let size_percent = size_percent.min(self.config.max_position_percent);

        // Calculate target value
        let target_value = account.portfolio_value * size_percent;

        // Convert to shares
        let shares = if signal.price_level.value() > Decimal::ZERO {
            (target_value / signal.price_level.value()).floor()
        } else {
            return Err(ExecutionError::PositionSizingError(
                "Invalid price level".to_string(),
            ));
        };

        // Ensure within risk limits
        let max_position_value = Decimal::from_f64_retain(risk_config.max_position_size_dollars)
            .unwrap_or(Decimal::ZERO);
        let max_shares = if signal.price_level.value() > Decimal::ZERO {
            (max_position_value / signal.price_level.value()).floor()
        } else {
            Decimal::ZERO
        };

        let final_shares = shares.min(max_shares);

        let quantity_value = final_shares
            .to_u32()
            .ok_or_else(|| ExecutionError::PositionSizingError("Invalid quantity".to_string()))?;

        Quantity::new(quantity_value)
            .map_err(|e| ExecutionError::PositionSizingError(e.to_string()))
    }

    fn determine_order_type(&self, _signal: &TradingSignal, _current_price: Price) -> OrderType {
        if self.config.use_limit_orders {
            OrderType::Limit
        } else {
            OrderType::Market
        }
    }

    fn determine_time_in_force(&self, _signal: &TradingSignal) -> TimeInForce {
        TimeInForce::Day
    }

    fn calculate_limit_price(&self, signal: &TradingSignal, current_price: Price) -> Option<Price> {
        if !self.config.use_limit_orders {
            return None;
        }

        // Calculate offset based on signal type
        let offset = current_price.value() * self.config.limit_offset_percent;

        let limit_value = match signal.signal_type {
            crate::events::types::SignalType::Buy => {
                // Buy slightly below current price
                current_price.value() - offset
            }
            crate::events::types::SignalType::Sell => {
                // Sell slightly above current price
                current_price.value() + offset
            }
        };

        Price::new(limit_value).ok()
    }

    fn name(&self) -> &str {
        "Aggressive"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::Confidence;
    use rust_decimal_macros::dec;

    fn create_test_signal() -> TradingSignal {
        TradingSignal::new(
            Symbol::new("AAPL").unwrap(),
            crate::events::types::SignalType::Buy,
            Confidence::new(0.80).unwrap(),
            Price::new(dec!(150.00)).unwrap(),
            crate::patterns::signals::MarketContext::new(
                crate::patterns::types::TrendContext::Sideways { volatility: 0.5 },
            ),
        )
    }

    fn create_test_account() -> Account {
        Account {
            account_number: "test-account".to_string(),
            buying_power: dec!(100000.00),
            cash: dec!(100000.00),
            portfolio_value: dec!(100000.00),
            day_trade_count: 0,
            is_pattern_day_trader: false,
            equity: dec!(100000.00),
            last_equity: dec!(100000.00),
        }
    }

    fn create_test_risk_config() -> RiskConfig {
        RiskConfig {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 10000.0,
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
        }
    }

    #[test]
    fn test_conservative_strategy_position_sizing() {
        let config = ConservativeConfig::default();
        let strategy = ConservativeStrategy::new(config);
        let signal = create_test_signal();
        let account = create_test_account();
        let risk_config = create_test_risk_config();

        let quantity = strategy
            .calculate_position_size(&signal, &account, &risk_config)
            .unwrap();

        // 5% of $100,000 = $5,000 / $150 = 33 shares (with confidence adjustment)
        assert!(quantity.value() > 0);
        assert!(quantity.value() <= 40); // Should be around 33-40 shares
    }

    #[test]
    fn test_aggressive_strategy_position_sizing() {
        let config = AggressiveConfig::default();
        let strategy = AggressiveStrategy::new(config);
        let signal = create_test_signal(); // 0.80 confidence
        let account = create_test_account();
        let risk_config = create_test_risk_config();

        let quantity = strategy
            .calculate_position_size(&signal, &account, &risk_config)
            .unwrap();

        // Should be larger than conservative due to confidence-based scaling
        assert!(quantity.value() > 0);
    }

    #[test]
    fn test_conservative_order_type() {
        let config = ConservativeConfig::default();
        let strategy = ConservativeStrategy::new(config);
        let signal = create_test_signal();
        let current_price = Price::new(dec!(150.00)).unwrap();

        let order_type = strategy.determine_order_type(&signal, current_price);
        assert_eq!(order_type, OrderType::Market);
    }

    #[test]
    fn test_aggressive_limit_price() {
        let config = AggressiveConfig::default();
        let strategy = AggressiveStrategy::new(config);
        let signal = create_test_signal();
        let current_price = Price::new(dec!(150.00)).unwrap();

        let limit_price = strategy.calculate_limit_price(&signal, current_price);
        assert!(limit_price.is_some());

        let limit = limit_price.unwrap();
        // Buy order should be slightly below current price
        assert!(limit.value() < current_price.value());
    }
}
