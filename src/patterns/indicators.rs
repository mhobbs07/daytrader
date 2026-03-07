//! Technical indicators for pattern recognition and signal generation
//!
//! This module provides implementations of common technical indicators used in trading:
//! - Moving averages (SMA, EMA) with crossover detection
//! - Momentum indicators (RSI, MACD) for overbought/oversold conditions
//! - Volatility indicators (Bollinger Bands, ATR) with breakout detection
//!
//! All indicators follow functional programming principles with pure functions,
//! iterator chains, and zero-copy operations where possible.
//!
//! # Architecture
//!
//! Indicators are implemented as pure functions that take price data as input
//! and return calculated values. Each indicator includes:
//!
//! - Core calculation functions
//! - Signal generation logic
//! - Event integration for real-time updates
//!
//! # Examples
//!
//! ```
//! use daytrader::patterns::indicators::{sma, ema, rsi, macd, bollinger_bands, atr};
//! use daytrader::events::types::Candle;
//! use rust_decimal::Decimal;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Calculate 20-period SMA
//! let prices = vec![Decimal::new(100, 0), Decimal::new(101, 0), /* ... */];
//! let sma_values = sma(&prices, 20)?;
//!
//! // Calculate RSI for overbought/oversold signals
//! let rsi_values = rsi(&prices, 14)?;
//! for &rsi_val in &rsi_values {
//!     if rsi_val > 70.0 {
//!         println!("Overbought condition");
//!     } else if rsi_val < 30.0 {
//!         println!("Oversold condition");
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use crate::brokers::types::Price;
use crate::events::types::{Candle, MarketEvent};
use crate::patterns::types::PatternError;
use rust_decimal::{Decimal, prelude::*};
use std::collections::VecDeque;

/// Result type for indicator calculations
pub type IndicatorResult<T> = Result<T, PatternError>;

// ============================================================================
// Moving Averages
// ============================================================================

/// Calculate Simple Moving Average (SMA)
///
/// SMA is the arithmetic mean of a series of values over a specified period.
/// Formula: SMA = (P1 + P2 + ... + Pn) / n
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `period` - Number of periods for the moving average
///
/// # Returns
/// Vector of SMA values, starting from the (period-1)th index
pub fn sma(prices: &[Decimal], period: usize) -> IndicatorResult<Vec<Decimal>> {
    if prices.len() < period {
        return Err(PatternError::InsufficientData {
            required: period,
            available: prices.len(),
        });
    }

    let mut result = Vec::with_capacity(prices.len().saturating_sub(period - 1));

    for i in (period - 1)..prices.len() {
        let sum: Decimal = prices[i.saturating_sub(period - 1)..=i]
            .iter()
            .sum();
        result.push(sum / Decimal::from(period));
    }

    Ok(result)
}

/// Calculate Exponential Moving Average (EMA)
///
/// EMA gives more weight to recent prices.
/// Formula: EMA(today) = (Price(today) * Multiplier) + (EMA(yesterday) * (1 - Multiplier))
/// where Multiplier = 2 / (period + 1)
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `period` - Number of periods for the moving average
///
/// # Returns
/// Vector of EMA values, starting from the (period-1)th index
pub fn ema(prices: &[Decimal], period: usize) -> IndicatorResult<Vec<Decimal>> {
    if prices.len() < period {
        return Err(PatternError::InsufficientData {
            required: period,
            available: prices.len(),
        });
    }

    let multiplier = Decimal::from(2) / Decimal::from(period + 1);
    let mut result = Vec::with_capacity(prices.len().saturating_sub(period - 1));

    // First EMA is SMA
    let first_sma: Decimal = prices[..period].iter().sum::<Decimal>() / Decimal::from(period);
    result.push(first_sma);

    // Calculate subsequent EMAs
    let mut prev_ema = first_sma;
    for &price in &prices[period..] {
        let current_ema = (price * multiplier) + (prev_ema * (Decimal::ONE - multiplier));
        result.push(current_ema);
        prev_ema = current_ema;
    }

    Ok(result)
}

/// Detect moving average crossovers
///
/// Returns true when fast MA crosses above slow MA (bullish) or
/// false when fast MA crosses below slow MA (bearish)
///
/// # Arguments
/// * `fast_ma` - Values from faster moving average
/// * `slow_ma` - Values from slower moving average
///
/// # Returns
/// Vector of Option<bool> where Some(true) = bullish crossover, Some(false) = bearish crossover
pub fn detect_ma_crossover(fast_ma: &[Decimal], slow_ma: &[Decimal]) -> Vec<Option<bool>> {
    let min_len = fast_ma.len().min(slow_ma.len());
    let mut result = Vec::with_capacity(min_len.saturating_sub(1));

    for i in 1..min_len {
        let prev_fast = fast_ma[i - 1];
        let prev_slow = slow_ma[i - 1];
        let curr_fast = fast_ma[i];
        let curr_slow = slow_ma[i];

        // Bullish crossover: fast crosses above slow
        if prev_fast <= prev_slow && curr_fast > curr_slow {
            result.push(Some(true));
        }
        // Bearish crossover: fast crosses below slow
        else if prev_fast >= prev_slow && curr_fast < curr_slow {
            result.push(Some(false));
        } else {
            result.push(None);
        }
    }

    result
}

// ============================================================================
// Momentum Indicators
// ============================================================================

/// Calculate Relative Strength Index (RSI)
///
/// RSI measures the speed and change of price movements on a scale of 0 to 100.
/// Values above 70 indicate overbought conditions, below 30 indicate oversold.
///
/// Formula:
/// RSI = 100 - (100 / (1 + RS))
/// RS = Average Gain / Average Loss
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `period` - Number of periods for RSI calculation (typically 14)
///
/// # Returns
/// Vector of RSI values (0-100)
pub fn rsi(prices: &[Decimal], period: usize) -> IndicatorResult<Vec<f64>> {
    if prices.len() < period + 1 {
        return Err(PatternError::InsufficientData {
            required: period + 1,
            available: prices.len(),
        });
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    // Calculate price changes
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > Decimal::ZERO {
            gains.push(change.to_f64().unwrap_or(0.0));
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push((-change).to_f64().unwrap_or(0.0));
        }
    }

    let mut result = Vec::with_capacity(gains.len().saturating_sub(period - 1));

    // Calculate initial averages
    let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

    // Calculate first RSI
    let rs = if avg_loss == 0.0 { f64::INFINITY } else { avg_gain / avg_loss };
    let rsi_val = 100.0 - (100.0 / (1.0 + rs));
    result.push(rsi_val);

    // Calculate smoothed RSI values
    for i in period..gains.len() {
        avg_gain = (avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;

        let rs = if avg_loss == 0.0 { f64::INFINITY } else { avg_gain / avg_loss };
        let rsi_val = 100.0 - (100.0 / (1.0 + rs));
        result.push(rsi_val);
    }

    Ok(result)
}

/// Calculate MACD (Moving Average Convergence Divergence)
///
/// MACD is calculated as the difference between a fast EMA and slow EMA.
/// A signal line is then calculated as an EMA of the MACD line.
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `fast_period` - Fast EMA period (typically 12)
/// * `slow_period` - Slow EMA period (typically 26)
/// * `signal_period` - Signal line EMA period (typically 9)
///
/// # Returns
/// Tuple of (macd_line, signal_line, histogram)
pub fn macd(
    prices: &[Decimal],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> IndicatorResult<(Vec<Decimal>, Vec<Decimal>, Vec<Decimal>)> {
    let fast_ema = ema(prices, fast_period)?;
    let slow_ema = ema(prices, slow_period)?;

    // MACD line = fast EMA - slow EMA
    let macd_line: Vec<Decimal> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(fast, slow)| fast - slow)
        .collect();

    // Signal line = EMA of MACD line
    let signal_line = ema(&macd_line, signal_period)?;

    // Histogram = MACD line - signal line
    let histogram: Vec<Decimal> = macd_line
        .iter()
        .zip(signal_line.iter())
        .map(|(macd, signal)| macd - signal)
        .collect();

    Ok((macd_line, signal_line, histogram))
}

/// Detect MACD crossovers
///
/// Returns true for bullish crossover (MACD crosses above signal) or
/// false for bearish crossover (MACD crosses below signal)
///
/// # Arguments
/// * `macd_line` - MACD line values
/// * `signal_line` - Signal line values
///
/// # Returns
/// Vector of Option<bool> where Some(true) = bullish crossover, Some(false) = bearish crossover
pub fn detect_macd_crossover(macd_line: &[Decimal], signal_line: &[Decimal]) -> Vec<Option<bool>> {
    detect_ma_crossover(macd_line, signal_line)
}

// ============================================================================
// Volatility Indicators
// ============================================================================

/// Calculate Bollinger Bands
///
/// Bollinger Bands consist of a middle band (SMA) and upper/lower bands
/// calculated as SMA ± (standard deviation × multiplier)
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `period` - Period for SMA and standard deviation (typically 20)
/// * `std_dev_multiplier` - Standard deviation multiplier (typically 2.0)
///
/// # Returns
/// Tuple of (upper_band, middle_band, lower_band)
pub fn bollinger_bands(
    prices: &[Decimal],
    period: usize,
    std_dev_multiplier: f64,
) -> IndicatorResult<(Vec<Decimal>, Vec<Decimal>, Vec<Decimal>)> {
    let middle_band = sma(prices, period)?;

    let mut upper_band = Vec::with_capacity(middle_band.len());
    let mut lower_band = Vec::with_capacity(middle_band.len());

    for (i, &sma_val) in middle_band.iter().enumerate() {
        let start_idx = i;
        let end_idx = i + period;
        let window = &prices[start_idx..end_idx];

        // Calculate standard deviation
        let mean = sma_val;
        let variance: Decimal = window
            .iter()
            .map(|price| {
                let diff = price - mean;
                diff * diff
            })
            .sum::<Decimal>() / Decimal::from(period);

        let std_dev = Decimal::from_f64((variance.to_f64().unwrap_or(0.0)).sqrt()).unwrap_or(Decimal::ZERO);
        let multiplier = Decimal::from_f64(std_dev_multiplier).unwrap_or(Decimal::from(2));

        upper_band.push(mean + (std_dev * multiplier));
        lower_band.push(mean - (std_dev * multiplier));
    }

    Ok((upper_band, middle_band, lower_band))
}

/// Calculate Average True Range (ATR)
///
/// ATR measures volatility by calculating the average of true ranges over a period.
/// True Range is the maximum of:
/// - Current High - Current Low
/// - |Current High - Previous Close|
/// - |Current Low - Previous Close|
///
/// # Arguments
/// * `candles` - Slice of candle data with OHLC
/// * `period` - Period for ATR calculation (typically 14)
///
/// # Returns
/// Vector of ATR values
pub fn atr(candles: &[Candle], period: usize) -> IndicatorResult<Vec<Decimal>> {
    if candles.len() < period + 1 {
        return Err(PatternError::InsufficientData {
            required: period + 1,
            available: candles.len(),
        });
    }

    let mut true_ranges = Vec::with_capacity(candles.len() - 1);

    // Calculate True Ranges
    for i in 1..candles.len() {
        let current = &candles[i];
        let previous = &candles[i - 1];

        let tr1 = current.high.value() - current.low.value();
        let tr2 = (current.high.value() - previous.close.value()).abs();
        let tr3 = (current.low.value() - previous.close.value()).abs();

        let true_range = tr1.max(tr2.max(tr3));
        true_ranges.push(true_range);
    }

    // Calculate ATR as EMA of True Ranges
    ema(&true_ranges, period)
}

/// Detect volatility breakouts
///
/// A breakout occurs when price moves beyond a volatility threshold.
/// Uses ATR to set dynamic breakout levels.
///
/// # Arguments
/// * `prices` - Slice of price values
/// * `atr_values` - Corresponding ATR values
/// * `multiplier` - ATR multiplier for breakout threshold (typically 2.0-3.0)
///
/// # Returns
/// Vector of Option<bool> where Some(true) = upside breakout, Some(false) = downside breakout
pub fn detect_volatility_breakout(
    prices: &[Decimal],
    atr_values: &[Decimal],
    multiplier: f64,
) -> Vec<Option<bool>> {
    let mut result = Vec::with_capacity(prices.len().saturating_sub(1));

    for i in 1..prices.len().min(atr_values.len() + 1) {
        let current_price = prices[i];
        let previous_price = prices[i - 1];
        let atr = atr_values[i - 1];
        let threshold = atr * Decimal::from_f64(multiplier).unwrap_or(Decimal::from(2));

        let change = (current_price - previous_price).abs();

        if change > threshold {
            // Determine direction of breakout
            if current_price > previous_price {
                result.push(Some(true)); // Upside breakout
            } else {
                result.push(Some(false)); // Downside breakout
            }
        } else {
            result.push(None);
        }
    }

    result
}

// ============================================================================
// Event Integration
// ============================================================================

/// Generate indicator update events for the event bus
///
/// This function creates market events when indicators are updated,
/// allowing other components to react to indicator changes.
///
/// # Arguments
/// * `symbol` - Trading symbol
/// * `indicator_name` - Name of the indicator (e.g., "SMA", "RSI")
/// * `values` - Latest indicator values
///
/// # Returns
/// Vector of market events to publish
pub fn generate_indicator_events(
    symbol: crate::brokers::types::Symbol,
    indicator_name: &str,
    values: &[f64],
) -> Vec<MarketEvent> {
    // For now, we'll create a simple event structure
    // In a full implementation, this would create specific indicator events
    vec![MarketEvent::PatternDetected {
        signal: crate::events::types::PatternSignal {
            symbol,
            pattern_name: crate::brokers::types::PatternName::new(indicator_name),
            signal_type: crate::events::types::SignalType::Buy, // Placeholder
            confidence: crate::brokers::types::Confidence::new(0.5).unwrap(), // Placeholder
            detected_at: chrono::Utc::now(),
            price: crate::brokers::types::Price::new(rust_decimal::Decimal::ZERO).unwrap(), // Placeholder
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_sma_calculation() {
        let prices = vec![
            Decimal::new(10, 0),
            Decimal::new(11, 0),
            Decimal::new(12, 0),
            Decimal::new(13, 0),
            Decimal::new(14, 0),
        ];

        let sma_values = sma(&prices, 3).unwrap();
        assert_eq!(sma_values.len(), 3);
        assert_eq!(sma_values[0], Decimal::new(11, 0)); // (10+11+12)/3
        assert_eq!(sma_values[1], Decimal::new(12, 0)); // (11+12+13)/3
        assert_eq!(sma_values[2], Decimal::new(13, 0)); // (12+13+14)/3
    }

    #[test]
    fn test_ema_calculation() {
        let prices = vec![
            Decimal::new(10, 0),
            Decimal::new(11, 0),
            Decimal::new(12, 0),
            Decimal::new(13, 0),
            Decimal::new(14, 0),
        ];

        let ema_values = ema(&prices, 3).unwrap();
        assert_eq!(ema_values.len(), 3);
        // First value should be SMA of first 3: (10+11+12)/3 = 11
        assert_eq!(ema_values[0], Decimal::new(11, 0));
    }

    #[test]
    fn test_rsi_calculation() {
        // Create a simple uptrend to test RSI
        let prices = vec![
            Decimal::new(100, 0),
            Decimal::new(101, 0),
            Decimal::new(102, 0),
            Decimal::new(103, 0),
            Decimal::new(104, 0),
            Decimal::new(105, 0),
            Decimal::new(106, 0),
            Decimal::new(107, 0),
            Decimal::new(108, 0),
            Decimal::new(109, 0),
            Decimal::new(110, 0),
            Decimal::new(111, 0),
            Decimal::new(112, 0),
            Decimal::new(113, 0),
            Decimal::new(114, 0),
        ];

        let rsi_values = rsi(&prices, 14).unwrap();
        assert_eq!(rsi_values.len(), 1);
        // Strong uptrend should have high RSI
        assert!(rsi_values[0] > 70.0);
    }

    #[test]
    fn test_insufficient_data_error() {
        let prices = vec![Decimal::new(100, 0), Decimal::new(101, 0)];
        assert!(sma(&prices, 5).is_err());
        assert!(ema(&prices, 5).is_err());
        assert!(rsi(&prices, 14).is_err());
    }
}