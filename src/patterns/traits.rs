//! Pattern detection traits and interfaces
//!
//! This module defines the core traits that all pattern detectors must implement.
//! The trait system enables a functional, composable approach to pattern detection
//! while maintaining type safety and performance.

use crate::events::types::Candle;
use crate::patterns::types::{PatternError, PatternParams, PatternResult, PatternStats};
use async_trait::async_trait;
use std::fmt::Debug;

/// Core trait for pattern detection algorithms
///
/// All pattern detectors implement this trait, providing a consistent interface
/// for pattern detection across different algorithms. The trait is designed for
/// functional programming with pure functions and immutable data.
///
/// # Design Principles
///
/// - **Pure Functions**: Detection methods are pure - same input always produces same output
/// - **Immutable Data**: All inputs are immutable references
/// - **Error Handling**: Comprehensive error handling with typed errors
/// - **Performance**: Designed for high-frequency real-time processing
///
/// # Examples
///
/// ```
/// use daytrader::patterns::traits::PatternDetector;
/// use daytrader::patterns::types::{PatternParams, PatternResult};
/// use daytrader::events::types::Candle;
///
/// struct HammerDetector;
///
/// impl PatternDetector for HammerDetector {
///     fn name(&self) -> &'static str {
///         "hammer"
///     }
///
///     fn min_candles_required(&self) -> usize {
///         1
///     }
///
///     fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
///         // Implementation here
///         Ok(None)
///     }
/// }
/// ```
pub trait PatternDetector: Send + Sync + Debug {
    /// Returns the unique name of this pattern
    ///
    /// This name is used for configuration, logging, and pattern identification.
    /// Must be unique across all pattern detectors.
    fn name(&self) -> &'static str;

    /// Returns the minimum number of candles required for detection
    ///
    /// This is used for validation and optimization. The detection method
    /// will only be called if sufficient candles are available.
    fn min_candles_required(&self) -> usize;

    /// Detects the pattern in the given candle data
    ///
    /// This is the core detection method. It should be a pure function that
    /// analyzes the candle data and returns a pattern result if detected.
    ///
    /// # Arguments
    ///
    /// * `candles` - Slice of candles to analyze (newest first)
    /// * `params` - Pattern detection parameters
    ///
    /// # Returns
    ///
    /// * `Ok(Some(result))` - Pattern detected with confidence and context
    /// * `Ok(None)` - Pattern not detected or confidence below threshold
    /// * `Err(error)` - Detection failed due to data or configuration issues
    ///
    /// # Performance Requirements
    ///
    /// - Must complete in < 1ms for typical candle sequences
    /// - Should not allocate memory in hot paths
    /// - Must be thread-safe for parallel processing
    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError>;

    /// Returns a description of this pattern for documentation
    ///
    /// Optional method that provides human-readable description of the pattern.
    /// Used for logging, debugging, and user interfaces.
    fn description(&self) -> &'static str {
        "No description available"
    }

    /// Returns the pattern category for grouping and filtering
    ///
    /// Patterns can be categorized for easier management and configuration.
    fn category(&self) -> PatternCategory {
        PatternCategory::Candlestick
    }

    /// Validates pattern parameters for this detector
    ///
    /// Optional method to validate pattern-specific parameters.
    /// Called during configuration loading to catch errors early.
    fn validate_params(&self, params: &PatternParams) -> Result<(), PatternError> {
        if params.min_confidence < 0.0 || params.min_confidence > 1.0 {
            return Err(PatternError::InvalidParameters {
                message: "min_confidence must be between 0.0 and 1.0".to_string(),
            });
        }
        Ok(())
    }

    /// Returns default parameters for this pattern
    ///
    /// Provides sensible defaults that can be overridden by configuration.
    fn default_params(&self) -> PatternParams {
        PatternParams::default()
    }

    /// Pre-processes candles before detection (optional)
    ///
    /// Some patterns may need to pre-process or validate candle data.
    /// This method is called before `detect()` and can be used for
    /// data validation or preparation.
    fn preprocess_candles(&self, candles: &[Candle]) -> Result<(), PatternError> {
        // Validate basic candle data quality
        for (i, candle) in candles.iter().enumerate() {
            if candle.high < candle.low {
                return Err(PatternError::DataQuality {
                    issue: format!("Candle {} has high < low", i),
                });
            }
            if candle.high < candle.open || candle.high < candle.close {
                return Err(PatternError::DataQuality {
                    issue: format!("Candle {} has high < open/close", i),
                });
            }
            if candle.low > candle.open || candle.low > candle.close {
                return Err(PatternError::DataQuality {
                    issue: format!("Candle {} has low > open/close", i),
                });
            }
        }
        Ok(())
    }
}

/// Async trait for pattern detectors that need async operations
///
/// Some pattern detectors may need to perform async operations like
/// database lookups or external API calls. This trait provides async
/// versions of the core detection methods.
#[async_trait]
pub trait AsyncPatternDetector: PatternDetector {
    /// Async version of pattern detection
    ///
    /// Same semantics as `detect()` but allows for async operations.
    async fn detect_async(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        // Default implementation delegates to sync version
        self.detect(candles, params)
    }

    /// Async initialization method
    ///
    /// Called once during pattern detector setup. Can be used to
    /// initialize async resources like database connections.
    async fn initialize(&self) -> Result<(), PatternError> {
        Ok(())
    }

    /// Async cleanup method
    ///
    /// Called during shutdown to clean up async resources.
    async fn cleanup(&self) -> Result<(), PatternError> {
        Ok(())
    }
}

/// Pattern categories for organization and filtering
///
/// Patterns are grouped into categories to make configuration and
/// management easier. Categories can be enabled/disabled as groups.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PatternCategory {
    /// Single or multi-candle candlestick patterns
    Candlestick,
    /// Technical indicator patterns
    Technical,
    /// Volume-based patterns
    Volume,
    /// Price action patterns
    PriceAction,
    /// Custom or experimental patterns
    Custom,
}

impl PatternCategory {
    /// Returns all available categories
    pub fn all() -> Vec<Self> {
        vec![
            Self::Candlestick,
            Self::Technical,
            Self::Volume,
            Self::PriceAction,
            Self::Custom,
        ]
    }

    /// Returns the category name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candlestick => "candlestick",
            Self::Technical => "technical",
            Self::Volume => "volume",
            Self::PriceAction => "price_action",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for PatternCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Trait for pattern detector factories
///
/// Enables dynamic creation of pattern detectors from configuration.
/// Used by the pattern registry for flexible pattern management.
pub trait PatternDetectorFactory: Send + Sync + Debug {
    /// Creates a new pattern detector instance
    fn create(&self) -> Box<dyn PatternDetector>;

    /// Returns the name of patterns this factory creates
    fn pattern_name(&self) -> &'static str;

    /// Returns the category of patterns this factory creates
    fn pattern_category(&self) -> PatternCategory;
}

/// Trait for pattern detector with performance monitoring
///
/// Extends the basic pattern detector with performance tracking capabilities.
/// Used for monitoring and optimization of pattern detection performance.
pub trait MonitoredPatternDetector: PatternDetector {
    /// Returns performance statistics for this detector
    fn stats(&self) -> &PatternStats;

    /// Returns mutable reference to statistics (for internal use)
    fn stats_mut(&mut self) -> &mut PatternStats;

    /// Resets performance statistics
    fn reset_stats(&mut self) {
        *self.stats_mut() = PatternStats::new();
    }

    /// Records a detection attempt with timing
    fn record_detection(&mut self, pattern_name: &str, confidence: f64, detection_time_ms: f64) {
        self.stats_mut()
            .record_detection(pattern_name, confidence, detection_time_ms);
    }
}

/// Trait for configurable pattern detectors
///
/// Allows pattern detectors to be configured at runtime with custom parameters.
/// Useful for A/B testing and parameter optimization.
pub trait ConfigurablePatternDetector: PatternDetector {
    /// Updates the detector configuration
    fn update_config(&mut self, params: PatternParams) -> Result<(), PatternError>;

    /// Returns the current configuration
    fn current_config(&self) -> &PatternParams;

    /// Validates a configuration before applying it
    fn validate_config(&self, params: &PatternParams) -> Result<(), PatternError> {
        self.validate_params(params)
    }
}

/// Utility functions for pattern detection
pub mod utils {
    use super::*;
    use crate::events::types::Candle;
    use rust_decimal::Decimal;

    /// Calculates the body size ratio for a candle
    ///
    /// Returns the ratio of body size to total range (0.0 to 1.0).
    /// Used by many candlestick pattern detectors.
    pub fn body_ratio(candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }

        let body_size = candle.body_size();
        (body_size / range).to_string().parse().unwrap_or(0.0)
    }

    /// Calculates the upper shadow ratio for a candle
    ///
    /// Returns the ratio of upper shadow to total range (0.0 to 1.0).
    pub fn upper_shadow_ratio(candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }

        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());
        (upper_shadow / range).to_string().parse().unwrap_or(0.0)
    }

    /// Calculates the lower shadow ratio for a candle
    ///
    /// Returns the ratio of lower shadow to total range (0.0 to 1.0).
    pub fn lower_shadow_ratio(candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }

        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        (lower_shadow / range).to_string().parse().unwrap_or(0.0)
    }

    /// Checks if a candle is a doji (small body relative to range)
    ///
    /// A doji is characterized by a very small body compared to the total range.
    pub fn is_doji(candle: &Candle, threshold: f64) -> bool {
        body_ratio(candle) <= threshold
    }

    /// Calculates average volume over a period
    ///
    /// Used for volume confirmation in pattern detection.
    pub fn average_volume(candles: &[Candle]) -> f64 {
        if candles.is_empty() {
            return 0.0;
        }

        let total_volume: u64 = candles.iter().map(|c| c.volume).sum();
        total_volume as f64 / candles.len() as f64
    }

    /// Calculates volume ratio for the most recent candle
    ///
    /// Compares the most recent candle's volume to the average of previous candles.
    pub fn volume_ratio(candles: &[Candle]) -> f64 {
        if candles.len() < 2 {
            return 1.0;
        }

        let current_volume = candles[0].volume as f64;
        let avg_volume = average_volume(&candles[1..]);

        if avg_volume == 0.0 {
            1.0
        } else {
            current_volume / avg_volume
        }
    }

    /// Validates that candles are in reverse chronological order
    ///
    /// Pattern detection expects candles to be ordered newest first.
    pub fn validate_candle_order(candles: &[Candle]) -> Result<(), PatternError> {
        for i in 1..candles.len() {
            if candles[i].timestamp > candles[i - 1].timestamp {
                return Err(PatternError::DataQuality {
                    issue: "Candles must be in reverse chronological order (newest first)"
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    /// Ensures minimum number of candles are available
    ///
    /// Helper function for pattern detectors to validate input data.
    pub fn ensure_min_candles(candles: &[Candle], required: usize) -> Result<(), PatternError> {
        if candles.len() < required {
            return Err(PatternError::InsufficientData {
                required,
                available: candles.len(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Price, Symbol};
    use crate::events::types::Candle;
    use chrono::Utc;
    use rust_decimal::Decimal;

    // Mock pattern detector for testing
    #[derive(Debug)]
    struct MockPatternDetector {
        name: &'static str,
        min_candles: usize,
    }

    impl PatternDetector for MockPatternDetector {
        fn name(&self) -> &'static str {
            self.name
        }

        fn min_candles_required(&self) -> usize {
            self.min_candles
        }

        fn detect(
            &self,
            candles: &[Candle],
            _params: &PatternParams,
        ) -> Result<Option<PatternResult>, PatternError> {
            if candles.len() < self.min_candles {
                return Err(PatternError::InsufficientData {
                    required: self.min_candles,
                    available: candles.len(),
                });
            }
            Ok(None)
        }
    }

    fn create_test_candle(open: i64, high: i64, low: i64, close: i64, volume: u64) -> Candle {
        Candle {
            symbol: Symbol::new("TEST").unwrap(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(open, 2)).unwrap(),
            high: Price::new(Decimal::new(high, 2)).unwrap(),
            low: Price::new(Decimal::new(low, 2)).unwrap(),
            close: Price::new(Decimal::new(close, 2)).unwrap(),
            volume,
        }
    }

    #[test]
    fn test_pattern_detector_trait() {
        let detector = MockPatternDetector {
            name: "test_pattern",
            min_candles: 2,
        };

        assert_eq!(detector.name(), "test_pattern");
        assert_eq!(detector.min_candles_required(), 2);
        assert_eq!(detector.category(), PatternCategory::Candlestick);
        assert!(detector.validate_params(&PatternParams::default()).is_ok());
    }

    #[test]
    fn test_pattern_category() {
        let categories = PatternCategory::all();
        assert_eq!(categories.len(), 5);
        assert!(categories.contains(&PatternCategory::Candlestick));
        assert!(categories.contains(&PatternCategory::Technical));

        assert_eq!(PatternCategory::Candlestick.as_str(), "candlestick");
        assert_eq!(PatternCategory::Technical.as_str(), "technical");
    }

    #[test]
    fn test_utils_body_ratio() {
        let candle = create_test_candle(10000, 11000, 9000, 10500, 1000);
        let ratio = utils::body_ratio(&candle);
        assert!((ratio - 0.25).abs() < 0.01); // Body is 500, range is 2000, so ratio is 0.25
    }

    #[test]
    fn test_utils_shadow_ratios() {
        let candle = create_test_candle(10000, 11000, 9000, 10500, 1000);

        let upper_ratio = utils::upper_shadow_ratio(&candle);
        let lower_ratio = utils::lower_shadow_ratio(&candle);

        // Upper shadow: 11000 - 10500 = 500, range = 2000, ratio = 0.25
        assert!((upper_ratio - 0.25).abs() < 0.01);
        // Lower shadow: 10000 - 9000 = 1000, range = 2000, ratio = 0.5
        assert!((lower_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_utils_is_doji() {
        let doji_candle = create_test_candle(10000, 11000, 9000, 10050, 1000);
        let normal_candle = create_test_candle(10000, 11000, 9000, 10500, 1000);

        assert!(utils::is_doji(&doji_candle, 0.1)); // Small body
        assert!(!utils::is_doji(&normal_candle, 0.1)); // Larger body
    }

    #[test]
    fn test_utils_volume_calculations() {
        let candles = vec![
            create_test_candle(10000, 11000, 9000, 10500, 2000), // Current
            create_test_candle(10000, 11000, 9000, 10500, 1000), // Previous
            create_test_candle(10000, 11000, 9000, 10500, 1000), // Previous
        ];

        let avg_volume = utils::average_volume(&candles);
        assert_eq!(avg_volume, 1333.3333333333333); // (2000 + 1000 + 1000) / 3

        let volume_ratio = utils::volume_ratio(&candles);
        assert_eq!(volume_ratio, 2.0); // 2000 / 1000
    }

    #[test]
    fn test_utils_validation() {
        // Ensure distinct, strictly-ascending timestamps (Utc::now() can return the same instant twice)
        // validate_candle_order expects reverse-chronological (newest first)
        let base = Utc::now();
        let mut c_newer = create_test_candle(10000, 11000, 9000, 10500, 1000);
        let mut c_older = create_test_candle(10000, 11000, 9000, 10500, 1000);
        c_newer.timestamp = base;
        c_older.timestamp = base - chrono::Duration::seconds(1);
        let candles = vec![c_newer, c_older];

        assert!(utils::validate_candle_order(&candles).is_ok());
        assert!(utils::ensure_min_candles(&candles, 2).is_ok());
        assert!(utils::ensure_min_candles(&candles, 3).is_err());
    }
}
