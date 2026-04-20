use daytrader::{
    brokers::{Candle, Quote, Symbol, traits::MarketDataProvider},
    db::{
        Database,
        repositories::{PdtRepository, PerformanceRepository},
    },
    events::bus::EventBus,
    events::types::PatternSignal,
    patterns::PatternRegistry,
    risk::RiskEngine,
    risk::pdt::PdtEnforcer,
    risk::position_tracker::PositionTracker,
    utils::holidays::MarketHolidayCalendar,
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

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
            Ok(candles
                .iter()
                .rev()
                .take(count)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub fn update_quote(&mut self, symbol: String, quote: Quote) {
        self.quotes.insert(symbol, quote);
    }

    pub fn add_candle(&mut self, symbol: String, candle: Candle) {
        self.candles
            .entry(symbol.clone())
            .or_insert_with(Vec::new)
            .push(candle);
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
        Ok(self
            .patterns
            .iter()
            .filter(|p| p.symbol.as_str() == symbol)
            .cloned()
            .collect())
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
    // Enhanced risk management components
    pub pdt_enforcer: Option<Arc<PdtEnforcer>>,
    pub position_tracker: Arc<PositionTracker>,
    pub holiday_calendar: Arc<MarketHolidayCalendar>,
    // Database integration
    pub database: Option<Arc<Database>>,
    pub db_pool: Option<SqlitePool>,
    pub pdt_repository: Option<Arc<PdtRepository>>,
    pub performance_repository: Option<Arc<PerformanceRepository>>,
    // Account information
    pub account_id: i64, // Numeric account ID for database operations
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
            // Enhanced risk management components (None for basic setup)
            pdt_enforcer: None,
            position_tracker: Arc::new(PositionTracker::new()),
            holiday_calendar: Arc::new(MarketHolidayCalendar::new()),
            // Database integration (None for basic setup)
            database: None,
            db_pool: None,
            pdt_repository: None,
            performance_repository: None,
            // Default account ID
            account_id: 1, // Default account ID
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
            // Enhanced risk management components (None for basic setup)
            pdt_enforcer: None,
            position_tracker: Arc::new(PositionTracker::new()),
            holiday_calendar: Arc::new(MarketHolidayCalendar::new()),
            // Database integration (None for basic setup)
            database: None,
            db_pool: None,
            pdt_repository: None,
            performance_repository: None,
            // Default account ID
            account_id: 1,
        }
    }

    /// Creates an enhanced AppState with database support and PDT enforcement
    pub async fn new_with_database(
        event_bus: EventBus,
        broker: Arc<dyn MarketDataProvider>,
        pattern_registry: Arc<PatternRegistry>,
        risk_engine: Arc<RiskEngine>,
        database: Arc<Database>,
        account_id: i64,
        initial_equity: rust_decimal::Decimal,
        config: daytrader::config::RiskConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use daytrader::risk::pdt::PdtConfig;

        let db_pool = database.pool.clone();

        // Create PDT enforcer with database support
        let pdt_config = PdtConfig::from_config(&config);
        let pdt_enforcer =
            PdtEnforcer::new(pdt_config, initial_equity, db_pool.clone(), account_id).await?;

        // Initialize repositories
        let pdt_repository = Arc::new(PdtRepository::new(db_pool.clone()));
        let performance_repository = Arc::new(PerformanceRepository::new(db_pool.clone()));

        Ok(Self {
            event_bus,
            broker,
            broker_connected: Arc::new(AtomicBool::new(false)),
            pattern_registry,
            market_cache: Arc::new(std::sync::Mutex::new(MarketDataCache::new())),
            pattern_cache: Arc::new(std::sync::Mutex::new(PatternCache::new())),
            risk_engine,
            // Enhanced risk management components
            pdt_enforcer: Some(Arc::new(pdt_enforcer)),
            position_tracker: Arc::new(PositionTracker::new()),
            holiday_calendar: Arc::new(MarketHolidayCalendar::new()),
            // Database integration
            database: Some(database),
            db_pool: Some(db_pool),
            pdt_repository: Some(pdt_repository),
            performance_repository: Some(performance_repository),
            // Account information
            account_id,
        })
    }

    /// Gets the PDT enforcer if available
    pub fn pdt_enforcer(&self) -> Option<&Arc<PdtEnforcer>> {
        self.pdt_enforcer.as_ref()
    }

    /// Gets the PDT repository if available
    pub fn pdt_repository(&self) -> Option<&Arc<PdtRepository>> {
        self.pdt_repository.as_ref()
    }

    /// Gets the performance repository if available
    pub fn performance_repository(&self) -> Option<&Arc<PerformanceRepository>> {
        self.performance_repository.as_ref()
    }

    /// Gets the holiday calendar
    pub fn holiday_calendar(&self) -> &Arc<MarketHolidayCalendar> {
        &self.holiday_calendar
    }

    /// Gets the position tracker
    pub fn position_tracker(&self) -> &Arc<PositionTracker> {
        &self.position_tracker
    }
}
