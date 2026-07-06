//! In-memory cache for market data
//!
//! This module provides a thread-safe, concurrent cache for market data using
//! DashMap. The cache maintains recent candles and latest quotes per symbol.

use crate::brokers::types::Symbol;
use crate::events::types::{Candle, Quote};
use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe in-memory cache for market data
///
/// Uses DashMap for concurrent access without locks. Maintains a bounded
/// number of recent candles per symbol and the latest quote.
///
/// # Examples
///
/// ```
/// use daytrader::data::cache::MarketDataCache;
/// use daytrader::events::types::Candle;
/// use daytrader::brokers::types::{Symbol, Price};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
/// use std::sync::Arc;
///
/// let cache = MarketDataCache::new(100);
/// let symbol = Symbol::new("AAPL");
///
/// let candle = Arc::new(Candle {
///     symbol: symbol.clone(),
///     timestamp: Utc::now(),
///     open: Price::new(Decimal::new(150, 0)).unwrap(),
///     high: Price::new(Decimal::new(155, 0)).unwrap(),
///     low: Price::new(Decimal::new(148, 0)).unwrap(),
///     close: Price::new(Decimal::new(152, 0)).unwrap(),
///     volume: 1_000_000,
/// });
///
/// cache.add_candle(symbol.clone(), candle);
/// let recent = cache.get_candles(&symbol, 10);
/// assert_eq!(recent.len(), 1);
/// ```
#[derive(Clone)]
pub struct MarketDataCache {
    /// Recent candles per symbol (bounded by max_candles)
    candles: Arc<DashMap<Symbol, Vec<Arc<Candle>>>>,

    /// Latest quote per symbol
    quotes: Arc<DashMap<Symbol, Quote>>,

    /// Maximum candles to keep per symbol
    max_candles: usize,
}

impl MarketDataCache {
    /// Create a new market data cache
    ///
    /// # Arguments
    ///
    /// * `max_candles` - Maximum number of candles to keep per symbol
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::data::cache::MarketDataCache;
    ///
    /// let cache = MarketDataCache::new(100);
    /// ```
    pub fn new(max_candles: usize) -> Self {
        Self {
            candles: Arc::new(DashMap::new()),
            quotes: Arc::new(DashMap::new()),
            max_candles,
        }
    }

    /// Add a candle to the cache
    ///
    /// Maintains the size limit by removing oldest candles when necessary.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol for the candle
    /// * `candle` - Candle data wrapped in Arc for zero-copy sharing
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::data::cache::MarketDataCache;
    /// use daytrader::events::types::Candle;
    /// use daytrader::brokers::types::{Symbol, Price};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    /// use std::sync::Arc;
    ///
    /// let cache = MarketDataCache::new(100);
    /// let symbol = Symbol::new("AAPL");
    /// let candle = Arc::new(Candle {
    ///     symbol: symbol.clone(),
    ///     timestamp: Utc::now(),
    ///     open: Price::new(Decimal::new(150, 0)).unwrap(),
    ///     high: Price::new(Decimal::new(155, 0)).unwrap(),
    ///     low: Price::new(Decimal::new(148, 0)).unwrap(),
    ///     close: Price::new(Decimal::new(152, 0)).unwrap(),
    ///     volume: 1_000_000,
    /// });
    ///
    /// cache.add_candle(symbol, candle);
    /// ```
    pub fn add_candle(&self, symbol: Symbol, candle: Arc<Candle>) {
        self.candles
            .entry(symbol)
            .and_modify(|candles| {
                candles.push(candle.clone());
                // Maintain size limit
                if candles.len() > self.max_candles {
                    candles.remove(0);
                }
            })
            .or_insert_with(|| vec![candle]);
    }

    /// Get recent candles for a symbol
    ///
    /// Returns up to `count` most recent candles in reverse chronological order
    /// (newest first).
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get candles for
    /// * `count` - Maximum number of candles to return
    ///
    /// # Returns
    ///
    /// Vector of candles (newest first), or empty vector if symbol not found
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::data::cache::MarketDataCache;
    /// use daytrader::brokers::types::Symbol;
    ///
    /// let cache = MarketDataCache::new(100);
    /// let symbol = Symbol::new("AAPL");
    /// let recent = cache.get_candles(&symbol, 10);
    /// ```
    pub fn get_candles(&self, symbol: &Symbol, count: usize) -> Vec<Arc<Candle>> {
        self.candles
            .get(symbol)
            .map(|candles| candles.iter().rev().take(count).cloned().collect())
            .unwrap_or_default()
    }

    /// Get all candles for a symbol
    ///
    /// Returns all cached candles in chronological order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get candles for
    ///
    /// # Returns
    ///
    /// Vector of candles (oldest first), or empty vector if symbol not found
    pub fn get_all_candles(&self, symbol: &Symbol) -> Vec<Arc<Candle>> {
        self.candles
            .get(symbol)
            .map(|candles| candles.clone())
            .unwrap_or_default()
    }

    /// Update the latest quote for a symbol
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol for the quote
    /// * `quote` - Quote data
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::data::cache::MarketDataCache;
    /// use daytrader::events::types::Quote;
    /// use daytrader::brokers::types::{Symbol, Price, Quantity};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    ///
    /// let cache = MarketDataCache::new(100);
    /// let symbol = Symbol::new("AAPL");
    /// let quote = Quote {
    ///     timestamp: Utc::now(),
    ///     bid: Price::new(Decimal::new(15000, 2)).unwrap(),
    ///     ask: Price::new(Decimal::new(15010, 2)).unwrap(),
    ///     bid_size: Quantity::new(100),
    ///     ask_size: Quantity::new(200),
    /// };
    ///
    /// cache.update_quote(symbol, quote);
    /// ```
    pub fn update_quote(&self, symbol: Symbol, quote: Quote) {
        self.quotes.insert(symbol, quote);
    }

    /// Get the latest quote for a symbol
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get quote for
    ///
    /// # Returns
    ///
    /// Latest quote if available, None otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use daytrader::data::cache::MarketDataCache;
    /// use daytrader::brokers::types::Symbol;
    ///
    /// let cache = MarketDataCache::new(100);
    /// let symbol = Symbol::new("AAPL");
    /// let quote = cache.get_quote(&symbol);
    /// assert!(quote.is_none());
    /// ```
    pub fn get_quote(&self, symbol: &Symbol) -> Option<Quote> {
        self.quotes.get(symbol).map(|q| q.clone())
    }

    /// Get the number of symbols with cached candles
    pub fn candle_symbol_count(&self) -> usize {
        self.candles.len()
    }

    /// Get the number of symbols with cached quotes
    pub fn quote_symbol_count(&self) -> usize {
        self.quotes.len()
    }

    /// Clear all cached data
    pub fn clear(&self) {
        self.candles.clear();
        self.quotes.clear();
    }

    /// Clear cached data for a specific symbol
    pub fn clear_symbol(&self, symbol: &Symbol) {
        self.candles.remove(symbol);
        self.quotes.remove(symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Price, Quantity};
    use chrono::Utc;
    use rust_decimal::Decimal;

    fn create_test_candle(symbol: Symbol, close: i64) -> Arc<Candle> {
        Arc::new(Candle {
            symbol,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(close - 1, 0)).unwrap(),
            high: Price::new(Decimal::new(close + 1, 0)).unwrap(),
            low: Price::new(Decimal::new(close - 2, 0)).unwrap(),
            close: Price::new(Decimal::new(close, 0)).unwrap(),
            volume: 1000,
        })
    }

    #[test]
    fn test_add_and_get_candles() {
        let cache = MarketDataCache::new(100);
        let symbol = Symbol::new("AAPL").unwrap();

        let candle1 = create_test_candle(symbol.clone(), 100);
        let candle2 = create_test_candle(symbol.clone(), 101);

        cache.add_candle(symbol.clone(), candle1);
        cache.add_candle(symbol.clone(), candle2);

        let recent = cache.get_candles(&symbol, 10);
        assert_eq!(recent.len(), 2);
        // Should be in reverse order (newest first)
        assert_eq!(recent[0].close.value(), Decimal::new(101, 0));
        assert_eq!(recent[1].close.value(), Decimal::new(100, 0));
    }

    #[test]
    fn test_candle_size_limit() {
        let cache = MarketDataCache::new(5);
        let symbol = Symbol::new("AAPL").unwrap();

        // Add 10 candles
        for i in 0..10 {
            let candle = create_test_candle(symbol.clone(), 100 + i);
            cache.add_candle(symbol.clone(), candle);
        }

        let all = cache.get_all_candles(&symbol);
        assert_eq!(all.len(), 5); // Should only keep 5

        // Should have the most recent 5
        assert_eq!(all[0].close.value(), Decimal::new(105, 0));
        assert_eq!(all[4].close.value(), Decimal::new(109, 0));
    }

    #[test]
    fn test_quote_operations() {
        let cache = MarketDataCache::new(100);
        let symbol = Symbol::new("AAPL").unwrap();

        let quote = Quote {
            timestamp: Utc::now(),
            bid: Price::new(Decimal::new(15000, 2)).unwrap(),
            ask: Price::new(Decimal::new(15010, 2)).unwrap(),
            bid_size: Quantity::new(100).unwrap(),
            ask_size: Quantity::new(200).unwrap(),
        };

        cache.update_quote(symbol.clone(), quote.clone());

        let retrieved = cache.get_quote(&symbol);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.bid, quote.bid);
        assert_eq!(retrieved.ask, quote.ask);
    }

    #[test]
    fn test_clear_operations() {
        let cache = MarketDataCache::new(100);
        let symbol1 = Symbol::new("AAPL").unwrap();
        let symbol2 = Symbol::new("GOOGL").unwrap();

        cache.add_candle(symbol1.clone(), create_test_candle(symbol1.clone(), 100));
        cache.add_candle(symbol2.clone(), create_test_candle(symbol2.clone(), 200));

        assert_eq!(cache.candle_symbol_count(), 2);

        cache.clear_symbol(&symbol1);
        assert_eq!(cache.candle_symbol_count(), 1);
        assert!(cache.get_candles(&symbol1, 10).is_empty());

        cache.clear();
        assert_eq!(cache.candle_symbol_count(), 0);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let cache = MarketDataCache::new(100);
        let symbol = Symbol::new("AAPL").unwrap();

        let cache_clone = cache.clone();
        let symbol_clone = symbol.clone();
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let candle = create_test_candle(symbol_clone.clone(), 100 + i);
                cache_clone.add_candle(symbol_clone.clone(), candle);
            }
        });

        for i in 50..100 {
            let candle = create_test_candle(symbol.clone(), 100 + i);
            cache.add_candle(symbol.clone(), candle);
        }

        handle.join().unwrap();

        let all = cache.get_all_candles(&symbol);
        assert_eq!(all.len(), 100);
    }
}
