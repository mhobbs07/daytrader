//! Pattern detection types and core data structures
//!
//! This module defines the fundamental types used throughout the pattern recognition system.
//! All types follow the newtype pattern for type safety and use functional programming principles.

use crate::brokers::types::{Confidence, PatternName, Price, Symbol};
use crate::events::types::{Candle, SignalType};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pattern detection result containing all information about a detected pattern
///
/// This is the primary output of pattern detection algorithms. It contains
/// everything needed to make trading decisions and integrate with the risk system.
///
/// # Examples
///
/// ```
/// use daytrader::patterns::types::{PatternResult, PatternContext};
/// use daytrader::brokers::types::{PatternName, Confidence, Price, Symbol};
/// use daytrader::events::types::SignalType;
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let result = PatternResult {
///     pattern_name: PatternName::new("hammer"),
///     signal_type: SignalType::Buy,
///     confidence: Confidence::new(0.85).unwrap(),
///     detected_at: Utc::now(),
///     price_level: Price::new(Decimal::new(15000, 2)).unwrap(),
///     context: PatternContext::default(),
/// };
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternResult {
    /// Name of the detected pattern
    pub pattern_name: PatternName,
    /// Trading signal type (buy/sell)
    pub signal_type: SignalType,
    /// Confidence score (0.0 to 1.0)
    pub confidence: Confidence,
    /// When the pattern was detected
    pub detected_at: DateTime<Utc>,
    /// Price level where pattern was detected
    pub price_level: Price,
    /// Additional context about the pattern
    pub context: PatternContext,
}

impl PatternResult {
    /// Creates a new pattern result
    pub fn new(
        pattern_name: PatternName,
        signal_type: SignalType,
        confidence: Confidence,
        price_level: Price,
    ) -> Self {
        Self {
            pattern_name,
            signal_type,
            confidence,
            detected_at: Utc::now(),
            price_level,
            context: PatternContext::default(),
        }
    }

    /// Creates a new pattern result with context
    pub fn with_context(
        pattern_name: PatternName,
        signal_type: SignalType,
        confidence: Confidence,
        price_level: Price,
        context: PatternContext,
    ) -> Self {
        Self {
            pattern_name,
            signal_type,
            confidence,
            detected_at: Utc::now(),
            price_level,
            context,
        }
    }

    /// Returns true if this is a bullish signal
    #[inline]
    pub fn is_bullish(&self) -> bool {
        matches!(self.signal_type, SignalType::Buy)
    }

    /// Returns true if this is a bearish signal
    #[inline]
    pub fn is_bearish(&self) -> bool {
        matches!(self.signal_type, SignalType::Sell)
    }

    /// Returns true if confidence is above the given threshold
    #[inline]
    pub fn confidence_above(&self, threshold: f64) -> bool {
        self.confidence.value() >= threshold
    }
}

/// Pattern context providing additional information about the detection
///
/// Contains metadata and contextual information that can help with
/// signal validation and risk assessment.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PatternContext {
    /// Number of candles used in pattern detection
    pub candles_used: usize,
    /// Volume confirmation (if applicable)
    pub volume_confirmation: Option<VolumeConfirmation>,
    /// Trend context at time of detection
    pub trend_context: Option<TrendContext>,
    /// Support/resistance levels nearby
    pub nearby_levels: Vec<Price>,
    /// Additional metadata specific to the pattern
    pub metadata: HashMap<String, String>,
}

impl PatternContext {
    /// Creates a new pattern context with the specified number of candles
    pub fn new(candles_used: usize) -> Self {
        Self {
            candles_used,
            ..Default::default()
        }
    }

    /// Adds volume confirmation to the context
    pub fn with_volume_confirmation(mut self, confirmation: VolumeConfirmation) -> Self {
        self.volume_confirmation = Some(confirmation);
        self
    }

    /// Adds trend context
    pub fn with_trend_context(mut self, trend: TrendContext) -> Self {
        self.trend_context = Some(trend);
        self
    }

    /// Adds a nearby support/resistance level
    pub fn add_nearby_level(mut self, level: Price) -> Self {
        self.nearby_levels.push(level);
        self
    }

    /// Adds metadata key-value pair
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Volume confirmation for pattern validation
///
/// Indicates whether volume supports the pattern signal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VolumeConfirmation {
    /// Volume confirms the pattern (above average)
    Confirmed { volume_ratio: f64 },
    /// Volume is neutral (near average)
    Neutral { volume_ratio: f64 },
    /// Volume contradicts the pattern (below average)
    Contradicted { volume_ratio: f64 },
}

impl VolumeConfirmation {
    /// Creates volume confirmation from volume ratio
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio >= 1.5 {
            Self::Confirmed { volume_ratio: ratio }
        } else if ratio >= 0.7 {
            Self::Neutral { volume_ratio: ratio }
        } else {
            Self::Contradicted { volume_ratio: ratio }
        }
    }

    /// Returns true if volume confirms the pattern
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    /// Returns the volume ratio
    pub fn ratio(&self) -> f64 {
        match self {
            Self::Confirmed { volume_ratio } => *volume_ratio,
            Self::Neutral { volume_ratio } => *volume_ratio,
            Self::Contradicted { volume_ratio } => *volume_ratio,
        }
    }
}

/// Trend context at the time of pattern detection
///
/// Provides information about the overall market trend which can
/// affect pattern reliability and signal strength.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrendContext {
    /// Strong uptrend
    StrongUptrend { strength: f64 },
    /// Weak uptrend
    WeakUptrend { strength: f64 },
    /// Sideways/ranging market
    Sideways { volatility: f64 },
    /// Weak downtrend
    WeakDowntrend { strength: f64 },
    /// Strong downtrend
    StrongDowntrend { strength: f64 },
}

impl TrendContext {
    /// Creates trend context from trend strength (-1.0 to 1.0)
    pub fn from_strength(strength: f64) -> Self {
        match strength {
            s if s >= 0.7 => Self::StrongUptrend { strength: s },
            s if s >= 0.3 => Self::WeakUptrend { strength: s },
            s if s >= -0.3 => Self::Sideways { volatility: s.abs() },
            s if s >= -0.7 => Self::WeakDowntrend { strength: s.abs() },
            s => Self::StrongDowntrend { strength: s.abs() },
        }
    }

    /// Returns true if trend is bullish
    pub fn is_bullish(&self) -> bool {
        matches!(self, Self::StrongUptrend { .. } | Self::WeakUptrend { .. })
    }

    /// Returns true if trend is bearish
    pub fn is_bearish(&self) -> bool {
        matches!(self, Self::StrongDowntrend { .. } | Self::WeakDowntrend { .. })
    }

    /// Returns true if market is sideways
    pub fn is_sideways(&self) -> bool {
        matches!(self, Self::Sideways { .. })
    }

    /// Returns the trend strength (always positive)
    pub fn strength(&self) -> f64 {
        match self {
            Self::StrongUptrend { strength } => *strength,
            Self::WeakUptrend { strength } => *strength,
            Self::Sideways { volatility } => *volatility,
            Self::WeakDowntrend { strength } => *strength,
            Self::StrongDowntrend { strength } => *strength,
        }
    }
}

/// Pattern detection parameters for customizing detection algorithms
///
/// Allows fine-tuning of pattern detection sensitivity and requirements.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternParams {
    /// Minimum confidence threshold for detection
    pub min_confidence: f64,
    /// Require volume confirmation
    pub require_volume_confirmation: bool,
    /// Minimum volume ratio for confirmation
    pub min_volume_ratio: f64,
    /// Consider trend context in confidence calculation
    pub use_trend_context: bool,
    /// Additional pattern-specific parameters
    pub custom_params: HashMap<String, f64>,
}

impl Default for PatternParams {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            require_volume_confirmation: false,
            min_volume_ratio: 1.2,
            use_trend_context: true,
            custom_params: HashMap::new(),
        }
    }
}

impl PatternParams {
    /// Creates new pattern parameters with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets minimum confidence threshold
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Enables volume confirmation requirement
    pub fn require_volume_confirmation(mut self, require: bool) -> Self {
        self.require_volume_confirmation = require;
        self
    }

    /// Sets minimum volume ratio for confirmation
    pub fn with_min_volume_ratio(mut self, ratio: f64) -> Self {
        self.min_volume_ratio = ratio.max(0.1);
        self
    }

    /// Enables or disables trend context usage
    pub fn use_trend_context(mut self, use_trend: bool) -> Self {
        self.use_trend_context = use_trend;
        self
    }

    /// Adds a custom parameter
    pub fn with_custom_param(mut self, key: impl Into<String>, value: f64) -> Self {
        self.custom_params.insert(key.into(), value);
        self
    }

    /// Gets a custom parameter value
    pub fn get_custom_param(&self, key: &str) -> Option<f64> {
        self.custom_params.get(key).copied()
    }
}

/// Pattern detection error types
///
/// Comprehensive error handling for pattern detection operations.
#[derive(Clone, Debug, thiserror::Error)]
pub enum PatternError {
    /// Insufficient data for pattern detection
    #[error("Insufficient data: need at least {required} candles, got {available}")]
    InsufficientData { required: usize, available: usize },

    /// Invalid pattern parameters
    #[error("Invalid pattern parameters: {message}")]
    InvalidParameters { message: String },

    /// Pattern detection failed
    #[error("Pattern detection failed: {reason}")]
    DetectionFailed { reason: String },

    /// Data quality issues
    #[error("Data quality issue: {issue}")]
    DataQuality { issue: String },

    /// Configuration error
    #[error("Configuration error: {error}")]
    Configuration { error: String },
}

/// Result type for pattern detection operations
pub type PatternResult_<T> = Result<T, PatternError>;

/// Pattern detection statistics for performance monitoring
///
/// Tracks pattern detection performance and accuracy metrics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PatternStats {
    /// Total number of detections
    pub total_detections: u64,
    /// Number of successful detections (above confidence threshold)
    pub successful_detections: u64,
    /// Average confidence score
    pub average_confidence: f64,
    /// Detection latency statistics
    pub avg_detection_time_ms: f64,
    /// Pattern-specific statistics
    pub pattern_counts: HashMap<String, u64>,
}

impl PatternStats {
    /// Creates new empty pattern statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a pattern detection
    pub fn record_detection(&mut self, pattern_name: &str, confidence: f64, detection_time_ms: f64) {
        self.total_detections += 1;
        
        if confidence >= 0.6 {
            self.successful_detections += 1;
        }

        // Update average confidence using incremental formula
        let n = self.total_detections as f64;
        self.average_confidence = ((n - 1.0) * self.average_confidence + confidence) / n;

        // Update average detection time
        self.avg_detection_time_ms = ((n - 1.0) * self.avg_detection_time_ms + detection_time_ms) / n;

        // Update pattern-specific counts
        *self.pattern_counts.entry(pattern_name.to_string()).or_insert(0) += 1;
    }

    /// Returns success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.total_detections == 0 {
            0.0
        } else {
            self.successful_detections as f64 / self.total_detections as f64
        }
    }

    /// Returns detection count for a specific pattern
    pub fn pattern_count(&self, pattern_name: &str) -> u64 {
        self.pattern_counts.get(pattern_name).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_pattern_result_creation() {
        let pattern_name = PatternName::new("hammer");
        let confidence = Confidence::new(0.85).unwrap();
        let price = Price::new(Decimal::new(15000, 2)).unwrap();

        let result = PatternResult::new(
            pattern_name.clone(),
            SignalType::Buy,
            confidence,
            price,
        );

        assert_eq!(result.pattern_name, pattern_name);
        assert_eq!(result.signal_type, SignalType::Buy);
        assert_eq!(result.confidence, confidence);
        assert_eq!(result.price_level, price);
        assert!(result.is_bullish());
        assert!(!result.is_bearish());
        assert!(result.confidence_above(0.8));
        assert!(!result.confidence_above(0.9));
    }

    #[test]
    fn test_pattern_context_builder() {
        let context = PatternContext::new(3)
            .with_volume_confirmation(VolumeConfirmation::Confirmed { volume_ratio: 1.8 })
            .with_trend_context(TrendContext::StrongUptrend { strength: 0.8 })
            .add_metadata("test_key", "test_value");

        assert_eq!(context.candles_used, 3);
        assert!(context.volume_confirmation.is_some());
        assert!(context.trend_context.is_some());
        assert_eq!(context.metadata.get("test_key"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_volume_confirmation() {
        let high_volume = VolumeConfirmation::from_ratio(2.0);
        assert!(high_volume.is_confirmed());
        assert_eq!(high_volume.ratio(), 2.0);

        let normal_volume = VolumeConfirmation::from_ratio(1.0);
        assert!(!normal_volume.is_confirmed());
        assert_eq!(normal_volume.ratio(), 1.0);

        let low_volume = VolumeConfirmation::from_ratio(0.5);
        assert!(!low_volume.is_confirmed());
        assert_eq!(low_volume.ratio(), 0.5);
    }

    #[test]
    fn test_trend_context() {
        let strong_up = TrendContext::from_strength(0.8);
        assert!(strong_up.is_bullish());
        assert!(!strong_up.is_bearish());
        assert!(!strong_up.is_sideways());
        assert_eq!(strong_up.strength(), 0.8);

        let sideways = TrendContext::from_strength(0.1);
        assert!(!sideways.is_bullish());
        assert!(!sideways.is_bearish());
        assert!(sideways.is_sideways());

        let strong_down = TrendContext::from_strength(-0.8);
        assert!(!strong_down.is_bullish());
        assert!(strong_down.is_bearish());
        assert!(!strong_down.is_sideways());
        assert_eq!(strong_down.strength(), 0.8);
    }

    #[test]
    fn test_pattern_params() {
        let params = PatternParams::new()
            .with_min_confidence(0.8)
            .require_volume_confirmation(true)
            .with_min_volume_ratio(1.5)
            .with_custom_param("body_ratio", 0.6);

        assert_eq!(params.min_confidence, 0.8);
        assert!(params.require_volume_confirmation);
        assert_eq!(params.min_volume_ratio, 1.5);
        assert_eq!(params.get_custom_param("body_ratio"), Some(0.6));
        assert_eq!(params.get_custom_param("nonexistent"), None);
    }

    #[test]
    fn test_pattern_stats() {
        let mut stats = PatternStats::new();
        
        stats.record_detection("hammer", 0.8, 1.5);
        stats.record_detection("doji", 0.5, 2.0);
        stats.record_detection("hammer", 0.9, 1.2);

        assert_eq!(stats.total_detections, 3);
        assert_eq!(stats.successful_detections, 2); // 0.8 and 0.9 are >= 0.6
        assert_eq!(stats.success_rate(), 2.0 / 3.0);
        assert_eq!(stats.pattern_count("hammer"), 2);
        assert_eq!(stats.pattern_count("doji"), 1);
        assert_eq!(stats.pattern_count("nonexistent"), 0);
    }
}