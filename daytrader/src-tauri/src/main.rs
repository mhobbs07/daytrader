// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod events;

use daytrader::events::bus::EventBus;
use daytrader::brokers::mock::MockBroker;
use daytrader::patterns::registry::PatternRegistry;
use std::sync::Arc;

fn main() {
    // Initialize existing backend systems
    let event_bus = EventBus::new(10000);
    let broker = Arc::new(MockBroker::default());
    let pattern_registry = Arc::new(PatternRegistry::new());

    // Create shared caches
    let market_cache = Arc::new(std::sync::Mutex::new(state::MarketDataCache::new()));
    let pattern_cache = Arc::new(std::sync::Mutex::new(state::PatternCache::new()));

    // Create application state
    let app_state = state::AppState::new_with_caches(
        event_bus.clone(),
        broker,
        pattern_registry,
        Arc::clone(&market_cache),
        Arc::clone(&pattern_cache)
    );

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // Start event bridge with shared caches
            let mut bridge = events::EventBridge::new(
                app.handle().clone(),
                event_bus.clone(),
                Arc::clone(&market_cache),
                Arc::clone(&pattern_cache)
            );
            bridge.start();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::market_data::get_latest_quote,
            commands::market_data::get_recent_candles,
            commands::market_data::subscribe_symbol,
            commands::patterns::get_detected_patterns_for_symbol,
            commands::patterns::get_pattern_statistics,
            commands::risk::get_risk_state,
            commands::risk::get_risk_limits,
            commands::risk::get_recent_risk_violations,
            commands::system::get_system_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
