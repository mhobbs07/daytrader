//! Basic candlestick pattern detection algorithms
//!
//! This module implements fundamental single-candle candlestick patterns including:
//! - Hammer: Bullish reversal pattern with long lower shadow
//! - Shooting Star: Bearish reversal pattern with long upper shadow  
//! - Doji: Indecision pattern with small body (Standard, Dragonfly, Gravestone)
//!
//! All patterns use mathematical validation of candlestick criteria with confidence scoring
//! based on pattern metrics, market context, and volume confirmation.

use crate::brokers::types::{Confidence, PatternName, Price};
use crate::events::types::{Candle, SignalType};
use crate::patterns::traits::PatternDetector;
use crate::patterns::types::{
    PatternContext, PatternError, PatternParams, PatternResult,
    VolumeConfirmation, TrendContext
};
use rust_decimal::Decimal;
use std::fmt::Debug;

/// Hammer pattern detector - bullish reversal pattern
///
/// A hammer is characterized by:
/// - Long lower shadow (at least 2x the body size)
/// - Small upper shadow (less than body size)
/// - Small to medium body size
/// - Appears after a downtrend (reversal signal)
///
/// The pattern indicates potential bullish reversal as buyers stepped in
/// at lower prices, pushing the price back up.
#[derive(Debug, Clone)]
pub struct HammerDetector;

impl HammerDetector {
    /// Creates a new hammer detector
    pub fn new() -> Self {
        Self
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(&self, candle: &Candle, context: &PatternContext) -> Result<Confidence, PatternError> {
        let range = candle.range();
        if range == Decimal::ZERO {
            return Ok(Confidence::new(0.0).unwrap());
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        // Base confidence from shadow ratios
        let lower_shadow_ratio = (lower_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);
        let upper_shadow_ratio = (upper_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);

        let mut confidence_score: f64 = 50.0; // Base 50%

        // Long lower shadow increases confidence
        if lower_shadow_ratio >= 0.6 {
            confidence_score += 25.0;
        } else if lower_shadow_ratio >= 0.4 {
            confidence_score += 15.0;
        }

        // Small upper shadow increases confidence
        if upper_shadow_ratio <= 0.1 {
            confidence_score += 15.0;
        } else if upper_shadow_ratio <= 0.2 {
            confidence_score += 10.0;
        }

        // Small to medium body increases confidence
        if body_ratio >= 0.1 && body_ratio <= 0.3 {
            confidence_score += 10.0;
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (hammer more reliable after downtrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bearish() {
                confidence_score += 15.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0)
            .map_err(|e| PatternError::InvalidParameters { message: e.to_string() })
    }

    /// Validates hammer pattern criteria
    fn validate_hammer(&self, candle: &Candle) -> bool {
        let range = candle.range();
        if range == Decimal::ZERO {
            return false;
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        // Lower shadow must be at least 2x the body size
        let lower_shadow_valid = lower_shadow >= body_size * Decimal::new(2, 0);
        
        // Upper shadow should be small (less than body size)
        let upper_shadow_valid = upper_shadow <= body_size;
        
        // Body should be small to medium (10-40% of range)
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);
        let body_valid = body_ratio >= 0.1 && body_ratio <= 0.4;

        lower_shadow_valid && upper_shadow_valid && body_valid
    }
}

impl PatternDetector for HammerDetector {
    fn name(&self) -> &'static str {
        "hammer"
    }

    fn min_candles_required(&self) -> usize {
        1
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        if candles.is_empty() {
            return Err(PatternError::InsufficientData {
                required: 1,
                available: 0,
            });
        }

        let candle = &candles[0];
        
        if !self.validate_hammer(candle) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(1);
        
        // Add volume confirmation if we have enough data
        if candles.len() > 1 {
            let volume_ratio = candle.volume as f64 / 
                (candles[1..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - 1) as f64);
            context = context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        let confidence = self.calculate_confidence(candle, &context)?;
        
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("hammer"),
            SignalType::Buy,
            confidence,
            candle.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Shooting Star pattern detector - bearish reversal pattern
///
/// A shooting star is characterized by:
/// - Long upper shadow (at least 2x the body size)
/// - Small lower shadow (less than body size)
/// - Small to medium body size
/// - Appears after an uptrend (reversal signal)
///
/// The pattern indicates potential bearish reversal as sellers stepped in
/// at higher prices, pushing the price back down.
#[derive(Debug, Clone)]
pub struct ShootingStarDetector;

impl ShootingStarDetector {
    /// Creates a new shooting star detector
    pub fn new() -> Self {
        Self
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(&self, candle: &Candle, context: &PatternContext) -> Result<Confidence, PatternError> {
        let range = candle.range();
        if range == Decimal::ZERO {
            return Ok(Confidence::new(0.0).unwrap());
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        // Base confidence from shadow ratios
        let lower_shadow_ratio = (lower_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);
        let upper_shadow_ratio = (upper_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);

        let mut confidence_score: f64 = 50.0; // Base 50%

        // Long upper shadow increases confidence
        if upper_shadow_ratio >= 0.6 {
            confidence_score += 25.0;
        } else if upper_shadow_ratio >= 0.4 {
            confidence_score += 15.0;
        }

        // Small lower shadow increases confidence
        if lower_shadow_ratio <= 0.1 {
            confidence_score += 15.0;
        } else if lower_shadow_ratio <= 0.2 {
            confidence_score += 10.0;
        }

        // Small to medium body increases confidence
        if body_ratio >= 0.1 && body_ratio <= 0.3 {
            confidence_score += 10.0;
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context (shooting star more reliable after uptrend)
        if let Some(trend) = &context.trend_context {
            if trend.is_bullish() {
                confidence_score += 15.0;
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0)
            .map_err(|e| PatternError::InvalidParameters { message: e.to_string() })
    }

    /// Validates shooting star pattern criteria
    fn validate_shooting_star(&self, candle: &Candle) -> bool {
        let range = candle.range();
        if range == Decimal::ZERO {
            return false;
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        // Upper shadow must be at least 2x the body size
        let upper_shadow_valid = upper_shadow >= body_size * Decimal::new(2, 0);
        
        // Lower shadow should be small (less than body size)
        let lower_shadow_valid = lower_shadow <= body_size;
        
        // Body should be small to medium (10-40% of range)
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);
        let body_valid = body_ratio >= 0.1 && body_ratio <= 0.4;

        upper_shadow_valid && lower_shadow_valid && body_valid
    }
}

impl PatternDetector for ShootingStarDetector {
    fn name(&self) -> &'static str {
        "shooting_star"
    }

    fn min_candles_required(&self) -> usize {
        1
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        if candles.is_empty() {
            return Err(PatternError::InsufficientData {
                required: 1,
                available: 0,
            });
        }

        let candle = &candles[0];
        
        if !self.validate_shooting_star(candle) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(1);
        
        // Add volume confirmation if we have enough data
        if candles.len() > 1 {
            let volume_ratio = candle.volume as f64 / 
                (candles[1..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - 1) as f64);
            context = context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        let confidence = self.calculate_confidence(candle, &context)?;
        
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::with_context(
            PatternName::new("shooting_star"),
            SignalType::Sell,
            confidence,
            candle.close,
            context,
        );

        Ok(Some(result))
    }
}

/// Doji pattern detector - indecision pattern
///
/// A doji is characterized by:
/// - Very small body (open ≈ close)
/// - Can have long or short shadows
/// - Indicates market indecision
/// - Types: Standard, Dragonfly (long lower shadow), Gravestone (long upper shadow)
///
/// The pattern indicates potential trend reversal or continuation depending on context.
#[derive(Debug, Clone)]
pub struct DojiDetector;

impl DojiDetector {
    /// Creates a new doji detector
    pub fn new() -> Self {
        Self
    }

    /// Determines the type of doji pattern
    fn classify_doji(&self, candle: &Candle) -> DojiType {
        let range = candle.range();
        if range == Decimal::ZERO {
            return DojiType::Standard;
        }

        let body_size = candle.body_size();
        let lower_shadow = candle.open.value().min(candle.close.value()) - candle.low.value();
        let upper_shadow = candle.high.value() - candle.open.value().max(candle.close.value());

        let lower_shadow_ratio = (lower_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);
        let upper_shadow_ratio = (upper_shadow / range).to_string().parse::<f64>().unwrap_or(0.0);

        // Dragonfly doji: long lower shadow, minimal upper shadow
        if lower_shadow_ratio >= 0.6 && upper_shadow_ratio <= 0.1 {
            DojiType::Dragonfly
        }
        // Gravestone doji: long upper shadow, minimal lower shadow
        else if upper_shadow_ratio >= 0.6 && lower_shadow_ratio <= 0.1 {
            DojiType::Gravestone
        }
        // Standard doji: balanced or moderate shadows
        else {
            DojiType::Standard
        }
    }

    /// Calculates confidence score based on pattern metrics
    fn calculate_confidence(&self, candle: &Candle, context: &PatternContext, doji_type: DojiType) -> Result<Confidence, PatternError> {
        let range = candle.range();
        if range == Decimal::ZERO {
            return Ok(Confidence::new(0.0).unwrap());
        }

        let body_size = candle.body_size();
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);

        let mut confidence_score: f64 = 40.0; // Base 40%

        // Smaller body increases confidence
        if body_ratio <= 0.05 {
            confidence_score += 30.0;
        } else if body_ratio <= 0.1 {
            confidence_score += 20.0;
        } else if body_ratio <= 0.15 {
            confidence_score += 10.0;
        }

        // Doji type specific adjustments
        match doji_type {
            DojiType::Dragonfly | DojiType::Gravestone => {
                // Specialized dojis have higher confidence
                confidence_score += 15.0;
            }
            DojiType::Standard => {
                // Standard doji gets moderate boost
                confidence_score += 5.0;
            }
        }

        // Volume confirmation
        if let Some(volume_conf) = &context.volume_confirmation {
            if volume_conf.is_confirmed() {
                confidence_score += 10.0;
            }
        }

        // Trend context affects confidence differently for each type
        if let Some(trend) = &context.trend_context {
            match doji_type {
                DojiType::Dragonfly => {
                    // More reliable after downtrend
                    if trend.is_bearish() {
                        confidence_score += 10.0;
                    }
                }
                DojiType::Gravestone => {
                    // More reliable after uptrend
                    if trend.is_bullish() {
                        confidence_score += 10.0;
                    }
                }
                DojiType::Standard => {
                    // Standard doji works in any trend
                    confidence_score += 5.0;
                }
            }
        }

        // Cap at 100%
        confidence_score = confidence_score.min(100.0);

        Confidence::new(confidence_score / 100.0)
            .map_err(|e| PatternError::InvalidParameters { message: e.to_string() })
    }

    /// Validates doji pattern criteria
    fn validate_doji(&self, candle: &Candle) -> bool {
        let range = candle.range();
        if range == Decimal::ZERO {
            return true; // Perfect doji
        }

        let body_size = candle.body_size();
        let body_ratio = (body_size / range).to_string().parse::<f64>().unwrap_or(0.0);

        // Body should be very small (less than 15% of range)
        body_ratio <= 0.15
    }
}

/// Types of doji patterns
#[derive(Debug, Clone, Copy, PartialEq)]
enum DojiType {
    /// Standard doji with balanced shadows
    Standard,
    /// Dragonfly doji with long lower shadow
    Dragonfly,
    /// Gravestone doji with long upper shadow
    Gravestone,
}

impl PatternDetector for DojiDetector {
    fn name(&self) -> &'static str {
        "doji"
    }

    fn min_candles_required(&self) -> usize {
        1
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        if candles.is_empty() {
            return Err(PatternError::InsufficientData {
                required: 1,
                available: 0,
            });
        }

        let candle = &candles[0];
        
        if !self.validate_doji(candle) {
            return Ok(None);
        }

        let doji_type = self.classify_doji(candle);

        // Build context
        let mut context = PatternContext::new(1);
        
        // Add doji type to metadata
        context = context.add_metadata("doji_type", match doji_type {
            DojiType::Standard => "standard",
            DojiType::Dragonfly => "dragonfly", 
            DojiType::Gravestone => "gravestone",
        });
        
        // Add volume confirmation if we have enough data
        if candles.len() > 1 {
            let volume_ratio = candle.volume as f64 / 
                (candles[1..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - 1) as f64);
            context = context.with_volume_confirmation(VolumeConfirmation::from_ratio(volume_ratio));
        }

        let confidence = self.calculate_confidence(candle, &context, doji_type)?;
        
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        // Doji signals depend on type and context
        let signal_type = match doji_type {
            DojiType::Dragonfly => SignalType::Buy,  // Bullish reversal
            DojiType::Gravestone => SignalType::Sell, // Bearish reversal
            DojiType::Standard => {
                // Neutral - could go either way, default to buy for now
                // In practice, this would need more context
                SignalType::Buy
            }
        };

        let result = PatternResult::with_context(
            PatternName::new("doji"),
            signal_type,
            confidence,
            candle.close,
            context,
        );

        Ok(Some(result))
    }
}