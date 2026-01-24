use daytrader::{
    brokers::{traits::MarketDataProvider, Symbol, Candle, Quote},
    events::bus::EventBus,
    patterns::PatternRegistry,
    events::types::PatternSignal,
    risk::RiskEngine,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;

// Simplified cache implementations for thin slice
pub struct MarketDataCache {
    quotes: HashMap<String, Quote>,
    candles: HashMap<String, Vec<Candle>>,
}

impl MarketDataCache {
    pub fn new() -> Self {
        Self {
            quotes: HashMap::new(),
            candles: HashMap::new(),
        }
    }

    pub fn get_latest_quote(&self, symbol: &Symbol) -> Option<Quote> {
        self.quotes.get(symbol.as_str()).cloned()
    }

    pub fn get_recent_candles(&self, symbol: &Symbol, count: usize) -> Result<Vec<Candle>, String> {
        if let Some(candles) = self.candles.get(symbol.as_str()) {
            Ok(candles.iter().rev().take(count).cloned().collect::<Vec<_>>().into_iter().rev().collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub fn update_quote(&mut self, symbol: String, quote: Quote) {
        self.quotes.insert(symbol, quote);
    }

    pub fn add_candle(&mut self, symbol: String, candle: Candle) {
        self.candles.entry(symbol.clone()).or_insert_with(Vec::new).push(candle);
        // Keep only last 100 candles per symbol
        if let Some(candles) = self.candles.get_mut(&symbol) {
            if candles.len() > 100 {
                candles.remove(0);
            }
        }
    }
}

pub struct PatternCache {
    patterns: Vec<PatternSignal>,
}

impl PatternCache {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn get_patterns_for_symbol(&self, symbol: &str) -> Result<Vec<PatternSignal>, String> {
        Ok(self.patterns.iter().filter(|p| p.symbol.as_str() == symbol).cloned().collect())
    }

    pub fn add_pattern(&mut self, signal: PatternSignal) {
        self.patterns.push(signal);
        // Keep only last 100 patterns
        if self.patterns.len() > 100 {
            self.patterns.remove(0);
        }
    }
}

pub struct AppState {
    pub event_bus: EventBus,
    pub broker: Arc<dyn MarketDataProvider>,
    pub broker_connected: Arc<AtomicBool>,
    pub pattern_registry: Arc<PatternRegistry>,
    pub market_cache: Arc<std::sync::Mutex<MarketDataCache>>,
    pub pattern_cache: Arc<std::sync::Mutex<PatternCache>>,
    pub risk_engine: Arc<RiskEngine>,
}

impl AppState {
    pub fn new(
        event_bus: EventBus,
        broker: Arc<dyn MarketDataProvider>,
        pattern_registry: Arc<PatternRegistry>,
        risk_engine: Arc<RiskEngine>,
    ) -> Self {
        Self {
            event_bus,
            broker,
            broker_connected: Arc::new(AtomicBool::new(false)),
            pattern_registry,
            market_cache: Arc::new(std::sync::Mutex::new(MarketDataCache::new())),
            pattern_cache: Arc::new(std::sync::Mutex::new(PatternCache::new())),
            risk_engine,
        }
    }

    pub fn new_with_caches(
        event_bus: EventBus,
        broker: Arc<dyn MarketDataProvider>,
        pattern_registry: Arc<PatternRegistry>,
        market_cache: Arc<std::sync::Mutex<MarketDataCache>>,
        pattern_cache: Arc<std::sync::Mutex<PatternCache>>,
        risk_engine: Arc<RiskEngine>,
    ) -> Self {
        Self {
            event_bus,
            broker,
            broker_connected: Arc::new(AtomicBool::new(false)),
            pattern_registry,
            market_cache,
            pattern_cache,
            risk_engine,
        }
    }
}