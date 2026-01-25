//! Signal generation engine with market context analysis
//!
//! This module provides a comprehensive signal generation system that combines
//! pattern detection results and technical indicators into actionable trading signals.
//! The system uses weighted confidence scoring, market context analysis, and
//! real-time streaming pipelines.
//!
//! # Architecture
//!
//! The signal generation system consists of several key components:
//!
//! - **Signal Types**: Core signal structures with confidence scoring
//! - **Market Context**: Trend analysis, support/resistance, volume confirmation
//! - **Signal Composition**: Multi-factor signal combination with weighted scoring
//! - **Real-time Pipeline**: Streaming signal generation with caching
//! - **Event Integration**: Event bus publishing for signal distribution
//!
//! # Design Principles
//!
//! - **Functional Programming**: Pure functions with immutable data
//! - **Type Safety**: Strong typing with newtype pattern for domain concepts
//! - **Performance**: Optimized for real-time processing with minimal allocations
//! - **Composability**: Signals can be combined and filtered dynamically
//! - **Extensibility**: Easy to add new signal types and composition rules
//!
//! # Examples
//!
//! ## Basic Signal Generation
//!
//! ```
//! use daytrader::patterns::signals::{SignalEngine, SignalConfig};
//! use daytrader::patterns::types::PatternResult;
//! use daytrader::events::types::Candle;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = SignalConfig::default();
//! let mut engine = SignalEngine::new(config);
//!
//! // Process pattern results and generate signals
//! let patterns = vec![]; // Pattern results from detection
//! let candles = vec![]; // Recent candle data
//!
//! let signals = engine.generate_signals(&patterns, &candles).await?;
//!
//! for signal in signals {
//!     println!("Generated signal: {} with confidence {:.2}",
//!              signal.signal_type, signal.confidence.value());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Market Context Analysis
//!
//! ```
//! use daytrader::patterns::signals::MarketContextAnalyzer;
//! use daytrader::events::types::Candle;
//!
//! # fn example() {
//! let analyzer = MarketContextAnalyzer::new();
//! let candles = vec![]; // Historical candle data
//!
//! let context = analyzer.analyze_context(&candles);
//!
//! if context.trend.is_bullish() {
//!     println!("Market is in uptrend with strength: {:.2}",
//!              context.trend.strength());
//! }
//!
//! for level in &context.support_resistance_levels {
//!     println!("Key level: ${}", level.value());
//! }
//! # }
//! ```

use crate::brokers::types::{Confidence, PatternName, Price, Symbol};
use crate::events::types::{Candle, MarketEvent, SignalType};
use crate::patterns::indicators;
use crate::patterns::types::{PatternContext, PatternError, PatternResult, TrendContext, VolumeConfirmation};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, prelude::*};
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, ToSmallVec};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result type for signal generation operations
pub type SignalResult<T> = Result<T, PatternError>;

/// Trading signal with enhanced context and confidence scoring
///
/// This represents a complete trading signal with all necessary information
/// for decision making and risk assessment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradingSignal {
    /// Unique signal identifier
    pub id: String,
    /// Trading symbol
    pub symbol: Symbol,
    /// Signal type (buy/sell)
    pub signal_type: SignalType,
    /// Overall confidence score (0.0 to 1.0)
    pub confidence: Confidence,
    /// Price level where signal was generated
    pub price_level: Price,
    /// When the signal was generated
    pub generated_at: DateTime<Utc>,
    /// Market context at signal generation
    pub market_context: MarketContext,
    /// Component signals that contributed to this signal
    pub components: SmallVec<[SignalComponent; 8]>,
    /// Signal metadata and reasoning
    pub metadata: HashMap<String, String>,
}

impl TradingSignal {
    /// Creates a new trading signal
    pub fn new(
        symbol: Symbol,
        signal_type: SignalType,
        confidence: Confidence,
        price_level: Price,
        market_context: MarketContext,
    ) -> Self {
        Self {
            id: format!("{}_{}_{}", symbol.as_str(), signal_type.as_str(), Utc::now().timestamp()),
            symbol,
            signal_type,
            confidence,
            price_level,
            generated_at: Utc::now(),
            market_context,
            components: SmallVec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Adds a component signal to this trading signal
    pub fn add_component(mut self, component: SignalComponent) -> Self {
        self.components.push(component);
        self
    }

    /// Adds metadata to the signal
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns true if this is a high-confidence signal
    pub fn is_high_confidence(&self) -> bool {
        self.confidence.value() >= 0.8
    }

    /// Returns true if this is a medium-confidence signal
    pub fn is_medium_confidence(&self) -> bool {
        self.confidence.value() >= 0.6
    }

    /// Returns the weighted confidence based on market context
    pub fn context_adjusted_confidence(&self) -> f64 {
        let base_confidence = self.confidence.value();
        let context_multiplier = self.market_context.confidence_multiplier();

        (base_confidence * context_multiplier).min(1.0)
    }
}

/// Component signal that contributes to a trading signal
///
/// Represents individual pattern or indicator signals that are combined
/// into a final trading signal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalComponent {
    /// Component type (pattern, indicator, etc.)
    pub component_type: SignalComponentType,
    /// Name of the component (e.g., "hammer", "rsi_oversold")
    pub name: String,
    /// Individual confidence score
    pub confidence: f64,
    /// Weight in the final signal (0.0 to 1.0)
    pub weight: f64,
    /// Signal direction (1.0 for buy, -1.0 for sell)
    pub direction: f64,
    /// Additional component data
    pub data: HashMap<String, String>,
}

impl SignalComponent {
    /// Creates a new signal component
    pub fn new(
        component_type: SignalComponentType,
        name: impl Into<String>,
        confidence: f64,
        weight: f64,
        direction: f64,
    ) -> Self {
        Self {
            component_type,
            name: name.into(),
            confidence: confidence.clamp(0.0, 1.0),
            weight: weight.clamp(0.0, 1.0),
            direction: direction.clamp(-1.0, 1.0),
            data: HashMap::new(),
        }
    }

    /// Adds data to the component
    pub fn add_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    /// Returns the weighted contribution to the final signal
    pub fn weighted_contribution(&self) -> f64 {
        self.confidence * self.weight * self.direction
    }
}

/// Types of signal components
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignalComponentType {
    /// Candlestick pattern
    Pattern,
    /// Technical indicator
    Indicator,
    /// Volume analysis
    Volume,
    /// Market structure
    MarketStructure,
    /// Custom component
    Custom,
}

impl SignalComponentType {
    /// Returns the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pattern => "pattern",
            Self::Indicator => "indicator",
            Self::Volume => "volume",
            Self::MarketStructure => "market_structure",
            Self::Custom => "custom",
        }
    }
}

/// Market context information for signal validation
///
/// Contains trend analysis, support/resistance levels, and other
/// market context data that affects signal reliability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketContext {
    /// Current trend context
    pub trend: TrendContext,
    /// Key support and resistance levels
    pub support_resistance_levels: Vec<Price>,
    /// Volume confirmation status
    pub volume_confirmation: Option<VolumeConfirmation>,
    /// Volatility level (0.0 = low, 1.0 = high)
    pub volatility: f64,
    /// Market strength indicator
    pub market_strength: f64,
    /// Additional context data
    pub metadata: HashMap<String, String>,
}

impl MarketContext {
    /// Creates a new market context
    pub fn new(trend: TrendContext) -> Self {
        Self {
            trend,
            support_resistance_levels: Vec::new(),
            volume_confirmation: None,
            volatility: 0.5,
            market_strength: 0.5,
            metadata: HashMap::new(),
        }
    }

    /// Adds a support/resistance level
    pub fn add_level(mut self, level: Price) -> Self {
        self.support_resistance_levels.push(level);
        self
    }

    /// Sets volume confirmation
    pub fn with_volume_confirmation(mut self, confirmation: VolumeConfirmation) -> Self {
        self.volume_confirmation = Some(confirmation);
        self
    }

    /// Sets volatility level
    pub fn with_volatility(mut self, volatility: f64) -> Self {
        self.volatility = volatility.clamp(0.0, 1.0);
        self
    }

    /// Sets market strength
    pub fn with_market_strength(mut self, strength: f64) -> Self {
        self.market_strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Returns a confidence multiplier based on market context
    pub fn confidence_multiplier(&self) -> f64 {
        let mut multiplier = 1.0;

        // Trend alignment bonus
        if self.trend.is_bullish() || self.trend.is_bearish() {
            multiplier *= 1.1;
        }

        // Volume confirmation bonus
        if self.volume_confirmation.as_ref().map_or(false, |vc| vc.is_confirmed()) {
            multiplier *= 1.05;
        }

        // High volatility penalty
        if self.volatility > 0.7 {
            multiplier *= 0.9;
        }

        // Market strength bonus
        multiplier *= 0.9 + (self.market_strength * 0.2);

        multiplier.clamp(0.5, 1.5)
    }
}

/// Configuration for the signal engine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalConfig {
    /// Minimum confidence threshold for signal generation
    pub min_confidence: f64,
    /// Maximum number of signals to cache
    pub max_cache_size: usize,
    /// Signal cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Component weights for signal composition
    pub component_weights: HashMap<String, f64>,
    /// Enable market context analysis
    pub enable_market_context: bool,
    /// Minimum volume ratio for confirmation
    pub min_volume_ratio: f64,
    /// Trend analysis period
    pub trend_period: usize,
    /// Support/resistance lookback period
    pub sr_lookback_period: usize,
}

impl Default for SignalConfig {
    fn default() -> Self {
        let mut component_weights = HashMap::new();
        component_weights.insert("pattern".to_string(), 0.4);
        component_weights.insert("indicator".to_string(), 0.3);
        component_weights.insert("volume".to_string(), 0.2);
        component_weights.insert("market_structure".to_string(), 0.1);

        Self {
            min_confidence: 0.6,
            max_cache_size: 1000,
            cache_ttl_seconds: 300, // 5 minutes
            component_weights,
            enable_market_context: true,
            min_volume_ratio: 1.2,
            trend_period: 20,
            sr_lookback_period: 50,
        }
    }
}

/// Market context analyzer for trend and structure analysis
#[derive(Clone)]
pub struct MarketContextAnalyzer {
    /// Configuration for analysis
    config: SignalConfig,
}

impl MarketContextAnalyzer {
    /// Creates a new market context analyzer
    pub fn new() -> Self {
        Self {
            config: SignalConfig::default(),
        }
    }

    /// Creates analyzer with custom config
    pub fn with_config(config: SignalConfig) -> Self {
        Self { config }
    }

    /// Analyzes market context from candle data
    pub fn analyze_context(&self, candles: &[Candle]) -> MarketContext {
        if candles.is_empty() {
            return MarketContext::new(TrendContext::Sideways { volatility: 0.5 });
        }

        let trend = self.analyze_trend(candles);
        let mut context = MarketContext::new(trend);

        // Add support/resistance levels
        let levels = self.find_support_resistance_levels(candles);
        for level in levels {
            context = context.add_level(level);
        }

        // Analyze volume confirmation
        if let Some(volume_conf) = self.analyze_volume_confirmation(candles) {
            context = context.with_volume_confirmation(volume_conf);
        }

        // Calculate volatility
        let volatility = self.calculate_volatility(candles);
        context = context.with_volatility(volatility);

        // Calculate market strength
        let strength = self.calculate_market_strength(candles);
        context = context.with_market_strength(strength);

        context
    }

    /// Analyzes trend using moving averages and price action
    fn analyze_trend(&self, candles: &[Candle]) -> TrendContext {
        if candles.len() < self.config.trend_period {
            return TrendContext::Sideways { volatility: 0.5 };
        }

        let closes: Vec<Decimal> = candles.iter().map(|c| c.close.value()).collect();

        // Calculate short and long term moving averages
        let short_ma = indicators::sma(&closes, 10).unwrap_or_default();
        let long_ma = indicators::sma(&closes, self.config.trend_period).unwrap_or_default();

        if short_ma.is_empty() || long_ma.is_empty() {
            return TrendContext::Sideways { volatility: 0.5 };
        }

        let short_ma_latest = short_ma.last().copied().unwrap_or(Decimal::ZERO);
        let long_ma_latest = long_ma.last().copied().unwrap_or(Decimal::ZERO);

        let strength = ((short_ma_latest - long_ma_latest) / long_ma_latest)
            .to_f64()
            .unwrap_or(0.0)
            .abs();

        if short_ma_latest > long_ma_latest {
            TrendContext::from_strength(strength)
        } else {
            TrendContext::from_strength(-strength)
        }
    }

    /// Finds support and resistance levels using pivot points
    fn find_support_resistance_levels(&self, candles: &[Candle]) -> Vec<Price> {
        let mut levels = Vec::new();
        let lookback = self.config.sr_lookback_period.min(candles.len());

        for i in 2..lookback.saturating_sub(2) {
            let current = &candles[i];
            let prev = &candles[i - 1];
            let next = &candles[i + 1];

            // Pivot high (resistance)
            if current.high > prev.high && current.high > next.high {
                if let Ok(price) = Price::new(current.high.value()) {
                    levels.push(price);
                }
            }

            // Pivot low (support)
            if current.low < prev.low && current.low < next.low {
                if let Ok(price) = Price::new(current.low.value()) {
                    levels.push(price);
                }
            }
        }

        // Remove duplicates and sort
        levels.sort_by(|a, b| a.value().cmp(&b.value()));
        levels.dedup_by(|a, b| (a.value() - b.value()).abs() < Decimal::new(1, 2)); // Within 0.01

        levels
    }

    /// Analyzes volume confirmation
    fn analyze_volume_confirmation(&self, candles: &[Candle]) -> Option<VolumeConfirmation> {
        if candles.len() < 20 {
            return None;
        }

        let recent_volume: u64 = candles[candles.len().saturating_sub(5)..]
            .iter()
            .map(|c| c.volume)
            .sum();

        let avg_volume: u64 = candles
            .iter()
            .map(|c| c.volume)
            .sum::<u64>()
            .checked_div(candles.len() as u64)
            .unwrap_or(1);

        if avg_volume == 0 {
            return None;
        }

        let ratio = recent_volume as f64 / (avg_volume * 5) as f64;
        Some(VolumeConfirmation::from_ratio(ratio))
    }

    /// Calculates volatility using ATR
    fn calculate_volatility(&self, candles: &[Candle]) -> f64 {
        if let Ok(atr_values) = indicators::atr(candles, 14) {
            if let Some(latest_atr) = atr_values.last() {
                let avg_price = candles
                    .last()
                    .map(|c| c.close.value())
                    .unwrap_or(Decimal::ZERO);

                if avg_price > Decimal::ZERO {
                    let volatility = (latest_atr.to_f64().unwrap_or(0.0) / avg_price.to_f64().unwrap_or(1.0))
                        .clamp(0.0, 1.0);
                    return volatility;
                }
            }
        }
        0.5 // Default medium volatility
    }

    /// Calculates market strength using ADX-like calculation
    fn calculate_market_strength(&self, candles: &[Candle]) -> f64 {
        if candles.len() < 14 {
            return 0.5;
        }

        let mut plus_dm = 0.0;
        let mut minus_dm = 0.0;
        let mut tr_sum = 0.0;

        for i in 1..candles.len().min(14) {
            let current = &candles[i];
            let prev = &candles[i - 1];

            let move_up = (current.high.value() - prev.high.value()).to_f64().unwrap_or(0.0);
            let move_down = (prev.low.value() - current.low.value()).to_f64().unwrap_or(0.0);

            if move_up > move_down && move_up > 0.0 {
                plus_dm += move_up;
            } else if move_down > move_up && move_down > 0.0 {
                minus_dm += move_down;
            }

            let tr = [
                (current.high.value() - current.low.value()).to_f64().unwrap_or(0.0),
                (current.high.value() - prev.close.value()).abs().to_f64().unwrap_or(0.0),
                (current.low.value() - prev.close.value()).abs().to_f64().unwrap_or(0.0),
            ].iter().cloned().fold(0.0, f64::max);

            tr_sum += tr;
        }

        if tr_sum == 0.0 {
            return 0.5;
        }

        let plus_di = (plus_dm / tr_sum) * 100.0;
        let minus_di = (minus_dm / tr_sum) * 100.0;

        let adx = ((plus_di - minus_di).abs() / (plus_di + minus_di)) * 100.0;
        (adx / 100.0).clamp(0.0, 1.0)
    }
}

/// Signal composition engine that combines multiple signal components
#[derive(Clone)]
pub struct SignalComposer {
    /// Configuration for composition
    config: SignalConfig,
}

impl SignalComposer {
    /// Creates a new signal composer
    pub fn new() -> Self {
        Self {
            config: SignalConfig::default(),
        }
    }

    /// Creates composer with custom config
    pub fn with_config(config: SignalConfig) -> Self {
        Self { config }
    }

    /// Composes a trading signal from pattern results and market context
    pub fn compose_signal(
        &self,
        symbol: Symbol,
        pattern_results: &[PatternResult],
        market_context: &MarketContext,
        candles: &[Candle],
    ) -> Option<TradingSignal> {
        let mut components = Vec::new();

        // Convert pattern results to signal components
        for pattern in pattern_results {
            if let Some(component) = self.pattern_to_component(pattern) {
                components.push(component);
            }
        }

        // Add indicator-based components
        if let Some(indicator_components) = self.generate_indicator_components(candles) {
            components.extend(indicator_components);
        }

        // Add market structure components
        if let Some(structure_components) = self.generate_market_structure_components(market_context) {
            components.extend(structure_components);
        }

        if components.is_empty() {
            return None;
        }

        // Calculate weighted confidence and direction
        let (total_weight, weighted_sum) = components
            .iter()
            .map(|c| (c.weight, c.weighted_contribution()))
            .fold((0.0, 0.0), |(w_acc, s_acc), (w, s)| (w_acc + w, s_acc + s));

        if total_weight == 0.0 {
            return None;
        }

        let avg_direction = weighted_sum / total_weight;
        let confidence = components
            .iter()
            .map(|c| c.confidence * c.weight)
            .sum::<f64>() / total_weight;

        if confidence < self.config.min_confidence {
            return None;
        }

        let signal_type = if avg_direction > 0.1 {
            SignalType::Buy
        } else if avg_direction < -0.1 {
            SignalType::Sell
        } else {
            return None; // Neutral signal
        };

        let confidence = Confidence::new(confidence.clamp(0.0, 1.0)).ok()?;
        let price_level = candles
            .last()
            .and_then(|c| Price::new(c.close.value()).ok())
            .unwrap_or_else(|| Price::new(Decimal::ZERO).unwrap());

        let mut signal = TradingSignal::new(
            symbol,
            signal_type,
            confidence,
            price_level,
            market_context.clone(),
        );

        signal.components = components.into();

        Some(signal)
    }

    /// Converts a pattern result to a signal component
    fn pattern_to_component(&self, pattern: &PatternResult) -> Option<SignalComponent> {
        let weight = self.config.component_weights.get("pattern").copied().unwrap_or(0.4);
        let direction = match pattern.signal_type {
            SignalType::Buy => 1.0,
            SignalType::Sell => -1.0,
        };

        Some(SignalComponent::new(
            SignalComponentType::Pattern,
            pattern.pattern_name.as_str(),
            pattern.confidence.value(),
            weight,
            direction,
        ))
    }

    /// Generates indicator-based signal components
    fn generate_indicator_components(&self, candles: &[Candle]) -> Option<Vec<SignalComponent>> {
        if candles.len() < 50 {
            return None;
        }

        let closes: Vec<Decimal> = candles.iter().map(|c| c.close.value()).collect();
        let mut components = Vec::new();

        // RSI component
        if let Ok(rsi_values) = indicators::rsi(&closes, 14) {
            if let Some(&rsi) = rsi_values.last() {
                let weight = self.config.component_weights.get("indicator").copied().unwrap_or(0.3);
                let (confidence, direction) = if rsi < 30.0 {
                    (0.8, 1.0) // Oversold - bullish
                } else if rsi > 70.0 {
                    (0.8, -1.0) // Overbought - bearish
                } else {
                    (0.5, 0.0) // Neutral
                };

                components.push(SignalComponent::new(
                    SignalComponentType::Indicator,
                    "rsi",
                    confidence,
                    weight * 0.4,
                    direction,
                ).add_data("rsi_value", rsi.to_string()));
            }
        }

        // MACD component
        if let Ok((macd_line, signal_line, _)) = indicators::macd(&closes, 12, 26, 9) {
            if let (Some(&macd), Some(&signal)) = (macd_line.last(), signal_line.last()) {
                let weight = self.config.component_weights.get("indicator").copied().unwrap_or(0.3);
                let direction = if macd > signal { 1.0 } else { -1.0 };
                let confidence = 0.7; // MACD signals are generally reliable

                components.push(SignalComponent::new(
                    SignalComponentType::Indicator,
                    "macd",
                    confidence,
                    weight * 0.6,
                    direction,
                ));
            }
        }

        Some(components)
    }

    /// Generates market structure-based signal components
    fn generate_market_structure_components(&self, context: &MarketContext) -> Option<Vec<SignalComponent>> {
        let mut components = Vec::new();
        let weight = self.config.component_weights.get("market_structure").copied().unwrap_or(0.1);

        // Trend component
        let trend_direction = if context.trend.is_bullish() {
            1.0
        } else if context.trend.is_bearish() {
            -1.0
        } else {
            0.0
        };

        if trend_direction != 0.0 {
            components.push(SignalComponent::new(
                SignalComponentType::MarketStructure,
                "trend",
                context.trend.strength().min(1.0),
                weight * 0.6,
                trend_direction,
            ));
        }

        // Support/resistance component
        if !context.support_resistance_levels.is_empty() {
            components.push(SignalComponent::new(
                SignalComponentType::MarketStructure,
                "key_levels",
                0.6, // Moderate confidence for level-based signals
                weight * 0.4,
                0.0, // Neutral direction
            ).add_data("level_count", context.support_resistance_levels.len().to_string()));
        }

        Some(components)
    }
}

/// Real-time signal generation engine
pub struct SignalEngine {
    /// Configuration
    config: SignalConfig,
    /// Market context analyzer
    context_analyzer: MarketContextAnalyzer,
    /// Signal composer
    composer: SignalComposer,
    /// Signal cache for deduplication and performance
    signal_cache: Arc<RwLock<SignalCache>>,
    /// Risk engine for position sizing and filtering
    risk_engine: Option<Arc<crate::risk::engine::RiskEngine>>,
    /// Signal repository for database persistence
    signal_repository: Option<Arc<crate::db::repositories::SignalRepository>>,
}

impl SignalEngine {
    /// Creates a new signal engine
    pub fn new(config: SignalConfig) -> Self {
        Self::with_risk_engine(config, None)
    }

    /// Creates a new signal engine with risk integration
    pub fn with_risk_engine(
        config: SignalConfig,
        risk_engine: Option<Arc<crate::risk::engine::RiskEngine>>,
    ) -> Self {
        Self {
            context_analyzer: MarketContextAnalyzer::with_config(config.clone()),
            composer: SignalComposer::with_config(config.clone()),
            signal_cache: Arc::new(RwLock::new(SignalCache::new(config.max_cache_size))),
            config,
            risk_engine,
            signal_repository: None,
        }
    }

    /// Sets the signal repository for database persistence
    pub fn with_signal_repository(
        mut self,
        repository: Arc<crate::db::repositories::SignalRepository>,
    ) -> Self {
        self.signal_repository = Some(repository);
        self
    }

    /// Generates trading signals from pattern results and candle data
    pub async fn generate_signals(
        &mut self,
        pattern_results: &[PatternResult],
        candles: &[Candle],
    ) -> SignalResult<Vec<TradingSignal>> {
        if pattern_results.is_empty() || candles.is_empty() {
            return Ok(Vec::new());
        }

        // Use parallel processing for multiple symbols if available
        self.generate_signals_parallel(pattern_results, candles).await
    }

    /// Generates signals with parallel processing for better performance
    async fn generate_signals_parallel(
        &mut self,
        pattern_results: &[PatternResult],
        candles: &[Candle],
    ) -> SignalResult<Vec<TradingSignal>> {
        use futures::future::join_all;

        // Group patterns by symbol for parallel processing
        let mut patterns_by_symbol: HashMap<String, Vec<&PatternResult>> = HashMap::new();

        for pattern in pattern_results {
            // In a full implementation, patterns would have symbol information
            // For now, assume all patterns are for the same symbol
            let symbol_key = "DEFAULT".to_string();
            patterns_by_symbol.entry(symbol_key).or_insert_with(Vec::new).push(pattern);
        }

        // Process each symbol in parallel
        let mut signal_futures = Vec::new();

        for (symbol_str, patterns) in patterns_by_symbol {
            let symbol = Symbol::new(&symbol_str).unwrap_or_else(|_| Symbol::new("UNKNOWN").unwrap());
            let patterns_clone = patterns.iter().map(|&p| p.clone()).collect::<Vec<_>>();
            let candles_clone = candles.to_vec();

            let config = self.config.clone();
            let context_analyzer = self.context_analyzer.clone();
            let composer = self.composer.clone();
            let risk_engine = self.risk_engine.clone();

            let future = async move {
                Self::generate_signal_for_symbol_static(
                    &symbol,
                    &patterns_clone,
                    &candles_clone,
                    &config,
                    &context_analyzer,
                    &composer,
                    &risk_engine,
                ).await
            };

            signal_futures.push(future);
        }

        // Wait for all parallel signal generation to complete
        let signal_results = join_all(signal_futures).await;

        // Collect valid signals and apply caching
        let mut signals = Vec::new();
        let cache = self.signal_cache.read().await;

        for signal_option in signal_results {
            if let Some(signal) = signal_option {
                if !cache.is_duplicate(&signal) {
                    signals.push(signal);
                }
            }
        }

        // Cache the new signals
        drop(cache); // Release read lock
        let mut cache = self.signal_cache.write().await;
        for signal in &signals {
            cache.add_signal(signal.clone());
        }

        // Persist signals to database if repository is available
        if let Some(ref repo) = self.signal_repository {
            for signal in &signals {
                if let Err(e) = repo.save_trading_signal(signal).await {
                    tracing::error!("Failed to persist signal to database: {}", e);
                }
            }
        }

        Ok(signals)
    }

    /// Generates a signal for a specific symbol (static version for async closures)
    async fn generate_signal_for_symbol_static(
        symbol: &Symbol,
        patterns: &[PatternResult],
        candles: &[Candle],
        config: &SignalConfig,
        context_analyzer: &MarketContextAnalyzer,
        composer: &SignalComposer,
        risk_engine: &Option<Arc<crate::risk::engine::RiskEngine>>,
    ) -> Option<TradingSignal> {
        // Analyze market context
        let market_context = if config.enable_market_context {
            context_analyzer.analyze_context(candles)
        } else {
            MarketContext::new(TrendContext::Sideways { volatility: 0.5 })
        };

        // Compose initial signal from patterns and context
        let mut signal = composer.compose_signal(
            symbol.clone(),
            patterns,
            &market_context,
            candles,
        )?;

        // Apply risk-adjusted filtering and position sizing if risk engine is available
        if let Some(risk_engine) = risk_engine {
            // Apply risk-adjusted filtering
            signal = Self::apply_risk_filtering_static(signal, risk_engine).await?;

            // Apply signal-based position sizing
            signal = Self::apply_position_sizing_static(signal, risk_engine, candles, config).await;
        }

        Some(signal)
    }

    /// Generates a signal for a specific symbol
    async fn generate_signal_for_symbol(
        &self,
        symbol: &Symbol,
        patterns: &[&PatternResult],
        candles: &[Candle],
    ) -> Option<TradingSignal> {
        // Analyze market context
        let market_context = if self.config.enable_market_context {
            self.context_analyzer.analyze_context(candles)
        } else {
            MarketContext::new(TrendContext::Sideways { volatility: 0.5 })
        };

        // Compose initial signal from patterns and context
        let mut signal = self.composer.compose_signal(
            symbol.clone(),
            patterns.iter().map(|&p| p.clone()).collect::<Vec<_>>().as_slice(),
            &market_context,
            candles,
        )?;

        // Apply risk-adjusted filtering and position sizing if risk engine is available
        if let Some(ref risk_engine) = self.risk_engine {
            // Apply risk-adjusted filtering
            signal = self.apply_risk_filtering(signal, risk_engine).await?;

            // Apply signal-based position sizing
            signal = self.apply_position_sizing(signal, risk_engine, candles).await;
        }

        Some(signal)
    }

    /// Applies risk-adjusted filtering to the signal (static version)
    async fn apply_risk_filtering_static(
        mut signal: TradingSignal,
        risk_engine: &Arc<crate::risk::engine::RiskEngine>,
    ) -> Option<TradingSignal> {
        // Get pattern name from signal components
        let pattern_name = signal.components.first()
            .and_then(|comp| {
                if let SignalComponentType::Pattern = comp.component_type {
                    Some(crate::brokers::types::PatternName::new(&comp.name))
                } else {
                    None
                }
            });

        if let Some(pattern_name) = pattern_name {
            // Check if pattern is enabled and meets risk criteria
            let evaluator = risk_engine.get_pattern_risk_evaluator().read().await;
            let order_value = rust_decimal::Decimal::new(100000, 2); // Placeholder order value

            let evaluation = evaluator.evaluate_pattern_trade(
                &pattern_name,
                signal.confidence.value(),
                order_value,
            );

            match evaluation {
                crate::risk::types::RiskEvaluation::Passed => {
                    // Signal passes risk filtering, adjust confidence based on risk multipliers
                    let risk_multiplier = evaluator.get_pattern_state(&pattern_name)
                        .map(|state| state.current_risk_multiplier)
                        .unwrap_or(1.0);

                    let adjusted_confidence = (signal.confidence.value() * risk_multiplier).min(1.0);
                    signal.confidence = crate::brokers::types::Confidence::new(adjusted_confidence)
                        .unwrap_or(signal.confidence);

                    Some(signal)
                }
                crate::risk::types::RiskEvaluation::Violated { .. } => {
                    // Signal violates risk rules, filter it out
                    tracing::debug!("Signal filtered out due to risk violation: {}", pattern_name.as_str());
                    None
                }
            }
        } else {
            // No pattern component found, return signal as-is
            Some(signal)
        }
    }

    /// Applies risk-adjusted filtering to the signal
    async fn apply_risk_filtering(
        &self,
        mut signal: TradingSignal,
        risk_engine: &Arc<crate::risk::engine::RiskEngine>,
    ) -> Option<TradingSignal> {
        // Get pattern name from signal components
        let pattern_name = signal.components.first()
            .and_then(|comp| {
                if let SignalComponentType::Pattern = comp.component_type {
                    Some(crate::brokers::types::PatternName::new(&comp.name))
                } else {
                    None
                }
            });

        if let Some(pattern_name) = pattern_name {
            // Check if pattern is enabled and meets risk criteria
            let evaluator = risk_engine.get_pattern_risk_evaluator().read().await;
            let order_value = rust_decimal::Decimal::new(100000, 2); // Placeholder order value

            let evaluation = evaluator.evaluate_pattern_trade(
                &pattern_name,
                signal.confidence.value(),
                order_value,
            );

            match evaluation {
                crate::risk::types::RiskEvaluation::Passed => {
                    // Signal passes risk filtering, adjust confidence based on risk multipliers
                    let risk_multiplier = evaluator.get_pattern_state(&pattern_name)
                        .map(|state| state.current_risk_multiplier)
                        .unwrap_or(1.0);

                    let adjusted_confidence = (signal.confidence.value() * risk_multiplier).min(1.0);
                    signal.confidence = crate::brokers::types::Confidence::new(adjusted_confidence)
                        .unwrap_or(signal.confidence);

                    Some(signal)
                }
                crate::risk::types::RiskEvaluation::Violated { .. } => {
                    // Signal violates risk rules, filter it out
                    tracing::debug!("Signal filtered out due to risk violation: {}", pattern_name.as_str());
                    None
                }
            }
        } else {
            // No pattern component found, return signal as-is
            Some(signal)
        }
    }

    /// Applies signal-based position sizing (static version)
    async fn apply_position_sizing_static(
        mut signal: TradingSignal,
        risk_engine: &Arc<crate::risk::engine::RiskEngine>,
        candles: &[Candle],
        config: &SignalConfig,
    ) -> TradingSignal {
        // Get current price for position sizing calculation
        let current_price = candles.last()
            .map(|c| c.close.value())
            .unwrap_or(rust_decimal::Decimal::ZERO);

        if current_price == rust_decimal::Decimal::ZERO {
            return signal;
        }

        // Calculate base position size from risk engine
        let base_position_size = risk_engine.get_position_sizer().calculate_position_size(
            &signal.symbol.as_str(),
            current_price,
            signal.confidence.value(),
        ).await;

        // Apply signal-specific adjustments
        let signal_multiplier = Self::calculate_signal_position_multiplier_static(&signal, config);

        // Calculate final position size
        let final_position_size = base_position_size * rust_decimal::Decimal::from_f64_retain(signal_multiplier).unwrap_or(rust_decimal::Decimal::ONE);

        // Add position sizing metadata to signal
        signal
            .add_metadata("position_size", &final_position_size.to_string())
            .add_metadata("base_position_size", &base_position_size.to_string())
            .add_metadata("signal_multiplier", &signal_multiplier.to_string())
    }

    /// Applies signal-based position sizing
    async fn apply_position_sizing(
        &self,
        mut signal: TradingSignal,
        risk_engine: &Arc<crate::risk::engine::RiskEngine>,
        candles: &[Candle],
    ) -> TradingSignal {
        // Get current price for position sizing calculation
        let current_price = candles.last()
            .map(|c| c.close.value())
            .unwrap_or(rust_decimal::Decimal::ZERO);

        if current_price == rust_decimal::Decimal::ZERO {
            return signal;
        }

        // Calculate base position size from risk engine
        let base_position_size = risk_engine.get_position_sizer().calculate_position_size(
            &signal.symbol.as_str(),
            current_price,
            signal.confidence.value(),
        ).await;

        // Apply signal-specific adjustments
        let signal_multiplier = self.calculate_signal_position_multiplier(&signal);

        // Calculate final position size
        let final_position_size = base_position_size * rust_decimal::Decimal::from_f64_retain(signal_multiplier).unwrap_or(rust_decimal::Decimal::ONE);

        // Add position sizing metadata to signal
        signal
            .add_metadata("position_size", &final_position_size.to_string())
            .add_metadata("base_position_size", &base_position_size.to_string())
            .add_metadata("signal_multiplier", &signal_multiplier.to_string())
    }

    /// Calculates position size multiplier based on signal characteristics (static version)
    fn calculate_signal_position_multiplier_static(signal: &TradingSignal, _config: &SignalConfig) -> f64 {
        let mut multiplier = 1.0;

        // Confidence-based adjustment
        if signal.confidence.value() > 0.8 {
            multiplier *= 1.2; // Increase position for high confidence
        } else if signal.confidence.value() < 0.6 {
            multiplier *= 0.8; // Reduce position for low confidence
        }

        // Market context adjustment
        if signal.market_context.trend.is_bullish() || signal.market_context.trend.is_bearish() {
            multiplier *= 1.1; // Increase in trending markets
        }

        if signal.market_context.volatility > 0.7 {
            multiplier *= 0.9; // Reduce in high volatility
        }

        // Component-based adjustments
        for component in &signal.components {
            match component.component_type {
                SignalComponentType::Pattern => {
                    multiplier *= 1.0 + (component.confidence - 0.5) * 0.2; // Pattern confidence adjustment
                }
                SignalComponentType::Indicator => {
                    multiplier *= 0.95; // Slightly reduce for indicator-only signals
                }
                _ => {}
            }
        }

        multiplier.clamp(0.5, 2.0) // Reasonable bounds
    }

    /// Calculates position size multiplier based on signal characteristics
    fn calculate_signal_position_multiplier(&self, signal: &TradingSignal) -> f64 {
        let mut multiplier = 1.0;

        // Confidence-based adjustment
        if signal.confidence.value() > 0.8 {
            multiplier *= 1.2; // Increase position for high confidence
        } else if signal.confidence.value() < 0.6 {
            multiplier *= 0.8; // Reduce position for low confidence
        }

        // Market context adjustment
        if signal.market_context.trend.is_bullish() || signal.market_context.trend.is_bearish() {
            multiplier *= 1.1; // Increase in trending markets
        }

        if signal.market_context.volatility > 0.7 {
            multiplier *= 0.9; // Reduce in high volatility
        }

        // Component-based adjustments
        for component in &signal.components {
            match component.component_type {
                SignalComponentType::Pattern => {
                    multiplier *= 1.0 + (component.confidence - 0.5) * 0.2; // Pattern confidence adjustment
                }
                SignalComponentType::Indicator => {
                    multiplier *= 0.95; // Slightly reduce for indicator-only signals
                }
                _ => {}
            }
        }

        multiplier.clamp(0.5, 2.0) // Reasonable bounds
    }

    /// Returns the current signal cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.signal_cache.read().await;
        (cache.size(), cache.max_size())
    }

    /// Clears the signal cache
    pub async fn clear_cache(&mut self) {
        let mut cache = self.signal_cache.write().await;
        cache.clear();
    }
}

/// Signal cache for deduplication and performance optimization
struct SignalCache {
    /// Cached signals
    signals: VecDeque<TradingSignal>,
    /// Maximum cache size
    max_size: usize,
}

impl SignalCache {
    /// Creates a new signal cache
    fn new(max_size: usize) -> Self {
        Self {
            signals: VecDeque::new(),
            max_size,
        }
    }

    /// Adds a signal to the cache
    fn add_signal(&mut self, signal: TradingSignal) {
        self.signals.push_front(signal);
        if self.signals.len() > self.max_size {
            self.signals.pop_back();
        }
    }

    /// Checks if a signal is a duplicate
    fn is_duplicate(&self, signal: &TradingSignal) -> bool {
        self.signals.iter().any(|cached| {
            cached.symbol == signal.symbol
                && cached.signal_type == signal.signal_type
                && (Utc::now() - cached.generated_at).num_seconds() < 60 // Within 1 minute
        })
    }

    /// Returns the current cache size
    fn size(&self) -> usize {
        self.signals.len()
    }

    /// Returns the maximum cache size
    fn max_size(&self) -> usize {
        self.max_size
    }

    /// Clears the cache
    fn clear(&mut self) {
        self.signals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_trading_signal_creation() {
        let symbol = Symbol::new("AAPL").unwrap();
        let confidence = Confidence::new(0.85).unwrap();
        let price = Price::new(Decimal::new(15000, 2)).unwrap();
        let context = MarketContext::new(TrendContext::StrongUptrend { strength: 0.8 });

        let signal = TradingSignal::new(
            symbol.clone(),
            SignalType::Buy,
            confidence,
            price,
            context,
        );

        assert_eq!(signal.symbol, symbol);
        assert_eq!(signal.signal_type, SignalType::Buy);
        assert_eq!(signal.confidence, confidence);
        assert!(signal.is_high_confidence());
    }

    #[test]
    fn test_signal_component() {
        let component = SignalComponent::new(
            SignalComponentType::Pattern,
            "hammer",
            0.9,
            0.4,
            1.0,
        );

        assert_eq!(component.name, "hammer");
        assert_eq!(component.confidence, 0.9);
        assert_eq!(component.weight, 0.4);
        assert_eq!(component.weighted_contribution(), 0.36);
    }

    #[test]
    fn test_market_context_confidence_multiplier() {
        let bullish_context = MarketContext::new(TrendContext::StrongUptrend { strength: 0.8 })
            .with_volume_confirmation(VolumeConfirmation::Confirmed { volume_ratio: 1.5 })
            .with_market_strength(0.8);

        let multiplier = bullish_context.confidence_multiplier();
        assert!(multiplier > 1.0); // Should be boosted
    }

    #[test]
    fn test_signal_cache() {
        let mut cache = SignalCache::new(10);

        let symbol = Symbol::new("AAPL").unwrap();
        let context = MarketContext::new(TrendContext::Sideways { volatility: 0.5 });
        let signal = TradingSignal::new(
            symbol,
            SignalType::Buy,
            Confidence::new(0.8).unwrap(),
            Price::new(Decimal::new(10000, 2)).unwrap(),
            context,
        );

        cache.add_signal(signal.clone());
        assert_eq!(cache.size(), 1);
        assert!(cache.is_duplicate(&signal));

        cache.clear();
        assert_eq!(cache.size(), 0);
    }
}