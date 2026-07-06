use daytrader::events::{bus::EventBus, types::MarketEvent};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, Emitter};

/// Bridges Rust event bus to Tauri frontend events
pub struct EventBridge {
    app_handle: AppHandle,
    event_bus: EventBus,
    market_cache: Arc<std::sync::Mutex<crate::state::MarketDataCache>>,
    pattern_cache: Arc<std::sync::Mutex<crate::state::PatternCache>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl EventBridge {
    pub fn new(
        app_handle: AppHandle,
        event_bus: EventBus,
        market_cache: Arc<std::sync::Mutex<crate::state::MarketDataCache>>,
        pattern_cache: Arc<std::sync::Mutex<crate::state::PatternCache>>,
    ) -> Self {
        Self {
            app_handle,
            event_bus,
            market_cache,
            pattern_cache,
            thread_handle: None,
        }
    }

    /// Start listening to backend events and forward to frontend
    pub fn start(&mut self) {
        let mut rx = self.event_bus.subscribe();
        let app_handle = self.app_handle.clone();
        let market_cache = Arc::clone(&self.market_cache);
        let pattern_cache = Arc::clone(&self.pattern_cache);

        let handle = thread::spawn(move || {
            while let Ok(event) = rx.blocking_recv() {
                let _ = Self::forward_event(&app_handle, event, &market_cache, &pattern_cache);
            }
        });

        self.thread_handle = Some(handle);
    }

    fn forward_event(
        app_handle: &AppHandle,
        event: MarketEvent,
        market_cache: &Arc<std::sync::Mutex<crate::state::MarketDataCache>>,
        pattern_cache: &Arc<std::sync::Mutex<crate::state::PatternCache>>,
    ) -> Result<(), tauri::Error> {
        match event {
            MarketEvent::NewCandle { symbol, candle } => {
                // Convert events::Candle to brokers::Candle for cache
                let broker_candle = daytrader::brokers::traits::Candle {
                    symbol: candle.symbol.clone(),
                    timestamp: candle.timestamp,
                    open: candle.open,
                    high: candle.high,
                    low: candle.low,
                    close: candle.close,
                    volume: candle.volume,
                };
                // Update cache
                if let Ok(mut cache) = market_cache.lock() {
                    cache.add_candle(symbol.to_string(), broker_candle);
                }
                // Emit to frontend
                app_handle.emit("market_data:new_candle", &*candle)?;
            }
            MarketEvent::QuoteUpdate { symbol, quote } => {
                // Convert events::Quote to brokers::Quote for cache
                let broker_quote = daytrader::brokers::traits::Quote {
                    symbol: symbol.clone(),
                    bid: quote.bid,
                    ask: quote.ask,
                    last: quote.bid, // Use bid as last price for now
                    bid_size: quote.bid_size.value(),
                    ask_size: quote.ask_size.value(),
                    timestamp: quote.timestamp,
                };
                // Update cache
                if let Ok(mut cache) = market_cache.lock() {
                    cache.update_quote(symbol.to_string(), broker_quote);
                }
                // Emit to frontend
                app_handle.emit("market_data:quote_update", &quote)?;
            }
            MarketEvent::PatternDetected { signal } => {
                // Update cache
                if let Ok(mut cache) = pattern_cache.lock() {
                    cache.add_pattern(signal.clone());
                }
                // Emit to frontend
                app_handle.emit("patterns:detected", &signal)?;
            }
            MarketEvent::ConnectionEstablished { provider } => {
                app_handle.emit("system:connection", provider.to_string())?;
            }
            MarketEvent::ConnectionLost { provider, reason } => {
                app_handle.emit("system:disconnection", reason.as_str())?;
            }
            MarketEvent::RiskViolation { violation } => {
                app_handle.emit("risk:violation", &violation)?;
            }
            _ => {
                // Other events not needed for thin slice
            }
        }
        Ok(())
    }
}
