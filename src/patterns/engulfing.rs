//! Engulfing candlestick pattern detection algorithms
//!
//! This module implements two-candle engulfing patterns:
//! - Bullish Engulfing: Bearish candle followed by larger bullish candle that engulfs it
//! - Bearish Engulfing: Bullish candle followed by larger bearish candle that engulfs it
//!
//! Engulfing patterns are powerful reversal signals that indicate a shift in market sentiment.
//! The algorithms validate engulfing criteria with confidence scoring based on pattern metrics,
//! volume confirmation, and market context.

use crate::brokers::types::{Confidence, PatternName, Price};
use crate::events::types::{Candle, SignalType};
use crate::patterns::traits::PatternDetector;
use crate::patterns::types::{
    PatternContext, PatternError, PatternParams, PatternResult,
    VolumeConfirmation, TrendContext
};
use rust_decimal::Decimal;
use std::fmt::Debug;

/// Bullish Engulfing pattern detector - bullish reversal pattern
///
/// A bullish engulfing pattern is characterized by:
/// - First candle: Bearish (red) candle
/// - Second candle: Bullish (green) candle that completely engulfs the first
/// - The second candle's body must completely contain the first candle's body
/// - Appears after a downtrend (reversal signal)
///
/// The pattern indicates strong bullish reversal as buyers overwhelmed sellers.
#[derive(Debug, Clone)]
pub struct BullishEngulfingDetector;

impl BullishEngulfingDetector {
    /// Creates a new bullish engulfing detector
    pub fn new() -> Self {
        Self
    }

    /// Validates bullish engulfing pattern criteria
    fn validate_bullish_engulfing(&self, first: &Candle, second: &Candle) -> bool {
        // First candle must be bearish (close < open)
        let first_bearish = first.close.value() < first.open.value();
        
        // Second candle must be bullish (close > open)
        let second_bullish = second.close.value() > second.open.value();
        
        // Second candle must engulf first candle's body
        let engulfs = second.open.value() < first.close.value() && 
                     second.close.value() > first.open.value();
        
        // Additional validation: second candle should be significantly larger
        let first_body = first.body_size();
        let second_body = second.body_size();
        let size_ratio = if first_body > Decimal::ZERO {
            (second_body / first_body).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        
        let size_valid = size_ratio >= 1.2; // Second candle at least 20% larger

        first_bearish && second_bullish && engulfs && size_valid
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(&self, first: &Candle, second: &Candle, context: &PatternContext) -> Result<Confidence, PatternError> {
        let first_body = first.body_size();
        let second_body = second.body_size();
        
        let mut confidence_score: f64 = 60.0; // Base 60%

        // Engulfing ratio - how much larger the second candle is
        if first_body > Decimal::ZERO {
            let engulfing_ratio = (second_body / first_body).to_string().parse::<f64>().unwrap_or(0.0);
            if engulfing_ratio >= 2.0 {
                confidence_score += 20.0;
            } else if engulfing_ratio >= 1.5 {
                confidence_score += 15.0;
            } else if engulfing_ratio >= 1.2 {
                confidence_score += 10.0;
            }
        }

        // Gap analysis - if there's a gap, it's more significant
        let gap_down = second.open.value() < first.close.value();
        if gap_down {
            let gap_size = (first.close.value() - second.open.value()) / first.close.value();
            let gap_ratio = gap_size.to_string().parse::<f64>().unwrap_or(0.0);
            if gap_ratio >= 0.02 {
                confidence_score += 15.0; // 2%+ gap
            } else if gap_ratio >= 0.01 {
                confidence_score += 10.0; // 1%+ gap
            }
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 15.0;
            } else if volume_conf.ratio() >= 1.0 {
                confidence_score += 5.0;
            }
        }

        // Trend context (more reliable after downtrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bearish() {
                confidence_score += 15.0;
            } else if trend.is_sideways() {
                confidence_score += 5.0;
            }
        }

        // Body size relative to range
        let first_range = first.range();
        let second_range = second.range();
        if first_range > Decimal::ZERO && second_range > Decimal::ZERO {
            let first_body_ratio = (first_body / first_range).to_string().parse::<f64>().unwrap_or(0.0);
            let second_body_ratio = (second_body / second_range).to_string().parse::<f64>().unwrap_or(0.0);
            
            // Strong bodies (low shadow ratio) increase confidence
            if first_body_ratio >= 0.7 && second_body_ratio >= 0.7 {
                confidence_score += 10.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0)
            .map_err(|e| PatternError::InvalidParameters { message: e.to_string() })
    }
}

impl PatternDetector for BullishEngulfingDetector {
    fn name(&self) -> &'static str {
        "bullish_engulfing"
    }

    fn min_candles_required(&self) -> usize {
        2
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 2 {
            return Err(PatternError::InsufficientData {
                required: 2,
                available: candles.len(),
            });
        }

        let first = &candles[1];  // Previous candle
        let second = &candles[0]; // Current candle
        
        if !self.validate_bullish_engulfing(first, second) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(2);
        
        // Add volume confirmation
        if candles.len() > 2 {
            let current_volume = second.volume as f64;
            let avg_volume = candles[2..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - 2) as f64;
            let volume_ratio = if avg_volume > 0.0 { current_volume / avg_volume } else { 1.0 };
            context = context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let engulfing_ratio = if first.body_size() > Decimal::ZERO {
            (second.body_size() / first.body_size()).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        context = context.add_metadata("engulfing_ratio", &engulfing_ratio.to_string());

        let confidence = self.calculate_confidence(first, second, &context)?;
        
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("bullish_engulfing"),
            SignalType::Buy,
            confidence,
            second.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Bearish Engulfing pattern detector - bearish reversal pattern
///
/// A bearish engulfing pattern is characterized by:
/// - First candle: Bullish (green) candle
/// - Second candle: Bearish (red) candle that completely engulfs the first
/// - The second candle's body must completely contain the first candle's body
/// - Appears after an uptrend (reversal signal)
///
/// The pattern indicates strong bearish reversal as sellers overwhelmed buyers.
#[derive(Debug, Clone)]
pub struct BearishEngulfingDetector;

impl BearishEngulfingDetector {
    /// Creates a new bearish engulfing detector
    pub fn new() -> Self {
        Self
    }

    /// Validates bearish engulfing pattern criteria
    fn validate_bearish_engulfing(&self, first: &Candle, second: &Candle) -> bool {
        // First candle must be bullish (close > open)
        let first_bullish = first.close.value() > first.open.value();
        
        // Second candle must be bearish (close < open)
        let second_bearish = second.close.value() < second.open.value();
        
        // Second candle must engulf first candle's body
        let engulfs = second.open.value() > first.close.value() && 
                     second.close.value() < first.open.value();
        
        // Additional validation: second candle should be significantly larger
        let first_body = first.body_size();
        let second_body = second.body_size();
        let size_ratio = if first_body > Decimal::ZERO {
            (second_body / first_body).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        
        let size_valid = size_ratio >= 1.2; // Second candle at least 20% larger

        first_bullish && second_bearish && engulfs && size_valid
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(&self, first: &Candle, second: &Candle, context: &PatternContext) -> Result<Confidence, PatternError> {
        let first_body = first.body_size();
        let second_body = second.body_size();
        
        let mut confidence_score: f64 = 60.0; // Base 60%

        // Engulfing ratio - how much larger the second candle is
        if first_body > Decimal::ZERO {
            let engulfing_ratio = (second_body / first_body).to_string().parse::<f64>().unwrap_or(0.0);
            if engulfing_ratio >= 2.0 {
                confidence_score += 20.0;
            } else if engulfing_ratio >= 1.5 {
                confidence_score += 15.0;
            } else if engulfing_ratio >= 1.2 {
                confidence_score += 10.0;
            }
        }

        // Gap analysis - if there's a gap, it's more significant
        let gap_up = second.open.value() > first.close.value();
        if gap_up {
            let gap_size = (second.open.value() - first.close.value()) / first.close.value();
            let gap_ratio = gap_size.to_string().parse::<f64>().unwrap_or(0.0);
            if gap_ratio >= 0.02 {
                confidence_score += 15.0; // 2%+ gap
            } else if gap_ratio >= 0.01 {
                confidence_score += 10.0; // 1%+ gap
            }
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 15.0;
            } else if volume_conf.ratio() >= 1.0 {
                confidence_score += 5.0;
            }
        }

        // Trend context (more reliable after uptrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bullish() {
                confidence_score += 15.0;
            } else if trend.is_sideways() {
                confidence_score += 5.0;
            }
        }

        // Body size relative to range
        let first_range = first.range();
        let second_range = second.range();
        if first_range > Decimal::ZERO && second_range > Decimal::ZERO {
            let first_body_ratio = (first_body / first_range).to_string().parse::<f64>().unwrap_or(0.0);
            let second_body_ratio = (second_body / second_range).to_string().parse::<f64>().unwrap_or(0.0);
            
            // Strong bodies (low shadow ratio) increase confidence
            if first_body_ratio >= 0.7 && second_body_ratio >= 0.7 {
                confidence_score += 10.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0)
            .map_err(|e| PatternError::InvalidParameters { message: e.to_string() })
    }
}

impl PatternDetector for BearishEngulfingDetector {
    fn name(&self) -> &'static str {
        "bearish_engulfing"
    }

    fn min_candles_required(&self) -> usize {
        2
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < 2 {
            return Err(PatternError::InsufficientData {
                required: 2,
                available: candles.len(),
            });
        }

        let first = &candles[1];  // Previous candle
        let second = &candles[0]; // Current candle
        
        if !self.validate_bearish_engulfing(first, second) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(2);
        
        // Add volume confirmation
        if candles.len() > 2 {
            let current_volume = second.volume as f64;
            let avg_volume = candles[2..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - 2) as f64;
            let volume_ratio = if avg_volume > 0.0 { current_volume / avg_volume } else { 1.0 };
            context = context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        // Add pattern-specific metadata
        let engulfing_ratio = if first.body_size() > Decimal::ZERO {
            (second.body_size() / first.body_size()).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        context = context.add_metadata("engulfing_ratio", &engulfing_ratio.to_string());

        let confidence = self.calculate_confidence(first, second, &context)?;
        
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("bearish_engulfing"),
            SignalType::Sell,
            confidence,
            second.close,
            context,
        );

        Ok(Some(result))
    }
}