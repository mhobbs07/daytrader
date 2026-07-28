//! Common utilities for pattern detection algorithms
//!
//! This module provides shared functionality used across different pattern detectors
//! to reduce code duplication and ensure consistent behavior.

use crate::brokers::types::Confidence;
use crate::events::types::Candle;
use crate::patterns::types::{PatternContext, PatternError, TrendContext, VolumeConfirmation};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Shadow ratios for a candle - proportions of upper and lower shadows relative to total range
#[derive(Debug, Clone, Copy)]
pub struct ShadowRatios {
    /// Ratio of upper shadow to total candle range (0.0 to 1.0)
    pub upper: f64,
    /// Ratio of lower shadow to total candle range (0.0 to 1.0)
    pub lower: f64,
    /// Ratio of body size to total candle range (0.0 to 1.0)
    pub body: f64,
}

impl ShadowRatios {
    /// Calculate shadow ratios for a candle
    ///
    /// Returns None if the candle has zero range (perfect doji)
    pub fn from_candle(candle: &Candle) -> Option<Self> {
        let range = candle.range();
        if range == Decimal::ZERO {
            return None;
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        Some(Self {
            upper: decimal_to_ratio(upper_shadow, range),
            lower: decimal_to_ratio(lower_shadow, range),
            body: decimal_to_ratio(body_size, range),
        })
    }
}

/// Convert a Decimal value to a ratio (f64) relative to a total
///
/// This is a common operation used throughout pattern detection.
///
/// Converts via `ToPrimitive` rather than a decimal-string round-trip. This is
/// evaluated per candle per detector, so the former cost a heap allocation and
/// a float parse on the hottest path in the system. Ratios are shape
/// measurements, not money, so `f64` is the correct representation.
///
/// Returns `0.0` when `total` is zero.
pub fn decimal_to_ratio(value: Decimal, total: Decimal) -> f64 {
    if total == Decimal::ZERO {
        0.0
    } else {
        (value / total).to_f64().unwrap_or(0.0)
    }
}

/// Calculate volume confirmation ratio from a sequence of candles
///
/// Returns the ratio of the current candle's volume to the average volume
/// of the preceding candles. Used for volume confirmation in pattern detection.
pub fn calculate_volume_ratio(current_candle: &Candle, previous_candles: &[Candle]) -> f64 {
    if previous_candles.is_empty() {
        return 1.0; // No previous data, assume neutral volume
    }

    let current_volume = current_candle.volume as f64;
    let avg_previous_volume = previous_candles
        .iter()
        .map(|c| c.volume as f64)
        .sum::<f64>()
        / previous_candles.len() as f64;

    if avg_previous_volume == 0.0 {
        1.0 // Avoid division by zero
    } else {
        current_volume / avg_previous_volume
    }
}

/// Create volume confirmation from a volume ratio
pub fn create_volume_confirmation(volume_ratio: f64) -> VolumeConfirmation {
    VolumeConfirmation::from_ratio(volume_ratio)
}

/// Common confidence calculation components
#[derive(Debug, Clone)]
pub struct ConfidenceComponents {
    pub base_score: f64,
    pub volume_bonus: f64,
    pub trend_bonus: f64,
    pub pattern_specific_bonus: f64,
}

impl ConfidenceComponents {
    pub fn new(base_score: f64) -> Self {
        Self {
            base_score,
            volume_bonus: 0.0,
            trend_bonus: 0.0,
            pattern_specific_bonus: 0.0,
        }
    }

    pub fn with_volume_bonus(mut self, bonus: f64) -> Self {
        self.volume_bonus = bonus;
        self
    }

    pub fn with_trend_bonus(mut self, bonus: f64) -> Self {
        self.trend_bonus = bonus;
        self
    }

    pub fn with_pattern_bonus(mut self, bonus: f64) -> Self {
        self.pattern_specific_bonus = bonus;
        self
    }

    pub fn calculate_final_confidence(&self) -> Result<Confidence, PatternError> {
        let total_score =
            self.base_score + self.volume_bonus + self.trend_bonus + self.pattern_specific_bonus;
        let capped_score = total_score.min(100.0).max(0.0);

        Confidence::new(capped_score / 100.0).map_err(|e| PatternError::InvalidParameters {
            message: format!("Invalid confidence score: {}", e),
        })
    }
}

/// Apply volume confirmation bonus to confidence components
pub fn apply_volume_confirmation_bonus(
    components: ConfidenceComponents,
    context: &PatternContext,
) -> ConfidenceComponents {
    if let Some(volume_conf) = &context.volume_confirmation {
        let bonus = if volume_conf.is_confirmed() {
            10.0 // Strong volume confirmation
        } else if volume_conf.ratio() >= 1.0 {
            5.0 // Moderate volume confirmation
        } else {
            0.0 // No volume confirmation
        };
        components.with_volume_bonus(bonus)
    } else {
        components
    }
}

/// Apply trend context bonus to confidence components
pub fn apply_trend_context_bonus(
    components: ConfidenceComponents,
    context: &PatternContext,
    bullish_preference: bool,
    bearish_preference: bool,
) -> ConfidenceComponents {
    if let Some(trend) = &context.trend_context {
        let bonus = if (bullish_preference && trend.is_bullish())
            || (bearish_preference && trend.is_bearish())
        {
            15.0 // Strong trend alignment
        } else if trend.is_sideways() {
            5.0 // Neutral trend
        } else {
            0.0 // Counter-trend
        };
        components.with_trend_bonus(bonus)
    } else {
        components
    }
}

/// Common validation helpers for pattern detection

/// Validate that a candle has a valid range (not a perfect doji)
pub fn validate_candle_range(candle: &Candle) -> bool {
    candle.range() > Decimal::ZERO
}

/// Validate body size ratio constraints
pub fn validate_body_ratio(body_ratio: f64, min_ratio: f64, max_ratio: f64) -> bool {
    body_ratio >= min_ratio && body_ratio <= max_ratio
}

/// Validate shadow ratio constraints
pub fn validate_shadow_ratio(shadow_ratio: f64, min_ratio: f64, max_ratio: f64) -> bool {
    shadow_ratio >= min_ratio && shadow_ratio <= max_ratio
}

/// Calculate engulfing ratio between two candles
pub fn calculate_engulfing_ratio(first_body: Decimal, second_body: Decimal) -> f64 {
    if first_body == Decimal::ZERO {
        0.0
    } else {
        decimal_to_ratio(second_body, first_body)
    }
}

/// Validate engulfing criteria
pub fn validate_engulfing(
    first: &Candle,
    second: &Candle,
    min_ratio: f64,
    allow_gap: bool,
) -> bool {
    // First candle direction check will be pattern-specific
    let second_body = second.body_size();

    // Size validation
    let first_body = first.body_size();
    let size_ratio = calculate_engulfing_ratio(first_body, second_body);
    if size_ratio < min_ratio {
        return false;
    }

    // Engulfing validation (bodies must overlap completely)
    let first_high = first.open.value().max(first.close.value());
    let first_low = first.open.value().min(first.close.value());
    let second_high = second.open.value().max(second.close.value());
    let second_low = second.open.value().min(second.close.value());

    let engulfs = second_low <= first_low && second_high >= first_high;

    if !allow_gap {
        engulfs
    } else {
        // Allow gaps but still require significant overlap
        engulfs || (second_low <= first_high && second_high >= first_low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::Candle;
    use rust_decimal::Decimal;

    #[test]
    fn test_shadow_ratios_from_candle() {
        let candle = Candle {
            symbol: crate::brokers::types::Symbol::new("TEST").unwrap(),
            timestamp: chrono::Utc::now(),
            open: crate::brokers::types::Price::new(Decimal::new(100, 0)).unwrap(),
            high: crate::brokers::types::Price::new(Decimal::new(110, 0)).unwrap(),
            low: crate::brokers::types::Price::new(Decimal::new(90, 0)).unwrap(),
            close: crate::brokers::types::Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        };

        let ratios = ShadowRatios::from_candle(&candle).unwrap();
        assert!(ratios.upper > 0.0);
        assert!(ratios.lower > 0.0);
        assert!(ratios.body > 0.0);
        assert!((ratios.upper + ratios.lower + ratios.body - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_decimal_to_ratio() {
        assert_eq!(
            decimal_to_ratio(Decimal::new(5, 0), Decimal::new(10, 0)),
            0.5
        );
        assert_eq!(
            decimal_to_ratio(Decimal::new(0, 0), Decimal::new(10, 0)),
            0.0
        );
        assert_eq!(
            decimal_to_ratio(Decimal::new(10, 0), Decimal::new(0, 0)),
            0.0
        );
    }

    #[test]
    fn test_confidence_components() {
        let components = ConfidenceComponents::new(50.0)
            .with_volume_bonus(10.0)
            .with_trend_bonus(15.0)
            .with_pattern_bonus(5.0);

        let confidence = components.calculate_final_confidence().unwrap();
        assert_eq!(confidence.value(), 0.8); // (50 + 10 + 15 + 5) / 100 = 0.8
    }

    #[test]
    fn test_confidence_capping() {
        let components = ConfidenceComponents::new(200.0); // Way over 100
        let confidence = components.calculate_final_confidence().unwrap();
        assert_eq!(confidence.value(), 1.0); // Should be capped at 100%
    }
}
/// Calculate confidence for single-candle patterns (hammer, shooting star, doji)
pub fn calculate_single_candle_confidence(
    shadow_ratios: &ShadowRatios,
    context: &PatternContext,
    pattern_type: SingleCandlePatternType,
) -> Result<Confidence, PatternError> {
    let mut components = ConfidenceComponents::new(50.0); // Base 50%

    // Apply shadow ratio bonuses based on pattern type
    match pattern_type {
        SingleCandlePatternType::Hammer => {
            // Long lower shadow increases confidence
            if shadow_ratios.lower >= 0.6 {
                components = components.with_pattern_bonus(25.0);
            } else if shadow_ratios.lower >= 0.4 {
                components = components.with_pattern_bonus(15.0);
            }

            // Small upper shadow increases confidence
            if shadow_ratios.upper <= 0.1 {
                components = components.with_pattern_bonus(15.0);
            } else if shadow_ratios.upper <= 0.2 {
                components = components.with_pattern_bonus(10.0);
            }

            // Small to medium body increases confidence
            if shadow_ratios.body >= 0.1 && shadow_ratios.body <= 0.3 {
                components = components.with_pattern_bonus(10.0);
            }
        }
        SingleCandlePatternType::ShootingStar => {
            // Long upper shadow increases confidence
            if shadow_ratios.upper >= 0.6 {
                components = components.with_pattern_bonus(25.0);
            } else if shadow_ratios.upper >= 0.4 {
                components = components.with_pattern_bonus(15.0);
            }

            // Small lower shadow increases confidence
            if shadow_ratios.lower <= 0.1 {
                components = components.with_pattern_bonus(15.0);
            } else if shadow_ratios.lower <= 0.2 {
                components = components.with_pattern_bonus(10.0);
            }

            // Small to medium body increases confidence
            if shadow_ratios.body >= 0.1 && shadow_ratios.body <= 0.3 {
                components = components.with_pattern_bonus(10.0);
            }
        }
        SingleCandlePatternType::Doji => {
            // Smaller body increases confidence
            if shadow_ratios.body <= 0.05 {
                components = components.with_pattern_bonus(30.0);
            } else if shadow_ratios.body <= 0.1 {
                components = components.with_pattern_bonus(20.0);
            } else if shadow_ratios.body <= 0.15 {
                components = components.with_pattern_bonus(10.0);
            }
        }
    }

    // Apply common bonuses
    components = apply_volume_confirmation_bonus(components, context);
    components = apply_trend_context_bonus(
        components,
        context,
        pattern_type.is_bullish(),
        pattern_type.is_bearish(),
    );

    components.calculate_final_confidence()
}

/// Calculate confidence for engulfing patterns
pub fn calculate_engulfing_confidence(
    engulfing_ratio: f64,
    gap_ratio: Option<f64>,
    shadow_ratios: &ShadowRatios,
    context: &PatternContext,
) -> Result<Confidence, PatternError> {
    let mut components = ConfidenceComponents::new(60.0); // Base 60%

    // Engulfing ratio bonus
    if engulfing_ratio >= 2.0 {
        components = components.with_pattern_bonus(20.0);
    } else if engulfing_ratio >= 1.5 {
        components = components.with_pattern_bonus(15.0);
    } else if engulfing_ratio >= 1.2 {
        components = components.with_pattern_bonus(10.0);
    }

    // Gap analysis bonus
    if let Some(gap) = gap_ratio {
        if gap >= 0.02 {
            components = components.with_pattern_bonus(15.0); // 2%+ gap
        } else if gap >= 0.01 {
            components = components.with_pattern_bonus(10.0); // 1%+ gap
        }
    }

    // Body size relative to range bonus
    if shadow_ratios.body >= 0.7 {
        components = components.with_pattern_bonus(10.0); // Strong bodies
    }

    // Apply common bonuses
    components = apply_volume_confirmation_bonus(components, context);
    components = apply_trend_context_bonus(components, context, true, false); // Assume bullish engulfing for now

    components.calculate_final_confidence()
}

/// Types of single-candle patterns for confidence calculation
#[derive(Debug, Clone, Copy)]
pub enum SingleCandlePatternType {
    Hammer,
    ShootingStar,
    Doji,
}

impl SingleCandlePatternType {
    pub fn is_bullish(&self) -> bool {
        matches!(self, SingleCandlePatternType::Hammer)
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, SingleCandlePatternType::ShootingStar)
    }
}

/// Validate hammer pattern criteria
pub fn validate_hammer(candle: &Candle) -> bool {
    if !validate_candle_range(candle) {
        return false;
    }

    let shadow_ratios = match ShadowRatios::from_candle(candle) {
        Some(ratios) => ratios,
        None => return false,
    };

    // Lower shadow must be at least 2x the body size
    let body_size = candle.body_size();
    let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();

    let lower_shadow_valid = lower_shadow >= body_size * Decimal::new(2, 0);

    // Upper shadow should be small (less than body size)
    let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());
    let upper_shadow_valid = upper_shadow <= body_size;

    // Body should be small to medium (10-40% of range)
    let body_valid = validate_body_ratio(shadow_ratios.body, 0.1, 0.4);

    lower_shadow_valid && upper_shadow_valid && body_valid
}

/// Validate shooting star pattern criteria
pub fn validate_shooting_star(candle: &Candle) -> bool {
    if !validate_candle_range(candle) {
        return false;
    }

    let shadow_ratios = match ShadowRatios::from_candle(candle) {
        Some(ratios) => ratios,
        None => return false,
    };

    // Upper shadow must be at least 2x the body size
    let body_size = candle.body_size();
    let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

    let upper_shadow_valid = upper_shadow >= body_size * Decimal::new(2, 0);

    // Lower shadow should be small (less than body size)
    let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
    let lower_shadow_valid = lower_shadow <= body_size;

    // Body should be small to medium (10-40% of range)
    let body_valid = validate_body_ratio(shadow_ratios.body, 0.1, 0.4);

    upper_shadow_valid && lower_shadow_valid && body_valid
}

/// Validate doji pattern criteria
pub fn validate_doji(candle: &Candle) -> bool {
    if !validate_candle_range(candle) {
        return true; // Perfect doji (zero range) is valid
    }

    let shadow_ratios = match ShadowRatios::from_candle(candle) {
        Some(ratios) => ratios,
        None => return true, // Perfect doji
    };

    // Body should be very small (less than 15% of range)
    validate_body_ratio(shadow_ratios.body, 0.0, 0.15)
}

/// Validate bullish engulfing pattern criteria
pub fn validate_bullish_engulfing(first: &Candle, second: &Candle) -> bool {
    // First candle must be bearish
    if first.close.value() >= first.open.value() {
        return false;
    }

    // Second candle must be bullish
    if second.close.value() <= second.open.value() {
        return false;
    }

    // Second candle must engulf first candle's body
    let engulfing =
        second.open.value() <= first.close.value() && second.close.value() >= first.open.value();

    // Size validation: second candle should be significantly larger
    let first_body = first.body_size();
    let second_body = second.body_size();
    let size_ratio = calculate_engulfing_ratio(first_body, second_body);

    engulfing && size_ratio >= 1.2
}

/// Validate bearish engulfing pattern criteria
pub fn validate_bearish_engulfing(first: &Candle, second: &Candle) -> bool {
    // First candle must be bullish
    if first.close.value() <= first.open.value() {
        return false;
    }

    // Second candle must be bearish
    if second.close.value() >= second.open.value() {
        return false;
    }

    // Second candle must engulf first candle's body
    let engulfing =
        second.open.value() >= first.close.value() && second.close.value() <= first.open.value();

    // Size validation: second candle should be significantly larger
    let first_body = first.body_size();
    let second_body = second.body_size();
    let size_ratio = calculate_engulfing_ratio(first_body, second_body);

    engulfing && size_ratio >= 1.2
}

/// Calculate gap ratio between two candles (for engulfing patterns)
pub fn calculate_gap_ratio(first: &Candle, second: &Candle) -> Option<f64> {
    let first_close = first.close.value();
    let second_open = second.open.value();

    if first_close == Decimal::ZERO {
        return None;
    }

    let gap_size = if second_open > first_close {
        second_open - first_close
    } else {
        first_close - second_open
    };

    let gap_ratio = decimal_to_ratio(gap_size, first_close);
    Some(gap_ratio)
}

/// Validate minimum data requirements for pattern detection
pub fn validate_minimum_data<T>(data: &[T], required: usize) -> Result<(), PatternError> {
    if data.len() < required {
        return Err(PatternError::InsufficientData {
            required,
            available: data.len(),
        });
    }
    Ok(())
}

/// Validate confidence threshold
pub fn validate_confidence_threshold(confidence: f64, min_threshold: f64) -> bool {
    confidence >= min_threshold
}
