// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod mock_driver;
mod state;

use crate::mock_driver::MockDataDriver;
use daytrader::brokers::mock::{MockBroker, MockConfig};
use daytrader::brokers::traits::{MarketDataProvider, OrderExecutionProvider};
use daytrader::config::AppConfig;
use daytrader::db::Database;
use daytrader::db::repositories::{PerformanceRepository, SignalRepository};
use daytrader::events::bus::EventBus;
use daytrader::execution::engine::{ExecutionConfig, ExecutionEngine};
use daytrader::execution::strategy::{ConservativeConfig, ConservativeStrategy};
use daytrader::orders::broker_adapter::AlpacaAdapter;
use daytrader::orders::engine::OrderEngine;
use daytrader::orders::types::OrderConfig;
use daytrader::patterns::registry::PatternRegistry;
use daytrader::patterns::signals::{SignalConfig, SignalEngine};
use daytrader::patterns::streaming::PatternStreamingPipeline;
use daytrader::patterns::{
    BearishEngulfingDetector, BullishEngulfingDetector, DojiDetector, EveningStarDetector,
    HammerDetector, MorningStarDetector, ShootingStarDetector, ThreeBlackCrowsDetector,
    ThreeWhiteSoldiersDetector,
};
use daytrader::risk::RiskEngine;
use daytrader::risk::types::AccountId;
use rust_decimal::Decimal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Names under which the detectors register; also used to pre-enable
/// per-pattern automation flags (the global automation toggle stays OFF
/// until explicitly enabled from the UI).
const PATTERN_NAMES: [&str; 9] = [
    "hammer",
    "shooting_star",
    "doji",
    "bullish_engulfing",
    "bearish_engulfing",
    "morning_star",
    "evening_star",
    "three_white_soldiers",
    "three_black_crows",
];

/// Resolves a repo-relative path from either the workspace root (packaged run)
/// or `daytrader/src-tauri` (`npm run tauri dev` cwd).
fn resolve_repo_path(relative: &Path) -> PathBuf {
    if relative.exists() {
        relative.to_path_buf()
    } else {
        PathBuf::from("../..").join(relative)
    }
}

fn load_config() -> AppConfig {
    let path = resolve_repo_path(Path::new("config/default.toml"));
    match AppConfig::from_file(&path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {e} — using built-in defaults",
                path.display()
            );
            AppConfig {
                broker: daytrader::config::BrokerConfig {
                    provider: "mock".to_string(),
                    is_paper: true,
                    api_key: String::new(),
                    api_secret: String::new(),
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
        }
    }
}

fn build_pattern_registry() -> PatternRegistry {
    let mut registry = PatternRegistry::new();

    let register = |registry: &mut PatternRegistry,
                    name: &str,
                    detector: Box<dyn daytrader::patterns::PatternDetector>| {
        if let Err(e) = registry.register(name, detector) {
            error!("Failed to register pattern detector {name}: {e}");
        }
    };

    register(&mut registry, "hammer", Box::new(HammerDetector::new()));
    register(
        &mut registry,
        "shooting_star",
        Box::new(ShootingStarDetector::new()),
    );
    register(&mut registry, "doji", Box::new(DojiDetector::new()));
    register(
        &mut registry,
        "bullish_engulfing",
        Box::new(BullishEngulfingDetector::new()),
    );
    register(
        &mut registry,
        "bearish_engulfing",
        Box::new(BearishEngulfingDetector::new()),
    );
    register(
        &mut registry,
        "morning_star",
        Box::new(MorningStarDetector::new()),
    );
    register(
        &mut registry,
        "evening_star",
        Box::new(EveningStarDetector::new()),
    );
    register(
        &mut registry,
        "three_white_soldiers",
        Box::new(ThreeWhiteSoldiersDetector::new()),
    );
    register(
        &mut registry,
        "three_black_crows",
        Box::new(ThreeBlackCrowsDetector::new()),
    );

    registry
}

#[tokio::main]
async fn main() {
    // Make Tauri reuse this Tokio runtime so bare `tokio::spawn` calls in
    // library code and this binary all land on one runtime. Without a
    // runtime, the first spawn panics at startup ("no reactor running").
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    if let Err(e) = daytrader::utils::init_error_logging() {
        eprintln!("Failed to initialize logging: {e}");
    }

    let config = load_config();

    // Event bus + synthetic market data
    let event_bus = EventBus::new(10000);
    let broker = Arc::new(MockBroker::new(MockConfig {
        // Complete the demo order lifecycle: accepted orders fill on the next
        // status poll at their limit price
        auto_fill_orders: true,
        ..MockConfig::default()
    }));
    let mock_driver = Arc::new(MockDataDriver::new(event_bus.clone()));

    let pattern_registry = Arc::new(build_pattern_registry());

    // Database: required for the trading loop (signal persistence, order
    // audit). The dashboard runs display-only without it.
    let db_path = resolve_repo_path(&config.database.path);
    let database = match Database::new(&db_path).await {
        Ok(db) => Some(Arc::new(db)),
        Err(e) => {
            error!(
                "Failed to open database at {} — trading engines disabled, dashboard is display-only: {e}",
                db_path.display()
            );
            None
        }
    };

    // Risk engine (shares the database when available)
    let account_id = AccountId::new("demo-account");
    let initial_equity = Decimal::new(10000000, 2); // $100,000.00, matches MockBroker account
    let risk_engine = Arc::new(RiskEngine::with_database(
        config.risk.clone(),
        Arc::new(event_bus.clone()),
        account_id,
        initial_equity,
        database.clone(),
    ));

    // Shared caches for the UI
    let market_cache = Arc::new(std::sync::Mutex::new(state::MarketDataCache::new()));
    let pattern_cache = Arc::new(std::sync::Mutex::new(state::PatternCache::new()));

    // Trading loop: signals -> execution -> orders -> fills -> positions.
    // Wired only when the database opened; automation itself starts OFF and
    // is toggled from the UI (set_automation_enabled).
    let mut order_engine: Option<Arc<OrderEngine>> = None;
    let mut execution_engine: Option<Arc<ExecutionEngine>> = None;
    let mut pnl_positions: Option<Arc<daytrader::positions::PositionTracker>> = None;

    if let Some(db) = &database {
        let db_pool = Arc::new(db.pool.clone());

        // AlpacaAdapter is a plain passthrough over any OrderExecutionProvider;
        // with the MockBroker underneath, submitted orders land in the mock's
        // order store so the status poller can observe their fills.
        let orders = Arc::new(OrderEngine::new(
            broker.clone() as Arc<dyn OrderExecutionProvider>,
            Arc::clone(&risk_engine),
            Arc::new(AlpacaAdapter::new(
                broker.clone() as Arc<dyn OrderExecutionProvider>
            )),
            Arc::new(event_bus.clone()),
            Arc::clone(&db_pool),
            1, // account row created by scripts/setup_db.sh
            OrderConfig::default(),
        ));

        // Per-pattern automation pre-enabled; the global toggle is the single
        // safety switch and starts disabled.
        let execution_config = ExecutionConfig {
            auto_execution_enabled: false,
            pattern_auto_enabled: PATTERN_NAMES
                .iter()
                .map(|name| (name.to_string(), true))
                .collect(),
            ..ExecutionConfig::default()
        };

        // Limit orders (not market): the signal's price level becomes the
        // limit, which is also the price the mock broker fills at.
        let strategy = Arc::new(ConservativeStrategy::new(ConservativeConfig {
            use_market_orders: false,
            ..ConservativeConfig::default()
        }));

        // P&L position tracker (positions module). Fed by the fill listener
        // below; distinct from the risk engine's FIFO day-trade tracker.
        let pnl_tracker = Arc::new(daytrader::positions::PositionTracker::new(
            broker.clone() as Arc<dyn OrderExecutionProvider>,
            event_bus.clone(),
            Arc::clone(&db_pool),
            daytrader::positions::PnLConfig::default(),
            daytrader::positions::tracker::PositionConfig::default(),
        ));

        match ExecutionEngine::new(
            Arc::clone(&orders),
            Arc::clone(&pnl_tracker),
            Arc::clone(&risk_engine),
            Arc::new(event_bus.clone()),
            Arc::clone(&db_pool),
            strategy,
            execution_config,
        )
        .await
        {
            Ok(engine) => {
                let engine = Arc::new(engine);
                if let Err(e) = engine.start().await {
                    error!("Failed to start execution engine: {e}");
                } else {
                    info!("Execution engine started (automation disabled until enabled from UI)");
                    execution_engine = Some(engine);
                }
            }
            Err(e) => error!("Failed to create execution engine: {e}"),
        }

        // Poll broker order statuses so fills surface as OrderFilled events
        // (feeds the risk engine's position tracker and the UI).
        let poller = Arc::clone(&orders);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                match poller.sync_order_statuses().await {
                    Ok(0) => {}
                    Ok(n) => debug!("Order status sync applied {n} transition(s)"),
                    Err(e) => warn!("Order status sync failed: {e}"),
                }
            }
        });

        // Route fills into the P&L tracker and keep its marks fresh from
        // quotes. Lagged receivers skip ahead instead of exiting.
        {
            let event_bus = event_bus.clone();
            let orders = Arc::clone(&orders);
            let pnl_tracker = Arc::clone(&pnl_tracker);
            tokio::spawn(async move {
                let mut rx = event_bus.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(daytrader::events::types::MarketEvent::OrderFilled {
                            order_id,
                            fill,
                        }) => {
                            let order = match orders.get_order(&order_id).await {
                                Ok(order) => order,
                                Err(e) => {
                                    warn!("Filled order {order_id} not found in tracker: {e}");
                                    continue;
                                }
                            };
                            let fill_info = daytrader::orders::types::FillInfo {
                                filled_quantity: fill.filled_quantity,
                                filled_price: fill.filled_price,
                                commission: fill.commission,
                                timestamp: fill.filled_at,
                            };
                            if let Err(e) = pnl_tracker.update_from_fill(&order, &fill_info).await {
                                warn!("Failed to apply fill to position tracker: {e}");
                            }
                        }
                        Ok(daytrader::events::types::MarketEvent::QuoteUpdate {
                            symbol,
                            quote,
                        }) => {
                            // Mark to the bid/ask midpoint
                            let mid = (quote.bid.value() + quote.ask.value())
                                / rust_decimal::Decimal::TWO;
                            if let Ok(price) = daytrader::brokers::types::Price::new(mid) {
                                let _ = pnl_tracker.update_market_price(&symbol, price).await;
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                            warn!("Fill listener lagged, skipped {missed} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        pnl_positions = Some(pnl_tracker);
        order_engine = Some(orders);

        // Full streaming pipeline: candle -> patterns -> PatternDetected +
        // SignalGenerated events + signal persistence.
        let signal_engine = Arc::new(tokio::sync::RwLock::new(SignalEngine::with_risk_engine(
            SignalConfig::default(),
            Some(Arc::clone(&risk_engine)),
        )));
        let signal_repository = Arc::new(SignalRepository::new(db.pool.clone()));
        let mut pipeline = PatternStreamingPipeline::new(
            event_bus.clone(),
            Arc::clone(&pattern_registry),
            signal_engine,
            signal_repository,
        );
        pipeline.start();
        // The spawned task keeps running when the handle drops
        std::mem::drop(pipeline);
    } else {
        // Display-only fallback: detect patterns for the dashboard without
        // signal generation or persistence.
        let event_bus = event_bus.clone();
        let pattern_registry = Arc::clone(&pattern_registry);
        let market_cache = Arc::clone(&market_cache);

        tokio::spawn(async move {
            let mut rx = event_bus.subscribe();
            info!("Pattern detection service started (display-only mode)");

            while let Ok(event) = rx.recv().await {
                if let daytrader::events::types::MarketEvent::NewCandle { symbol, candle: _ } =
                    event
                {
                    // Snapshot candles inside the lock, then release it before
                    // awaiting: the std MutexGuard must not live across .await.
                    let recent_candles = match market_cache.lock() {
                        Ok(cache) => cache.get_recent_candles(&symbol, 50).unwrap_or_default(),
                        Err(_) => continue,
                    };
                    if recent_candles.is_empty() {
                        continue;
                    }

                    let patterns = pattern_registry.detect_all(&recent_candles, None).await;
                    for pattern_result in patterns {
                        let signal = daytrader::events::types::PatternSignal {
                            symbol: symbol.clone(),
                            pattern_name: pattern_result.pattern_name,
                            signal_type: pattern_result.signal_type,
                            confidence: pattern_result.confidence,
                            detected_at: pattern_result.detected_at,
                            price: pattern_result.price_level,
                        };
                        let _ = event_bus.publish(
                            daytrader::events::types::MarketEvent::PatternDetected { signal },
                        );
                    }
                }
            }
        });
    }

    // Application state for Tauri commands. The position tracker is the risk
    // engine's own instance so fills recorded from OrderFilled events are
    // visible to the UI.
    let mut app_state = state::AppState::new_with_caches(
        event_bus.clone(),
        broker.clone() as Arc<dyn MarketDataProvider>,
        Arc::clone(&pattern_registry),
        Arc::clone(&market_cache),
        Arc::clone(&pattern_cache),
        Arc::clone(&risk_engine),
        Some(Arc::clone(&mock_driver)),
    );
    app_state.position_tracker = Arc::clone(risk_engine.position_tracker());
    app_state.performance_repository = database
        .as_ref()
        .map(|db| Arc::new(PerformanceRepository::new(db.pool.clone())));
    app_state.database = database;
    app_state.order_engine = order_engine;
    app_state.execution_engine = execution_engine;
    app_state.pnl_tracker = pnl_positions;

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // Start RiskEngine
            let risk_engine_clone = Arc::clone(&risk_engine);
            tokio::spawn(async move {
                if let Err(e) = risk_engine_clone.start().await {
                    error!("Failed to start risk engine: {e}");
                }
            });

            // Start event bridge with shared caches
            let mut bridge = events::EventBridge::new(
                app.handle().clone(),
                event_bus.clone(),
                Arc::clone(&market_cache),
                Arc::clone(&pattern_cache),
            );
            bridge.start();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::market_data::get_latest_quote,
            commands::market_data::get_recent_candles,
            commands::market_data::subscribe_symbol,
            commands::market_data::unsubscribe_symbol,
            commands::patterns::get_pattern_statistics,
            commands::risk::get_risk_state,
            commands::risk::get_risk_limits,
            commands::risk::get_recent_risk_violations,
            commands::risk::get_pdt_status,
            commands::risk::get_day_trade_history,
            commands::risk::check_pdt_eligibility,
            commands::risk::get_pdt_violations,
            commands::risk::get_business_day_info,
            commands::system::get_system_status,
            commands::execution::get_execution_status,
            commands::execution::set_automation_enabled,
            commands::execution::get_active_orders,
            commands::execution::get_recent_orders,
            commands::execution::get_open_positions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
