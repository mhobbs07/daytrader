//! Data validation module with pure functions
//!
//! This module provides validation functions for market data using pure,
//! functional patterns. All functions are side-effect free and return
//! Results or anomaly lists.

use crate::events::types::{Candle, Quote};
use crate::utils::errors::{AppError, PatternError};
use rust_decimal::Decimal;
use std::fmt;

/// Result type for validation operations
pub type Result<T> = std::result::Result<T, AppError>;

/// Anomaly types detected in market data
#[derive(Debug, Clone, PartialEq)]
pub enum Anomaly {
    /// Price jump exceeds threshold
    UnrealisticPriceJump {
        /// Percentage change
        change_percent: Decimal,
        /// Threshold that was exceeded
        threshold: Decimal,
    },
    /// Volume is zero
    ZeroVolume,
    /// OHLC relationships are invalid
    InvalidOHLC {
        /// Description of the violation
        reason: String,
    },
    /// Spread is unusually wide
    WideSpread {
        /// Spread value
        spread: Decimal,
        /// Threshold that was exceeded
        threshold: Decimal,
    },
}

impl fmt::Display for Anomaly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrealisticPriceJump {
                change_percent,
                threshold,
            } => write!(
                f,
                "Unrealistic price jump: {:.2}% (threshold: {:.2}%)",
                change_percent, threshold
            ),
            Self::ZeroVolume => write!(f, "Zero volume detected"),
            Self::InvalidOHLC { reason } => write!(f, "Invalid OHLC: {}", reason),
            Self::WideSpread { spread, threshold } => {
                write!(f, "Wide spread: {} (threshold: {})", spread, threshold)
            }
        }
    }
}

/// Validate OHLC relationships in a candle
///
/// Ensures that high is the highest price, low is the lowest price,
/// and volume is non-negative.
///
/// # Examples
///
/// ```
/// use daytrader::data::validation::validate_candle;
/// use daytrader::events::types::Candle;
/// use daytrader::brokers::types::{Symbol, Price};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let candle = Candle {
///     symbol: Symbol::new("AAPL"),
///     timestamp: Utc::now(),
///     open: Price::new(Decimal::new(150, 0)).unwrap(),
///     high: Price::new(Decimal::new(155, 0)).unwrap(),
///     low: Price::new(Decimal::new(148, 0)).unwrap(),
///     close: Price::new(Decimal::new(152, 0)).unwrap(),
///     volume: 1_000_000,
/// };
///
/// assert!(validate_candle(&candle).is_ok());
/// ```
pub fn validate_candle(candle: &Candle) -> Result<()> {
    // High must be >= all other prices
    if candle.high < candle.open {
        return Err(PatternError::InvalidCandleData(format!(
            "High ({}) < Open ({})",
            candle.high, candle.open
        ))
        .into());
    }
    if candle.high < candle.close {
        return Err(PatternError::InvalidCandleData(format!(
            "High ({}) < Close ({})",
            candle.high, candle.close
        ))
        .into());
    }
    if candle.high < candle.low {
        return Err(PatternError::InvalidCandleData(format!(
            "High ({}) < Low ({})",
            candle.high, candle.low
        ))
        .into());
    }

    // Low must be <= all other prices
    if candle.low > candle.open {
        return Err(PatternError::InvalidCandleData(format!(
            "Low ({}) > Open ({})",
            candle.low, candle.open
        ))
        .into());
    }
    if candle.low > candle.close {
        return Err(PatternError::InvalidCandleData(format!(
            "Low ({}) > Close ({})",
            candle.low, candle.close
        ))
        .into());
    }

    // Volume must be non-negative (u64 is always non-negative, but check for reasonableness)
    if candle.volume == 0 {
        // Zero volume is allowed but will be flagged as an anomaly
    }

    Ok(())
}

/// Detect anomalies in candle data
///
/// Compares current candle with previous candle to detect unusual patterns
/// such as large price jumps or zero volume.
///
/// # Examples
///
/// ```
/// use daytrader::data::validation::detect_anomalies;
/// use daytrader::events::types::Candle;
/// use daytrader::brokers::types::{Symbol, Price};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let prev_candle = Candle {
///     symbol: Symbol::new("AAPL"),
///     timestamp: Utc::now(),
///     open: Price::new(Decimal::new(150, 0)).unwrap(),
///     high: Price::new(Decimal::new(155, 0)).unwrap(),
///     low: Price::new(Decimal::new(148, 0)).unwrap(),
///     close: Price::new(Decimal::new(152, 0)).unwrap(),
///     volume: 1_000_000,
/// };
///
/// let current_candle = Candle {
///     symbol: Symbol::new("AAPL"),
///     timestamp: Utc::now(),
///     open: Price::new(Decimal::new(152, 0)).unwrap(),
///     high: Price::new(Decimal::new(154, 0)).unwrap(),
///     low: Price::new(Decimal::new(151, 0)).unwrap(),
///     close: Price::new(Decimal::new(153, 0)).unwrap(),
///     volume: 1_000_000,
/// };
///
/// let anomalies = detect_anomalies(&current_candle, Some(&prev_candle));
/// assert!(anomalies.is_empty());
/// ```
pub fn detect_anomalies(candle: &Candle, prev_candle: Option<&Candle>) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    // Check for zero volume
    if candle.volume == 0 {
        anomalies.push(Anomaly::ZeroVolume);
    }

    // Compare with previous candle if available
    if let Some(prev) = prev_candle {
        // Check for unrealistic price jumps (>50%)
        let price_change = (candle.close.value() - prev.close.value()).abs();
        let change_percent = if prev.close.value() != Decimal::ZERO {
            (price_change / prev.close.value()) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let threshold = Decimal::from(50); // 50% threshold
        if change_percent > threshold {
            anomalies.push(Anomaly::UnrealisticPriceJump {
                change_percent,
                threshold,
            });
        }
    }

    anomalies
}

/// Validate quote data
///
/// Ensures bid < ask and sizes are positive.
///
/// # Examples
///
/// ```
/// use daytrader::data::validation::validate_quote;
/// use daytrader::events::types::Quote;
/// use daytrader::brokers::types::{Price, Quantity};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let quote = Quote {
///     timestamp: Utc::now(),
///     bid: Price::new(Decimal::new(15000, 2)).unwrap(),
///     ask: Price::new(Decimal::new(15010, 2)).unwrap(),
///     bid_size: Quantity::new(100),
///     ask_size: Quantity::new(200),
/// };
///
/// assert!(validate_quote(&quote).is_ok());
/// ```
pub fn validate_quote(quote: &Quote) -> Result<()> {
    // Bid must be less than ask
    if quote.bid >= quote.ask {
        return Err(PatternError::InvalidCandleData(format!(
            "Bid ({}) >= Ask ({})",
            quote.bid, quote.ask
        ))
        .into());
    }

    // Sizes are already validated by Quantity type (must be positive)

    Ok(())
}

/// Detect anomalies in quote data
///
/// Checks for unusually wide spreads.
pub fn detect_quote_anomalies(quote: &Quote) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    // Check for wide spread (> 2% of mid price)
    let spread = quote.spread();
    let mid = quote.mid_price();
    let spread_percent = if mid != Decimal::ZERO {
        (spread / mid) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    let threshold = Decimal::from(2); // 2% threshold
    if spread_percent > threshold {
        anomalies.push(Anomaly::WideSpread { spread, threshold });
    }

    anomalies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Price, Quantity, Symbol};
    use chrono::Utc;

    fn create_test_candle(open: i64, high: i64, low: i64, close: i64, volume: u64) -> Candle {
        Candle {
            symbol: Symbol::new("TEST").unwrap(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(open, 0)).unwrap(),
            high: Price::new(Decimal::new(high, 0)).unwrap(),
            low: Price::new(Decimal::new(low, 0)).unwrap(),
            close: Price::new(Decimal::new(close, 0)).unwrap(),
            volume,
        }
    }

    #[test]
    fn test_valid_candle() {
        let candle = create_test_candle(100, 110, 95, 105, 1000);
        assert!(validate_candle(&candle).is_ok());
    }

    #[test]
    fn test_invalid_high_below_open() {
        let candle = create_test_candle(110, 100, 95, 105, 1000);
        assert!(validate_candle(&candle).is_err());
    }

    #[test]
    fn test_invalid_low_above_close() {
        let candle = create_test_candle(100, 110, 106, 105, 1000);
        assert!(validate_candle(&candle).is_err());
    }

    #[test]
    fn test_zero_volume_anomaly() {
        let candle = create_test_candle(100, 110, 95, 105, 0);
        let anomalies = detect_anomalies(&candle, None);
        assert_eq!(anomalies.len(), 1);
        assert!(matches!(anomalies[0], Anomaly::ZeroVolume));
    }

    #[test]
    fn test_price_jump_anomaly() {
        let prev = create_test_candle(100, 110, 95, 105, 1000);
        let current = create_test_candle(200, 210, 195, 205, 1000);
        let anomalies = detect_anomalies(&current, Some(&prev));
        assert!(!anomalies.is_empty());
        assert!(matches!(anomalies[0], Anomaly::UnrealisticPriceJump { .. }));
    }

    #[test]
    fn test_valid_quote() {
        let quote = Quote {
            timestamp: Utc::now(),
            bid: Price::new(Decimal::new(10000, 2)).unwrap(),
            ask: Price::new(Decimal::new(10010, 2)).unwrap(),
            bid_size: Quantity::new(100).unwrap(),
            ask_size: Quantity::new(200).unwrap(),
        };
        assert!(validate_quote(&quote).is_ok());
    }

    #[test]
    fn test_invalid_quote_bid_above_ask() {
        let quote = Quote {
            timestamp: Utc::now(),
            bid: Price::new(Decimal::new(10010, 2)).unwrap(),
            ask: Price::new(Decimal::new(10000, 2)).unwrap(),
            bid_size: Quantity::new(100).unwrap(),
            ask_size: Quantity::new(200).unwrap(),
        };
        assert!(validate_quote(&quote).is_err());
    }

    #[test]
    fn test_wide_spread_anomaly() {
        let quote = Quote {
            timestamp: Utc::now(),
            bid: Price::new(Decimal::new(10000, 2)).unwrap(),
            ask: Price::new(Decimal::new(10500, 2)).unwrap(), // 5% spread
            bid_size: Quantity::new(100).unwrap(),
            ask_size: Quantity::new(200).unwrap(),
        };
        let anomalies = detect_quote_anomalies(&quote);
        assert!(!anomalies.is_empty());
        assert!(matches!(anomalies[0], Anomaly::WideSpread { .. }));
    }
}
