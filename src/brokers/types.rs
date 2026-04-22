//! Domain types using the newtype pattern for type safety
//!
//! This module provides strongly-typed wrappers around primitive types
//! to prevent errors like passing a price where a quantity is expected.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Newtype for stock symbols
///
/// Ensures symbols are validated and normalized (uppercase).
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Symbol;
///
/// let symbol = Symbol::new("AAPL").unwrap();
/// assert_eq!(symbol.as_str(), "AAPL");
///
/// // Invalid symbols are rejected
/// assert!(Symbol::new("").is_err());
/// assert!(Symbol::new("TOOLONGSYMBOL").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(String);

impl Symbol {
    /// Creates a new symbol, validating and normalizing to uppercase
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol is empty or longer than 10 characters
    pub fn new(s: impl Into<String>) -> Result<Self, &'static str> {
        let s = s.into();
        if s.is_empty() {
            return Err("Symbol cannot be empty");
        }
        if s.len() > 10 {
            return Err("Symbol cannot be longer than 10 characters");
        }
        Ok(Self(s.to_uppercase()))
    }

    /// Returns the symbol as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Newtype for prices (always positive)
///
/// Ensures prices are always positive and uses Decimal for precision.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Price;
/// use rust_decimal::Decimal;
///
/// let price = Price::new(Decimal::new(15050, 2)).unwrap(); // $150.50
/// assert_eq!(price.value(), Decimal::new(15050, 2));
///
/// // Negative or zero prices are rejected
/// assert!(Price::new(Decimal::ZERO).is_err());
/// assert!(Price::new(Decimal::new(-100, 0)).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Price(Decimal);

impl Price {
    /// Creates a new price
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not positive
    #[inline]
    pub fn new(value: Decimal) -> Result<Self, &'static str> {
        if value <= Decimal::ZERO {
            return Err("Price must be positive");
        }
        Ok(Self(value))
    }

    /// Returns the price value
    #[inline]
    pub fn value(&self) -> Decimal {
        self.0
    }

    /// Returns the price as f64 (use sparingly, prefer Decimal)
    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0.to_string().parse().unwrap_or(0.0)
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// Newtype for quantities (always positive)
///
/// Ensures quantities are always positive integers.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Quantity;
///
/// let qty = Quantity::new(100).unwrap();
/// assert_eq!(qty.value(), 100);
///
/// // Zero quantity is rejected
/// assert!(Quantity::new(0).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Quantity(u32);

impl Quantity {
    /// Creates a new quantity
    ///
    /// # Errors
    ///
    /// Returns an error if the value is zero
    #[inline]
    pub fn new(value: u32) -> Result<Self, &'static str> {
        if value == 0 {
            return Err("Quantity must be positive");
        }
        Ok(Self(value))
    }

    /// Returns the quantity value
    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for confidence scores (0.0 to 1.0)
///
/// Ensures confidence scores are within valid range.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Confidence;
///
/// let conf = Confidence::new(0.75).unwrap();
/// assert_eq!(conf.value(), 0.75);
///
/// // Out of range values are rejected
/// assert!(Confidence::new(-0.1).is_err());
/// assert!(Confidence::new(1.5).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    /// Creates a new confidence score
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not between 0.0 and 1.0 (inclusive)
    #[inline]
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !(0.0..=1.0).contains(&value) {
            return Err("Confidence must be between 0.0 and 1.0");
        }
        Ok(Self(value))
    }

    /// Returns the confidence value
    #[inline]
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Returns the confidence as a percentage (0-100)
    #[inline]
    pub fn as_percentage(&self) -> f64 {
        self.0 * 100.0
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percentage())
    }
}

/// Newtype for order IDs
///
/// Wraps order IDs from broker APIs.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::OrderId;
///
/// let id = OrderId::new("order-123");
/// assert_eq!(id.as_str(), "order-123");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(String);

impl OrderId {
    /// Creates a new order ID
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the order ID as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for OrderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Pattern name (e.g., "Hammer", "Doji", "Bullish Engulfing")
///
/// Wraps pattern names for type safety.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::PatternName;
///
/// let pattern = PatternName::new("Hammer");
/// assert_eq!(pattern.as_str(), "Hammer");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatternName(String);

impl PatternName {
    /// Creates a new pattern name
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the pattern name as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for PatternName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for PatternName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Reason for an action (cancellation, rejection, halt, etc.)
///
/// Wraps reason strings for type safety.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Reason;
///
/// let reason = Reason::new("Insufficient funds");
/// assert_eq!(reason.as_str(), "Insufficient funds");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reason(String);

impl Reason {
    /// Creates a new reason
    #[inline]
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }

    /// Returns the reason as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Reason {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Warning or informational message
///
/// Wraps message strings for type safety.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Message;
///
/// let msg = Message::new("Position size approaching limit");
/// assert_eq!(msg.as_str(), "Position size approaching limit");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message(String);

impl Message {
    /// Creates a new message
    #[inline]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// Returns the message as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Message {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Broker provider name
///
/// Enum for known broker providers with a Custom variant for others.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::BrokerProvider;
///
/// let provider = BrokerProvider::Alpaca;
/// assert_eq!(provider.to_string(), "Alpaca");
///
/// let custom = BrokerProvider::Custom("TDAmeritrade".to_string());
/// assert_eq!(custom.to_string(), "TDAmeritrade");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrokerProvider {
    Alpaca,
    InteractiveBrokers,
    Mock,
    Custom(String),
}

impl BrokerProvider {
    /// Creates a new broker provider from a string
    ///
    /// Matches known providers by name (case-insensitive), otherwise creates Custom variant.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        match name.to_lowercase().as_str() {
            "alpaca" => Self::Alpaca,
            "interactive brokers" | "ibkr" | "ib" => Self::InteractiveBrokers,
            "mock" => Self::Mock,
            _ => Self::Custom(name),
        }
    }

    /// Returns the provider name as a string slice
    pub fn as_str(&self) -> &str {
        match self {
            Self::Alpaca => "Alpaca",
            Self::InteractiveBrokers => "Interactive Brokers",
            Self::Mock => "Mock",
            Self::Custom(name) => name,
        }
    }
}

impl fmt::Display for BrokerProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Optional notes or descriptions
///
/// Wraps note strings for type safety.
///
/// # Examples
///
/// ```
/// use daytrader::brokers::types::Notes;
///
/// let notes = Notes::new("Pattern detected at support level");
/// assert!(!notes.is_empty());
/// assert_eq!(notes.as_str(), "Pattern detected at support level");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notes(String);

impl Notes {
    /// Creates new notes
    #[inline]
    pub fn new(notes: impl Into<String>) -> Self {
        Self(notes.into())
    }

    /// Returns the notes as a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if the notes are empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Consumes self and returns the inner String
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Notes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Notes {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_validation() {
        assert!(Symbol::new("AAPL").is_ok());
        assert!(Symbol::new("aapl").is_ok()); // Should normalize to uppercase
        assert!(Symbol::new("").is_err());
        assert!(Symbol::new("TOOLONGSYMBOL").is_err());

        let symbol = Symbol::new("aapl").unwrap();
        assert_eq!(symbol.as_str(), "AAPL");
    }

    #[test]
    fn test_price_validation() {
        assert!(Price::new(Decimal::new(15050, 2)).is_ok());
        assert!(Price::new(Decimal::ZERO).is_err());
        assert!(Price::new(Decimal::new(-100, 0)).is_err());

        let price = Price::new(Decimal::new(15050, 2)).unwrap();
        assert_eq!(price.value(), Decimal::new(15050, 2));
    }

    #[test]
    fn test_quantity_validation() {
        assert!(Quantity::new(100).is_ok());
        assert!(Quantity::new(1).is_ok());
        assert!(Quantity::new(0).is_err());

        let qty = Quantity::new(100).unwrap();
        assert_eq!(qty.value(), 100);
    }

    #[test]
    fn test_confidence_validation() {
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(0.5).is_ok());
        assert!(Confidence::new(1.0).is_ok());
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.1).is_err());

        let conf = Confidence::new(0.75).unwrap();
        assert_eq!(conf.value(), 0.75);
        assert_eq!(conf.as_percentage(), 75.0);
    }

    #[test]
    fn test_order_id() {
        let id = OrderId::new("order-123");
        assert_eq!(id.as_str(), "order-123");
        assert_eq!(format!("{}", id), "order-123");
    }
}
