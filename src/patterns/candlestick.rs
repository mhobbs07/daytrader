use crate::brokers::types::{Confidence, PatternName, Price};
use crate::events::types::{Candle, SignalType};
use crate::patterns::traits::PatternDetector;
use crate::patterns::types::{
    PatternContext, PatternError, PatternParams, PatternResult,
    VolumeConfirmation, TrendContext
};
use crate::patterns::utils::{
    calculate_single_candle_confidence, calculate_volume_ratio, create_volume_confirmation,
    validate_hammer, validate_minimum_data, SingleCandlePatternType
};
use rust_decimal::Decimal;
use std::fmt::Debug;
//! Basic candlestick pattern detection algorithms
//!
//! This module implements fundamental single-candle candlestick patterns including:
//! - Hammer: Bullish reversal pattern with long lower shadow
//! - Shooting Star: Bearish reversal pattern with long upper shadow  
//! - Doji: Indecision pattern with small body (Standard, Dragonfly, Gravestone)
//!
//! All patterns use mathematical validation of candlestick criteria with confidence scoring
//! based on pattern metrics, market context, and volume confirmation.


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
}

impl PatternDetector for HammerDetector {
    fn name(&self) -> &'static str {
        "hammer"
    }

    fn min_candles_required(&self) -> usize {
        1
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        validate_minimum_data(candles, 1)?;

        let candle = &candles[0];
        
        if !validate_hammer(candle) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(1);
        
        // Add volume confirmation if we have enough data
        if candles.len() > 1 {
            let volume_ratio = calculate_volume_ratio(candle, &candles[1..]);
            context = context.with_volume_confirmation(create_volume_confirmation(volume_ratio));
        }

        let confidence = calculate_single_candle_confidence(&candle, &context, SingleCandlePatternType::Hammer)?;
        
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
}

impl PatternDetector for ShootingStarDetector {
    fn name(&self) -> &'static str {
        "shooting_star"
    }

    fn min_candles_required(&self) -> usize {
        1
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        validate_minimum_data(candles, 1)?;

        let candle = &candles[0];
        
        if !validate_shooting_star(candle) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(1);
        
        // Add volume confirmation if we have enough data
        if candles.len() > 1 {
            let volume_ratio = calculate_volume_ratio(candle, &candles[1..]);
            context = context.with_volume_confirmation(create_volume_confirmation(volume_ratio));
        }

        let confidence = calculate_single_candle_confidence(&candle, &context, SingleCandlePatternType::ShootingStar)?;
        
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
        validate_minimum_data(candles, 1)?;

        let candle = &candles[0];
        
        if !validate_doji(candle) {
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
            let volume_ratio = calculate_volume_ratio(candle, &candles[1..]);
            context = context.with_volume_confirmation(create_volume_confirmation(volume_ratio));
        }

        let confidence = calculate_single_candle_confidence(&candle, &context, SingleCandlePatternType::Doji)?;
        
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