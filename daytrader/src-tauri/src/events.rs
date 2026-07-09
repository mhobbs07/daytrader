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
            loop {
                match rx.blocking_recv() {
                    Ok(event) => {
                        let _ =
                            Self::forward_event(&app_handle, event, &market_cache, &pattern_cache);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                        // Skip ahead — exiting here would freeze all frontend
                        // updates for the rest of the session
                        tracing::warn!("Event bridge lagged, skipped {missed} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
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
                // Update cache (Candle is one type shared by brokers and events)
                if let Ok(mut cache) = market_cache.lock() {
                    cache.add_candle(symbol.to_string(), (*candle).clone());
                }
                // Emit to frontend
                app_handle.emit("market_data:new_candle", &*candle)?;
            }
            MarketEvent::QuoteUpdate { symbol, quote } => {
                // Convert events::Quote to brokers::Quote for cache.
                // events::Quote carries no last-trade price; use the bid/ask
                // midpoint as the documented proxy rather than the bid, which
                // downstream consumers would mistake for a real trade price.
                let last =
                    daytrader::brokers::types::Price::new(quote.mid_price()).unwrap_or(quote.bid);
                let broker_quote = daytrader::brokers::traits::Quote {
                    symbol: symbol.clone(),
                    bid: quote.bid,
                    ask: quote.ask,
                    last,
                    bid_size: quote.bid_size.value(),
                    ask_size: quote.ask_size.value(),
                    timestamp: quote.timestamp,
                };
                // Emit to frontend with the symbol included (the inner Quote
                // struct has no symbol field, which the frontend keys on)
                let payload = crate::commands::market_data::QuoteResponse::from(&broker_quote);
                // Update cache
                if let Ok(mut cache) = market_cache.lock() {
                    cache.update_quote(symbol.to_string(), broker_quote);
                }
                app_handle.emit("market_data:quote_update", payload)?;
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
