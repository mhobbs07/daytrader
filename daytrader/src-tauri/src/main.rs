// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod events;

use daytrader::events::bus::EventBus;
use daytrader::events::types::{MarketEvent, PatternSignal};
use daytrader::brokers::mock::MockBroker;
use daytrader::patterns::registry::PatternRegistry;
use daytrader::patterns::{
    HammerDetector, ShootingStarDetector, DojiDetector,
    BullishEngulfingDetector, BearishEngulfingDetector,
    MorningStarDetector, EveningStarDetector,
    ThreeWhiteSoldiersDetector, ThreeBlackCrowsDetector,
};
use daytrader::config::AppConfig;
use daytrader::risk::RiskEngine;
use daytrader::risk::types::AccountId;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

fn main() {
    // Load configuration
    let config = AppConfig::from_file("../../config/default.toml")
        .unwrap_or_else(|_| {
            eprintln!("Failed to load config, using defaults");
            // Create a basic config for now
            AppConfig {
                broker: daytrader::config::BrokerConfig {
                    provider: "mock".to_string(),
                    is_paper: true,
                    api_key: "".to_string(),
                    api_secret: "".to_string(),
                },
                database: daytrader::config::DatabaseConfig {
                    path: "data/db/daytrader.db".into(),
                    max_connections: 10,
                },
                logging: daytrader::config::LoggingConfig {
                    level: "info".to_string(),
                    file_path: "data/logs/daytrader.log".into(),
                },
                risk: daytrader::config::RiskConfig {
                    max_daily_loss_dollars: 500.0,
                    max_daily_loss_percent: 2.0,
                    halt_trading_on_limit: true,
                    max_position_size_dollars: 5000.0,
                    max_position_size_percent: 10.0,
                    max_concurrent_positions: 5,
                    enforce_pdt_rule: true,
                    pdt_min_equity: 25000.0,
                    max_day_trades_under_pdt: 3,
                    max_trades_per_pattern_per_day: 5,
                    stop_pattern_after_consecutive_losses: 3,
                    required_win_rate_for_auto: 60.0,
                    min_pattern_sample_size: 20,
                    min_account_balance: 1000.0,
                    emergency_liquidate_threshold: 500.0,
                    pattern_profiles: std::collections::HashMap::new(),
                    confidence_multipliers: vec![],
                    pattern_performance_thresholds: Default::default(),
                },
            }
        });

    // Initialize existing backend systems
    let event_bus = EventBus::new(10000);
    let broker = Arc::new(MockBroker::default());

    // Initialize pattern registry with all implemented patterns
    let mut pattern_registry = PatternRegistry::new();

    // Register basic candlestick patterns
    pattern_registry.register("hammer", Box::new(HammerDetector::new())).unwrap();
    pattern_registry.register("shooting_star", Box::new(ShootingStarDetector::new())).unwrap();
    pattern_registry.register("doji", Box::new(DojiDetector::new())).unwrap();

    // Register engulfing patterns
    pattern_registry.register("bullish_engulfing", Box::new(BullishEngulfingDetector::new())).unwrap();
    pattern_registry.register("bearish_engulfing", Box::new(BearishEngulfingDetector::new())).unwrap();

    // Register multi-candle patterns
    pattern_registry.register("morning_star", Box::new(MorningStarDetector::new())).unwrap();
    pattern_registry.register("evening_star", Box::new(EveningStarDetector::new())).unwrap();
    pattern_registry.register("three_white_soldiers", Box::new(ThreeWhiteSoldiersDetector::new())).unwrap();
    pattern_registry.register("three_black_crows", Box::new(ThreeBlackCrowsDetector::new())).unwrap();

    let pattern_registry = Arc::new(pattern_registry);

    // Create RiskEngine
    let account_id = AccountId::new("demo-account");
    let initial_equity = Decimal::new(5000000, 2); // $50,000.00
    let risk_engine = Arc::new(RiskEngine::new(config.risk, Arc::clone(&event_bus), account_id, initial_equity, Arc::clone(&broker)));

    // Create shared caches
    let market_cache = Arc::new(std::sync::Mutex::new(state::MarketDataCache::new()));
    let pattern_cache = Arc::new(std::sync::Mutex::new(state::PatternCache::new()));

    // Create application state
    let app_state = state::AppState::new_with_caches(
        event_bus.clone(),
        broker,
        pattern_registry.clone(),
        Arc::clone(&market_cache),
        Arc::clone(&pattern_cache),
        Arc::clone(&risk_engine)
    );

    // Start pattern detection service
    let pattern_detection_handle = {
        let event_bus = event_bus.clone();
        let pattern_registry = pattern_registry.clone();
        let market_cache = Arc::clone(&market_cache);

        tokio::spawn(async move {
            let mut rx = event_bus.subscribe();

            info!("Pattern detection service started");

            while let Ok(event) = rx.recv().await {
                if let MarketEvent::NewCandle { symbol, candle } = event {
                    // Get recent candles for pattern detection
                    if let Ok(mut cache) = market_cache.lock() {
                        if let Ok(recent_candles) = cache.get_recent_candles(&symbol, 50) {
                            // Run pattern detection on the recent candles
                            match pattern_registry.detect_all(&recent_candles, None).await {
                                patterns => {
                                    for pattern_result in patterns {
                                        // Create pattern signal
                                        let signal = PatternSignal {
                                            symbol: symbol.clone(),
                                            pattern_name: pattern_result.pattern_name,
                                            signal_type: pattern_result.signal_type,
                                            confidence: pattern_result.confidence,
                                            detected_at: pattern_result.detected_at,
                                            price: pattern_result.price_level,
                                        };

                                        // Publish pattern detected event
                                        let _ = event_bus.publish(MarketEvent::PatternDetected { signal });

                                        debug!("Pattern detected: {} for {} with confidence {:.2}",
                                               pattern_result.pattern_name.as_str(),
                                               symbol.as_str(),
                                               pattern_result.confidence.value());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            info!("Pattern detection service stopped");
        })
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // Start RiskEngine
            let risk_engine_clone = Arc::clone(&risk_engine);
            tokio::spawn(async move {
                if let Err(e) = risk_engine_clone.start().await {
                    eprintln!("Failed to start risk engine: {}", e);
                }
            });

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
            // New PDT-specific commands
            commands::risk::get_pdt_status,
            commands::risk::get_day_trade_history,
            commands::risk::check_pdt_eligibility,
            commands::risk::get_pdt_violations,
            commands::risk::get_business_day_info,
            commands::system::get_system_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
