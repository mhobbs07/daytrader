//! Multi-candle pattern detection algorithms
//!
//! This module implements complex multi-candle patterns that require analysis of 3 or more candles:
//! - Morning Star: 3-candle bullish reversal pattern (bearish, small body/doji, bullish)
//! - Evening Star: 3-candle bearish reversal pattern (bullish, small body/doji, bearish)
//! - Three White Soldiers: 3-candle bullish continuation pattern (three consecutive bullish candles)
//! - Three Black Crows: 3-candle bearish continuation pattern (three consecutive bearish candles)
//!
//! All patterns use mathematical validation with confidence scoring based on pattern metrics,
//! market context, and volume confirmation following functional programming principles.

use crate::brokers::types::{Confidence, PatternName, Price};
use crate::events::types::{Candle, SignalType};
use crate::patterns::traits::PatternDetector;
use crate::patterns::types::{
    PatternContext, PatternError, PatternParams, PatternResult, TrendContext, VolumeConfirmation,
};
use crate::patterns::utils::decimal_to_ratio;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::fmt::Debug;

/// Morning Star pattern detector - 3-candle bullish reversal pattern
///
/// A morning star is characterized by:
/// - First candle: Bearish (red) candle in a downtrend
/// - Second candle: Small body or doji (indecision)
/// - Third candle: Bullish (green) candle that closes above the midpoint of the first candle
/// - Gap down after first candle and gap up before third candle (optional but increases confidence)
///
/// The pattern indicates potential bullish reversal as selling pressure diminishes and buying resumes.
#[derive(Debug, Clone)]
pub struct MorningStarDetector;

impl MorningStarDetector {
    /// Creates a new morning star detector
    pub fn new() -> Self {
        Self
    }

    /// Validates morning star pattern criteria
    fn validate_morning_star(&self, first: &Candle, second: &Candle, third: &Candle) -> bool {
        // First candle must be bearish with substantial body
        let first_bearish = first.close.value() < first.open.value();
        let first_body_ratio = self.body_ratio(first);
        let first_substantial = first_body_ratio >= 0.3; // At least 30% of range

        // Second candle must have small body (doji-like)
        let second_body_ratio = self.body_ratio(second);
        let second_small = second_body_ratio <= 0.3; // Small body

        // Third candle must be bullish
        let third_bullish = third.close.value() > third.open.value();

        // Third candle should close above the midpoint of the first candle
        let first_midpoint = (first.open.value() + first.close.value()) / Decimal::new(2, 0);
        let third_closes_high = third.close.value() > first_midpoint;

        // Second candle should be lower than both first and third (star position)
        let second_is_star = second.high.value() < first.close.value().max(third.open.value());

        first_bearish
            && first_substantial
            && second_small
            && third_bullish
            && third_closes_high
            && second_is_star
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(
        &self,
        first: &Candle,
        second: &Candle,
        third: &Candle,
        context: &PatternContext,
    ) -> Result<Confidence, PatternError> {
        let mut confidence_score: f64 = 55.0; // Base 55%

        // Gap analysis - gaps increase confidence significantly
        let gap_down = second.high.value() < first.close.value();
        let gap_up = third.open.value() > second.high.value();

        if gap_down && gap_up {
            confidence_score += 25.0; // Both gaps present
        } else if gap_down || gap_up {
            confidence_score += 15.0; // One gap present
        }

        // Body size analysis
        let first_body_ratio = self.body_ratio(first);
        let second_body_ratio = self.body_ratio(second);
        let third_body_ratio = self.body_ratio(third);

        // Strong first candle (large bearish body)
        if first_body_ratio >= 0.6 {
            confidence_score += 15.0;
        } else if first_body_ratio >= 0.4 {
            confidence_score += 10.0;
        }

        // Very small second candle (doji-like)
        if second_body_ratio <= 0.1 {
            confidence_score += 15.0;
        } else if second_body_ratio <= 0.2 {
            confidence_score += 10.0;
        }

        // Strong third candle (large bullish body)
        if third_body_ratio >= 0.6 {
            confidence_score += 15.0;
        } else if third_body_ratio >= 0.4 {
            confidence_score += 10.0;
        }

        // Third candle penetration into first candle
        let first_midpoint = (first.open.value() + first.close.value()) / Decimal::new(2, 0);
        let penetration_ratio = if first.open.value() > first.close.value() {
            ((third.close.value() - first.close.value())
                / (first.open.value() - first.close.value()))
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0)
        } else {
            0.0
        };

        if penetration_ratio >= 0.7 {
            confidence_score += 15.0; // Deep penetration
        } else if penetration_ratio >= 0.5 {
            confidence_score += 10.0; // Moderate penetration
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (more reliable after downtrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bearish() {
                confidence_score += 15.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0).map_err(|e| PatternError::InvalidParameters {
            message: e.to_string(),
        })
    }

    /// Helper function to calculate body ratio
    fn body_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let body_size = candle.body_size();
        decimal_to_ratio(body_size, range)
    }
}

impl PatternDetector for MorningStarDetector {
    fn name(&self) -> &'static str {
        "morning_star"
    }

    fn min_candles_required(&self) -> usize {
        3
    }

    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Err(PatternError::InsufficientData {
                required: 3,
                available: candles.len(),
            });
        }

        let first = &candles[2]; // Oldest candle
        let second = &candles[1]; // Middle candle
        let third = &candles[0]; // Newest candle

        if !self.validate_morning_star(first, second, third) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(3);

        // Add volume confirmation based on third candle
        if candles.len() > 3 {
            let current_volume = third.volume as f64;
            let avg_volume = candles[3..].iter().map(|c| c.volume as f64).sum::<f64>()
                / (candles.len() - 3) as f64;
            let volume_ratio = if avg_volume > 0.0 {
                current_volume / avg_volume
            } else {
                1.0
            };
            context =
                context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let gap_down = second.high.value() < first.close.value();
        let gap_up = third.open.value() > second.high.value();
        context = context.add_metadata("has_gap_down", &gap_down.to_string());
        context = context.add_metadata("has_gap_up", &gap_up.to_string());

        let confidence = self.calculate_confidence(first, second, third, &context)?;

        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("morning_star"),
            SignalType::Buy,
            confidence,
            third.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Evening Star pattern detector - 3-candle bearish reversal pattern
///
/// An evening star is characterized by:
/// - First candle: Bullish (green) candle in an uptrend
/// - Second candle: Small body or doji (indecision)
/// - Third candle: Bearish (red) candle that closes below the midpoint of the first candle
/// - Gap up after first candle and gap down before third candle (optional but increases confidence)
///
/// The pattern indicates potential bearish reversal as buying pressure diminishes and selling resumes.
#[derive(Debug, Clone)]
pub struct EveningStarDetector;

impl EveningStarDetector {
    /// Creates a new evening star detector
    pub fn new() -> Self {
        Self
    }

    /// Validates evening star pattern criteria
    fn validate_evening_star(&self, first: &Candle, second: &Candle, third: &Candle) -> bool {
        // First candle must be bullish with substantial body
        let first_bullish = first.close.value() > first.open.value();
        let first_body_ratio = self.body_ratio(first);
        let first_substantial = first_body_ratio >= 0.3; // At least 30% of range

        // Second candle must have small body (doji-like)
        let second_body_ratio = self.body_ratio(second);
        let second_small = second_body_ratio <= 0.3; // Small body

        // Third candle must be bearish
        let third_bearish = third.close.value() < third.open.value();

        // Third candle should close below the midpoint of the first candle
        let first_midpoint = (first.open.value() + first.close.value()) / Decimal::new(2, 0);
        let third_closes_low = third.close.value() < first_midpoint;

        // Second candle should be higher than both first and third (star position)
        let second_is_star = second.low.value() > first.close.value().min(third.open.value());

        first_bullish
            && first_substantial
            && second_small
            && third_bearish
            && third_closes_low
            && second_is_star
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(
        &self,
        first: &Candle,
        second: &Candle,
        third: &Candle,
        context: &PatternContext,
    ) -> Result<Confidence, PatternError> {
        let mut confidence_score: f64 = 55.0; // Base 55%

        // Gap analysis - gaps increase confidence significantly
        let gap_up = second.low.value() > first.close.value();
        let gap_down = third.open.value() < second.low.value();

        if gap_up && gap_down {
            confidence_score += 25.0; // Both gaps present
        } else if gap_up || gap_down {
            confidence_score += 15.0; // One gap present
        }

        // Body size analysis
        let first_body_ratio = self.body_ratio(first);
        let second_body_ratio = self.body_ratio(second);
        let third_body_ratio = self.body_ratio(third);

        // Strong first candle (large bullish body)
        if first_body_ratio >= 0.6 {
            confidence_score += 15.0;
        } else if first_body_ratio >= 0.4 {
            confidence_score += 10.0;
        }

        // Very small second candle (doji-like)
        if second_body_ratio <= 0.1 {
            confidence_score += 15.0;
        } else if second_body_ratio <= 0.2 {
            confidence_score += 10.0;
        }

        // Strong third candle (large bearish body)
        if third_body_ratio >= 0.6 {
            confidence_score += 15.0;
        } else if third_body_ratio >= 0.4 {
            confidence_score += 10.0;
        }

        // Third candle penetration into first candle
        let penetration_ratio = if first.close.value() > first.open.value() {
            ((first.close.value() - third.close.value())
                / (first.close.value() - first.open.value()))
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0)
        } else {
            0.0
        };

        if penetration_ratio >= 0.7 {
            confidence_score += 15.0; // Deep penetration
        } else if penetration_ratio >= 0.5 {
            confidence_score += 10.0; // Moderate penetration
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (more reliable after uptrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bullish() {
                confidence_score += 15.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0).map_err(|e| PatternError::InvalidParameters {
            message: e.to_string(),
        })
    }

    /// Helper function to calculate body ratio
    fn body_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let body_size = candle.body_size();
        decimal_to_ratio(body_size, range)
    }
}

impl PatternDetector for EveningStarDetector {
    fn name(&self) -> &'static str {
        "evening_star"
    }

    fn min_candles_required(&self) -> usize {
        3
    }

    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Err(PatternError::InsufficientData {
                required: 3,
                available: candles.len(),
            });
        }

        let first = &candles[2]; // Oldest candle
        let second = &candles[1]; // Middle candle
        let third = &candles[0]; // Newest candle

        if !self.validate_evening_star(first, second, third) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(3);

        // Add volume confirmation based on third candle
        if candles.len() > 3 {
            let current_volume = third.volume as f64;
            let avg_volume = candles[3..].iter().map(|c| c.volume as f64).sum::<f64>()
                / (candles.len() - 3) as f64;
            let volume_ratio = if avg_volume > 0.0 {
                current_volume / avg_volume
            } else {
                1.0
            };
            context =
                context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let gap_up = second.low.value() > first.close.value();
        let gap_down = third.open.value() < second.low.value();
        context = context.add_metadata("has_gap_up", &gap_up.to_string());
        context = context.add_metadata("has_gap_down", &gap_down.to_string());

        let confidence = self.calculate_confidence(first, second, third, &context)?;

        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("evening_star"),
            SignalType::Sell,
            confidence,
            third.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Three White Soldiers pattern detector - 3-candle bullish continuation pattern
///
/// Three White Soldiers is characterized by:
/// - Three consecutive bullish (green) candles
/// - Each candle opens within the previous candle's body
/// - Each candle closes higher than the previous candle's close
/// - Bodies should be substantial (not dojis)
/// - Minimal upper shadows (strong buying pressure)
///
/// The pattern indicates strong bullish momentum and continuation of uptrend.
#[derive(Debug, Clone)]
pub struct ThreeWhiteSoldiersDetector;

impl ThreeWhiteSoldiersDetector {
    /// Creates a new three white soldiers detector
    pub fn new() -> Self {
        Self
    }

    /// Validates three white soldiers pattern criteria
    fn validate_three_white_soldiers(
        &self,
        first: &Candle,
        second: &Candle,
        third: &Candle,
    ) -> bool {
        // All three candles must be bullish
        let all_bullish = first.close.value() > first.open.value()
            && second.close.value() > second.open.value()
            && third.close.value() > third.open.value();

        // Each candle should close higher than the previous
        let ascending_closes = second.close.value() > first.close.value()
            && third.close.value() > second.close.value();

        // Each candle should open within the previous candle's body
        let opens_within_body = second.open.value() > first.open.value()
            && second.open.value() < first.close.value()
            && third.open.value() > second.open.value()
            && third.open.value() < second.close.value();

        // Bodies should be substantial (at least 30% of range)
        let substantial_bodies = self.body_ratio(first) >= 0.3
            && self.body_ratio(second) >= 0.3
            && self.body_ratio(third) >= 0.3;

        // Upper shadows should be small (less than 25% of range)
        let small_upper_shadows = self.upper_shadow_ratio(first) <= 0.25
            && self.upper_shadow_ratio(second) <= 0.25
            && self.upper_shadow_ratio(third) <= 0.25;

        all_bullish
            && ascending_closes
            && opens_within_body
            && substantial_bodies
            && small_upper_shadows
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(
        &self,
        first: &Candle,
        second: &Candle,
        third: &Candle,
        context: &PatternContext,
    ) -> Result<Confidence, PatternError> {
        let mut confidence_score: f64 = 60.0; // Base 60%

        // Body size consistency - similar sized bodies increase confidence
        let body_ratios = [
            self.body_ratio(first),
            self.body_ratio(second),
            self.body_ratio(third),
        ];
        let avg_body_ratio = body_ratios.iter().sum::<f64>() / 3.0;
        let body_consistency = body_ratios
            .iter()
            .map(|&ratio| (ratio - avg_body_ratio).abs())
            .sum::<f64>()
            / 3.0;

        if body_consistency <= 0.1 {
            confidence_score += 15.0; // Very consistent bodies
        } else if body_consistency <= 0.2 {
            confidence_score += 10.0; // Moderately consistent bodies
        }

        // Strong bodies (large relative to range)
        if avg_body_ratio >= 0.7 {
            confidence_score += 15.0;
        } else if avg_body_ratio >= 0.5 {
            confidence_score += 10.0;
        }

        // Minimal upper shadows
        let avg_upper_shadow = [
            self.upper_shadow_ratio(first),
            self.upper_shadow_ratio(second),
            self.upper_shadow_ratio(third),
        ]
        .iter()
        .sum::<f64>()
            / 3.0;

        if avg_upper_shadow <= 0.1 {
            confidence_score += 15.0; // Very small shadows
        } else if avg_upper_shadow <= 0.2 {
            confidence_score += 10.0; // Small shadows
        }

        // Progressive price advancement
        let first_advance = (first.close.value() - first.open.value()) / first.open.value();
        let second_advance = (second.close.value() - second.open.value()) / second.open.value();
        let third_advance = (third.close.value() - third.open.value()) / third.open.value();

        let advances = [
            first_advance.to_f64().unwrap_or(0.0),
            second_advance.to_f64().unwrap_or(0.0),
            third_advance.to_f64().unwrap_or(0.0),
        ];

        let avg_advance = advances.iter().sum::<f64>() / 3.0;
        if avg_advance >= 0.03 {
            confidence_score += 15.0; // Strong advances (3%+)
        } else if avg_advance >= 0.02 {
            confidence_score += 10.0; // Good advances (2%+)
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (more reliable in uptrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bullish() {
                confidence_score += 10.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0).map_err(|e| PatternError::InvalidParameters {
            message: e.to_string(),
        })
    }

    /// Helper function to calculate body ratio
    fn body_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let body_size = candle.body_size();
        decimal_to_ratio(body_size, range)
    }

    /// Helper function to calculate upper shadow ratio
    fn upper_shadow_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());
        decimal_to_ratio(upper_shadow, range)
    }
}

impl PatternDetector for ThreeWhiteSoldiersDetector {
    fn name(&self) -> &'static str {
        "three_white_soldiers"
    }

    fn min_candles_required(&self) -> usize {
        3
    }

    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Err(PatternError::InsufficientData {
                required: 3,
                available: candles.len(),
            });
        }

        let first = &candles[2]; // Oldest candle
        let second = &candles[1]; // Middle candle
        let third = &candles[0]; // Newest candle

        if !self.validate_three_white_soldiers(first, second, third) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(3);

        // Add volume confirmation based on third candle
        if candles.len() > 3 {
            let current_volume = third.volume as f64;
            let avg_volume = candles[3..].iter().map(|c| c.volume as f64).sum::<f64>()
                / (candles.len() - 3) as f64;
            let volume_ratio = if avg_volume > 0.0 {
                current_volume / avg_volume
            } else {
                1.0
            };
            context =
                context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let total_advance = ((third.close.value() - first.open.value()) / first.open.value())
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);
        context = context.add_metadata("total_advance", &format!("{:.4}", total_advance));

        let confidence = self.calculate_confidence(first, second, third, &context)?;

        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("three_white_soldiers"),
            SignalType::Buy,
            confidence,
            third.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Three Black Crows pattern detector - 3-candle bearish continuation pattern
///
/// Three Black Crows is characterized by:
/// - Three consecutive bearish (red) candles
/// - Each candle opens within the previous candle's body
/// - Each candle closes lower than the previous candle's close
/// - Bodies should be substantial (not dojis)
/// - Minimal lower shadows (strong selling pressure)
///
/// The pattern indicates strong bearish momentum and continuation of downtrend.
#[derive(Debug, Clone)]
pub struct ThreeBlackCrowsDetector;

impl ThreeBlackCrowsDetector {
    /// Creates a new three black crows detector
    pub fn new() -> Self {
        Self
    }

    /// Validates three black crows pattern criteria
    fn validate_three_black_crows(&self, first: &Candle, second: &Candle, third: &Candle) -> bool {
        // All three candles must be bearish
        let all_bearish = first.close.value() < first.open.value()
            && second.close.value() < second.open.value()
            && third.close.value() < third.open.value();

        // Each candle should close lower than the previous
        let descending_closes = second.close.value() < first.close.value()
            && third.close.value() < second.close.value();

        // Each candle should open within the previous candle's body
        let opens_within_body = second.open.value() < first.open.value()
            && second.open.value() > first.close.value()
            && third.open.value() < second.open.value()
            && third.open.value() > second.close.value();

        // Bodies should be substantial (at least 30% of range)
        let substantial_bodies = self.body_ratio(first) >= 0.3
            && self.body_ratio(second) >= 0.3
            && self.body_ratio(third) >= 0.3;

        // Lower shadows should be small (less than 25% of range)
        let small_lower_shadows = self.lower_shadow_ratio(first) <= 0.25
            && self.lower_shadow_ratio(second) <= 0.25
            && self.lower_shadow_ratio(third) <= 0.25;

        all_bearish
            && descending_closes
            && opens_within_body
            && substantial_bodies
            && small_lower_shadows
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(
        &self,
        first: &Candle,
        second: &Candle,
        third: &Candle,
        context: &PatternContext,
    ) -> Result<Confidence, PatternError> {
        let mut confidence_score: f64 = 60.0; // Base 60%

        // Body size consistency - similar sized bodies increase confidence
        let body_ratios = [
            self.body_ratio(first),
            self.body_ratio(second),
            self.body_ratio(third),
        ];
        let avg_body_ratio = body_ratios.iter().sum::<f64>() / 3.0;
        let body_consistency = body_ratios
            .iter()
            .map(|&ratio| (ratio - avg_body_ratio).abs())
            .sum::<f64>()
            / 3.0;

        if body_consistency <= 0.1 {
            confidence_score += 15.0; // Very consistent bodies
        } else if body_consistency <= 0.2 {
            confidence_score += 10.0; // Moderately consistent bodies
        }

        // Strong bodies (large relative to range)
        if avg_body_ratio >= 0.7 {
            confidence_score += 15.0;
        } else if avg_body_ratio >= 0.5 {
            confidence_score += 10.0;
        }

        // Minimal lower shadows
        let avg_lower_shadow = [
            self.lower_shadow_ratio(first),
            self.lower_shadow_ratio(second),
            self.lower_shadow_ratio(third),
        ]
        .iter()
        .sum::<f64>()
            / 3.0;

        if avg_lower_shadow <= 0.1 {
            confidence_score += 15.0; // Very small shadows
        } else if avg_lower_shadow <= 0.2 {
            confidence_score += 10.0; // Small shadows
        }

        // Progressive price decline
        let first_decline = (first.open.value() - first.close.value()) / first.open.value();
        let second_decline = (second.open.value() - second.close.value()) / second.open.value();
        let third_decline = (third.open.value() - third.close.value()) / third.open.value();

        let declines = [
            first_decline.to_f64().unwrap_or(0.0),
            second_decline.to_f64().unwrap_or(0.0),
            third_decline.to_f64().unwrap_or(0.0),
        ];

        let avg_decline = declines.iter().sum::<f64>() / 3.0;
        if avg_decline >= 0.03 {
            confidence_score += 15.0; // Strong declines (3%+)
        } else if avg_decline >= 0.02 {
            confidence_score += 10.0; // Good declines (2%+)
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (more reliable in downtrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bearish() {
                confidence_score += 10.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0).map_err(|e| PatternError::InvalidParameters {
            message: e.to_string(),
        })
    }

    /// Helper function to calculate body ratio
    fn body_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let body_size = candle.body_size();
        decimal_to_ratio(body_size, range)
    }

    /// Helper function to calculate lower shadow ratio
    fn lower_shadow_ratio(&self, candle: &Candle) -> f64 {
        let range = candle.range();
        if range == Decimal::ZERO {
            return 0.0;
        }
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        decimal_to_ratio(lower_shadow, range)
    }
}

impl PatternDetector for ThreeBlackCrowsDetector {
    fn name(&self) -> &'static str {
        "three_black_crows"
    }

    fn min_candles_required(&self) -> usize {
        3
    }

    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 3 {
            return Err(PatternError::InsufficientData {
                required: 3,
                available: candles.len(),
            });
        }

        let first = &candles[2]; // Oldest candle
        let second = &candles[1]; // Middle candle
        let third = &candles[0]; // Newest candle

        if !self.validate_three_black_crows(first, second, third) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(3);

        // Add volume confirmation based on third candle
        if candles.len() > 3 {
            let current_volume = third.volume as f64;
            let avg_volume = candles[3..].iter().map(|c| c.volume as f64).sum::<f64>()
                / (candles.len() - 3) as f64;
            let volume_ratio = if avg_volume > 0.0 {
                current_volume / avg_volume
            } else {
                1.0
            };
            context =
                context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let total_decline = ((first.open.value() - third.close.value()) / first.open.value())
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);
        context = context.add_metadata("total_decline", &format!("{:.4}", total_decline));

        let confidence = self.calculate_confidence(first, second, third, &context)?;

        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("three_black_crows"),
            SignalType::Sell,
            confidence,
            third.close,
            context,
        );

        Ok(Some(result))
    }
}
