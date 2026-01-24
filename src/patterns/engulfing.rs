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
use crate::patterns::utils::{
    calculate_engulfing_confidence, calculate_gap_ratio, calculate_volume_ratio,
    create_volume_confirmation, validate_bearish_engulfing, validate_bullish_engulfing,
    validate_minimum_data
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
}

impl PatternDetector for BullishEngulfingDetector {
    fn name(&self) -> &'static str {
        "bullish_engulfing"
    }

    fn min_candles_required(&self) -> usize {
        2
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        validate_minimum_data(candles, 2)?;

        let first = &candles[1];  // Previous candle
        let second = &candles[0]; // Current candle
        
        if !validate_bullish_engulfing(first, second) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(2);
        
        // Add volume confirmation
        if candles.len() > 2 {
            let volume_ratio = calculate_volume_ratio(second, &candles[2..]);
            context = context.with_volume_confirmation(create_volume_confirmation(volume_ratio));
        }

        // Add pattern-specific metadata
        let engulfing_ratio = if first.body_size() > Decimal::ZERO {
            (second.body_size() / first.body_size()).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        context = context.add_metadata("engulfing_ratio", &engulfing_ratio.to_string());

        // Calculate gap ratio for bullish engulfing (gap down)
        let gap_ratio = calculate_gap_ratio(first, second);

        let confidence = calculate_engulfing_confidence(engulfing_ratio, gap_ratio, &second, &context)?;
        
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
}

impl PatternDetector for BearishEngulfingDetector {
    fn name(&self) -> &'static str {
        "bearish_engulfing"
    }

    fn min_candles_required(&self) -> usize {
        2
    }

    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>, PatternError> {
        validate_minimum_data(candles, 2)?;

        let first = &candles[1];  // Previous candle
        let second = &candles[0]; // Current candle
        
        if !validate_bearish_engulfing(first, second) {
            return Ok(None);
        }

        // Build context
        let mut context = PatternContext::new(2);
        
        // Add volume confirmation
        if candles.len() > 2 {
            let volume_ratio = calculate_volume_ratio(second, &candles[2..]);
            context = context.with_volume_confirmation(create_volume_confirmation(volume_ratio));
        }

        // Add pattern-specific metadata
        let engulfing_ratio = if first.body_size() > Decimal::ZERO {
            (second.body_size() / first.body_size()).to_string().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        context = context.add_metadata("engulfing_ratio", &engulfing_ratio.to_string());

        // Calculate gap ratio for bearish engulfing (gap up)
        let gap_ratio = calculate_gap_ratio(first, second);

        let confidence = calculate_engulfing_confidence(engulfing_ratio, gap_ratio, &second, &context)?;
        
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