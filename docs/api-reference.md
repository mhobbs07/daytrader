# Day Trading Platform API Reference

## Overview

This document provides comprehensive API reference documentation for the Day Trading Platform. All public APIs are documented with their signatures, parameters, return types, and usage examples.

## Core Types

### Configuration

#### `AppConfig`
Main application configuration structure.

```rust
pub struct AppConfig {
    pub broker: BrokerConfig,
    pub risk: RiskConfig,
    pub patterns: PatternConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
}
```

**Methods:**
- `from_file(path: &str) -> Result<Self>` - Load configuration from TOML file
- `from_env() -> Result<Self>` - Load configuration from environment variables
- `validate(&self) -> Result<()>` - Validate configuration values

#### `BrokerConfig`
Broker-specific configuration.

```rust
pub struct BrokerConfig {
    pub provider: BrokerProvider,
    pub api_key: String,
    pub api_secret: String,
    pub is_paper: bool,
    pub rate_limit: RateLimitConfig,
}
```

### Domain Types (Newtypes)

#### `Symbol`
Stock symbol with validation.

```rust
pub struct Symbol(String);

impl Symbol {
    pub fn new(s: &str) -> Result<Self> { ... }
    pub fn as_str(&self) -> &str { ... }
}
```

**Validation:** 1-5 uppercase letters, no special characters.

#### `Price`
Monetary value with decimal precision.

```rust
pub struct Price(Decimal);

impl Price {
    pub fn new(value: Decimal) -> Result<Self> { ... }
    pub fn value(&self) -> Decimal { ... }
}
```

**Validation:** Must be positive.

#### `Quantity`
Share quantity.

```rust
pub struct Quantity(u32);

impl Quantity {
    pub fn new(value: u32) -> Result<Self> { ... }
    pub fn value(&self) -> u32 { ... }
}
```

**Validation:** Must be positive.

#### `Confidence`
Pattern confidence score (0.0 to 1.0).

```rust
pub struct Confidence(f64);

impl Confidence {
    pub fn new(value: f64) -> Result<Self> { ... }
    pub fn value(&self) -> f64 { ... }
    pub fn as_percentage(&self) -> f64 { ... }
}
```

## Broker Module

### Traits

#### `Broker`
Core broker interface for trading operations.

```rust
#[async_trait]
pub trait Broker: Send + Sync {
    async fn place_order(&self, order: Order) -> Result<OrderId>;
    async fn cancel_order(&self, order_id: OrderId) -> Result<()>;
    async fn get_order_status(&self, order_id: OrderId) -> Result<OrderStatus>;
    async fn get_account_balance(&self) -> Result<DollarAmount>;
    async fn get_positions(&self) -> Result<Vec<Position>>;
    async fn subscribe_market_data(&self, symbols: &[Symbol]) -> Result<MarketDataStream>;
}
```

### Implementations

#### `AlpacaBroker`
Alpaca trading API implementation.

```rust
pub struct AlpacaBroker {
    config: BrokerConfig,
    client: reqwest::Client,
}

impl AlpacaBroker {
    pub fn new(config: BrokerConfig) -> Result<Self> { ... }
}
```

**Features:**
- REST API integration
- WebSocket streaming
- Paper trading support
- Rate limiting

#### `MockBroker`
Mock broker for testing.

```rust
pub struct MockBroker {
    positions: Arc<Mutex<HashMap<Symbol, Position>>>,
    orders: Arc<Mutex<HashMap<OrderId, OrderStatus>>>,
}

impl MockBroker {
    pub fn new() -> Self { ... }
    pub fn set_balance(&self, balance: DollarAmount) { ... }
    pub fn add_position(&self, symbol: Symbol, position: Position) { ... }
}
```

## Pattern Recognition Module

### Core Types

#### `PatternResult`
Result of pattern detection.

```rust
pub struct PatternResult {
    pub pattern_name: PatternName,
    pub signal_type: SignalType,
    pub confidence: Confidence,
    pub detected_at: DateTime<Utc>,
    pub price_level: Price,
    pub context: PatternContext,
}
```

#### `PatternParams`
Parameters for pattern detection.

```rust
pub struct PatternParams {
    pub min_confidence: f64,
    pub require_volume_confirmation: bool,
    pub max_lookback_candles: usize,
    pub trend_context: Option<TrendDirection>,
}
```

### Traits

#### `PatternDetector`
Interface for pattern detection algorithms.

```rust
pub trait PatternDetector: Send + Sync {
    fn name(&self) -> &'static str;
    fn min_candles_required(&self) -> usize;
    fn detect(&self, candles: &[Candle], params: &PatternParams) -> Result<Option<PatternResult>>;
}
```

### Pattern Detectors

#### `HammerDetector`
Detects hammer candlestick patterns.

```rust
pub struct HammerDetector;

impl HammerDetector {
    pub fn new() -> Self { ... }
}
```

**Criteria:**
- Lower shadow ≥ 2x body size
- Upper shadow ≤ body size
- Body ratio: 10-40% of total range

#### `ShootingStarDetector`
Detects shooting star patterns.

```rust
pub struct ShootingStarDetector;

impl ShootingStarDetector {
    pub fn new() -> Self { ... }
}
```

**Criteria:**
- Upper shadow ≥ 2x body size
- Lower shadow ≤ body size
- Bearish reversal signal

#### `DojiDetector`
Detects doji patterns with variants.

```rust
pub struct DojiDetector;

impl DojiDetector {
    pub fn new() -> Self { ... }
}
```

**Types:**
- Standard Doji
- Dragonfly Doji (long lower shadow)
- Gravestone Doji (long upper shadow)

#### `BullishEngulfingDetector`
Detects bullish engulfing patterns.

```rust
pub struct BullishEngulfingDetector;

impl BullishEngulfingDetector {
    pub fn new() -> Self { ... }
}
```

**Criteria:**
- Bearish candle followed by larger bullish candle
- Complete body engulfment
- Volume confirmation

#### `BearishEngulfingDetector`
Detects bearish engulfing patterns.

```rust
pub struct BearishEngulfingDetector;

impl BearishEngulfingDetector {
    pub fn new() -> Self { ... }
}
```

### Pattern Registry

#### `PatternRegistry`
Manages multiple pattern detectors.

```rust
pub struct PatternRegistry {
    detectors: HashMap<String, Box<dyn PatternDetector>>,
    config: RegistryConfig,
}

impl PatternRegistry {
    pub async fn new() -> Self { ... }
    pub fn register(&self, name: &str, detector: Box<dyn PatternDetector>) -> Result<()> { ... }
    pub async fn detect_all(&self, candles: &[Candle], params: Option<&PatternParams>) -> Vec<PatternResult> { ... }
    pub async fn detect_by_category(&self, candles: &[Candle], category: PatternCategory, params: Option<&PatternParams>) -> Vec<PatternResult> { ... }
}
```

#### `PatternRegistryBuilder`
Builder for configuring pattern registry.

```rust
pub struct PatternRegistryBuilder {
    config: RegistryConfig,
}

impl PatternRegistryBuilder {
    pub fn new() -> Self { ... }
    pub fn max_parallel_patterns(mut self, max: usize) -> Self { ... }
    pub fn global_min_confidence(mut self, min: f64) -> Self { ... }
    pub fn enabled_categories(mut self, categories: Vec<PatternCategory>) -> Self { ... }
    pub async fn build(self) -> Result<PatternRegistry> { ... }
}
```

## Risk Management Module

### Core Types

#### `RiskConfig`
Risk management configuration.

```rust
pub struct RiskConfig {
    pub max_daily_loss_dollars: DollarAmount,
    pub max_daily_loss_percent: Percentage,
    pub max_position_size_dollars: DollarAmount,
    pub max_position_size_percent: Percentage,
    pub max_concurrent_positions: Count,
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: DollarAmount,
    pub max_day_trades_under_pdt: Count,
}
```

#### `RiskState`
Current risk state.

```rust
pub struct RiskState {
    pub account_id: AccountId,
    pub current_equity: DollarAmount,
    pub daily_pl: DollarAmount,
    pub day_trade_count: u32,
    pub open_positions: HashMap<Symbol, Position>,
    pub is_trading_halted: bool,
    pub violations: Vec<RiskViolation>,
}
```

### Risk Engine

#### `RiskEngine`
Central risk management engine with Phase 2C-D features including position sizing, PDT enforcement, emergency liquidation, and audit trails.

```rust
pub struct RiskEngine {
    config: RiskConfig,
    state: Arc<RwLock<RiskState>>,
    event_bus: Arc<EventBus>,
    rule_engine: RiskRuleEngine,
    daily_loss_tracker: Arc<DailyLossTracker>,
    position_sizer: Arc<PositionSizer>,
    pdt_enforcer: Option<Arc<PdtEnforcer>>,
    position_tracker: Arc<PositionTracker>,
    pattern_stats_tracker: Arc<RwLock<PatternStatsTracker>>,
    pattern_risk_evaluator: Arc<RwLock<PatternRiskEvaluator>>,
    broker: Arc<dyn OrderExecutionProvider>,
    database: Option<Arc<Database>>,
}

impl RiskEngine {
    pub fn new(config: RiskConfig, event_bus: Arc<EventBus>, account_id: AccountId, initial_equity: Decimal, broker: Arc<dyn OrderExecutionProvider>) -> Self { ... }
    pub fn with_database(config: RiskConfig, event_bus: Arc<EventBus>, account_id: AccountId, initial_equity: Decimal, broker: Arc<dyn OrderExecutionProvider>, database: Option<Arc<Database>>) -> Self { ... }
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    pub async fn validate_order_placement(&self, order: &Order) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    pub async fn update_on_fill(&self, order_id: OrderId, fill: &Fill) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    pub async fn update_position_pl(&self, position: &Position) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    pub async fn update_daily_pl(&self, realized_pl: Decimal) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    pub async fn get_risk_state(&self) -> RiskState { ... }
    pub async fn is_trading_allowed(&self) -> bool { ... }
    pub async fn halt_trading(&self, reason: impl Into<String>) { ... }
    pub async fn resume_trading(&self) { ... }
    pub async fn perform_emergency_liquidation(&self) -> Result<(Vec<Symbol>, Decimal), Box<dyn std::error::Error + Send + Sync>> { ... }
}
```

### Risk Rules

#### `PDTMonitor`
Pattern Day Trading rule monitor.

```rust
pub struct PDTMonitor {
    config: PDTConfig,
    state: PDTState,
}

impl PDTMonitor {
    pub fn new(config: PDTConfig) -> Self { ... }
    pub fn can_place_order(&self, order: &Order) -> Result<()> { ... }
    pub fn record_day_trade(&self) -> Result<()> { ... }
    pub fn is_pdt_violation(&self) -> bool { ... }
}
```

#### `PositionSizer`
Position sizing calculator.

```rust
pub struct PositionSizer {
    config: PositionSizeConfig,
}

impl PositionSizer {
    pub fn new(config: PositionSizeConfig) -> Self { ... }
    pub fn calculate_size(&self, signal: &PatternSignal, account_balance: DollarAmount) -> Result<Quantity> { ... }
    pub fn validate_size(&self, quantity: Quantity, price: Price, account_balance: DollarAmount) -> Result<()> { ... }
}
```

## Event System

### Core Types

#### `MarketEvent`
Events published by the system.

```rust
pub enum MarketEvent {
    PatternDetected { signal: PatternSignal },
    OrderPlaced { order: Order },
    OrderFilled { order_id: OrderId, fill: Fill },
    OrderCancelled { order_id: OrderId },
    RiskViolation { violation: RiskViolation },
    MarketDataUpdate { symbol: Symbol, candle: Candle },
}
```

#### `EventBus`
Central event bus for pub/sub messaging.

```rust
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<MarketEvent>>>>,
    buffer_size: usize,
}

impl EventBus {
    pub fn new(buffer_size: usize) -> Self { ... }
    pub fn subscribe(&self) -> mpsc::Receiver<MarketEvent> { ... }
    pub fn publish(&self, event: MarketEvent) { ... }
}
```

## Database Module

### Database Interface

#### `Database`
Database abstraction trait.

```rust
#[async_trait]
pub trait Database: Send + Sync {
    async fn store_pattern(&self, result: &PatternResult) -> Result<()>;
    async fn get_patterns(&self, symbol: &Symbol, limit: usize) -> Result<Vec<PatternResult>>;
    async fn store_order(&self, order: &Order) -> Result<OrderId>;
    async fn update_order_status(&self, order_id: &OrderId, status: OrderStatus) -> Result<()>;
    async fn get_positions(&self, account_id: &AccountId) -> Result<Vec<Position>>;
}
```

### SQLite Implementation

#### `SQLiteDatabase`
SQLite database implementation.

```rust
pub struct SQLiteDatabase {
    pool: sqlx::SqlitePool,
}

impl SQLiteDatabase {
    pub async fn new(path: &str) -> Result<Self> { ... }
    pub async fn migrate(&self) -> Result<()> { ... }
}
```

## Utility Modules

### Error Handling

#### `AppError`
Main application error type.

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Broker error: {0}")]
    Broker(#[from] BrokerError),

    #[error("Pattern error: {0}")]
    Pattern(#[from] PatternError),

    #[error("Risk error: {0}")]
    Risk(#[from] RiskError),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
}
```

### Logging

#### `Logger`
Structured logging interface.

```rust
pub struct Logger {
    subscriber: tracing::subscriber::DefaultGuard,
}

impl Logger {
    pub fn init(level: &str) -> Result<Self> { ... }
    pub fn with_file_output(level: &str, file_path: &str) -> Result<Self> { ... }
}
```

### Metrics

#### `MetricsCollector`
Performance metrics collection.

```rust
pub struct MetricsCollector {
    registry: prometheus::Registry,
}

impl MetricsCollector {
    pub fn new() -> Self { ... }
    pub fn record_pattern_detection(&self, duration: Duration, success: bool) { ... }
    pub fn record_order_placed(&self, symbol: &Symbol, quantity: Quantity) { ... }
    pub fn get_metrics(&self) -> String { ... }
}
```

### Performance Monitoring

#### `PerformanceMonitor`
Real-time performance monitoring.

```rust
pub struct PerformanceMonitor {
    metrics: Arc<MetricsCollector>,
}

impl PerformanceMonitor {
    pub fn new() -> Self { ... }
    pub fn start_monitoring(&self) { ... }
    pub fn record_operation<T, F>(&self, operation: &str, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    { ... }
}
```

### Retry Logic

#### `RetryConfig`
Retry configuration.

```rust
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
}
```

#### `Retryable`
Trait for retryable operations.

```rust
pub trait Retryable<T, E> {
    fn retry(self, config: &RetryConfig) -> RetryFuture<T, E>;
}
```

## Error Types

### Broker Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum BrokerError {
    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Order rejected: {reason}")]
    OrderRejected { reason: String },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}
```

### Pattern Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum PatternError {
    #[error("Insufficient data: required {required}, available {available}")]
    InsufficientData { required: usize, available: usize },

    #[error("Invalid candle data: {0}")]
    InvalidCandleData(String),

    #[error("Pattern detection failed: {0}")]
    DetectionFailed(String),
}
```

### Risk Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum RiskError {
    #[error("Daily loss limit exceeded: current ${current}, limit ${limit}")]
    DailyLossLimitExceeded { current: Decimal, limit: Decimal },

    #[error("Position size limit exceeded")]
    PositionSizeLimitExceeded,

    #[error("PDT rule violation")]
    PDTViolation,

    #[error("Risk validation failed: {0}")]
    ValidationFailed(String),
}
```

## Constants

### Pattern Detection
```rust
pub const MIN_PATTERN_CONFIDENCE: f64 = 0.5;
pub const MAX_LOOKBACK_CANDLES: usize = 50;
pub const DEFAULT_PATTERN_TIMEOUT: Duration = Duration::from_millis(100);
```

### Risk Management
```rust
pub const PDT_MAX_DAY_TRADES: u32 = 3;
pub const PDT_MIN_EQUITY: Decimal = Decimal::new(2500000, 2); // $25,000
pub const DEFAULT_MAX_DAILY_LOSS_PERCENT: f64 = 0.02; // 2%
```

### Broker Limits
```rust
pub const ALPACA_RATE_LIMIT_REQUESTS: u32 = 200;
pub const ALPACA_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
```

## Feature Flags

The following Cargo features are available:

- `mock-broker`: Enable mock broker for testing
- `alpaca-streaming`: Enable Alpaca WebSocket streaming
- `prometheus-metrics`: Enable Prometheus metrics export
- `performance-monitoring`: Enable detailed performance monitoring

## Examples

See `api-usage-examples.md` for comprehensive usage examples covering all major APIs.

## Related Documentation

- [Risk Management Guide](risk-management-guide.md) - Comprehensive risk system documentation including position sizing, emergency liquidation, and audit trails
- [PDT Implementation Guide](pdt-implementation-guide.md)
- [PDT Compliance Checklist](pdt-compliance-checklist.md)
- [Troubleshooting Guide](troubleshooting.md)

**Last Updated:** January 24, 2026
**Version:** 2.0 (Phase 2C-D)
**Next Review:** February 24, 2026</content>
</xai:function_call">The API reference document has been created successfully. It provides comprehensive documentation for all public modules, types, traits, and functions in the Day Trading Platform. The document includes:

- Core configuration types
- Domain types with validation rules
- Complete broker module API
- Pattern recognition system
- Risk management interfaces
- Event system
- Database abstractions
- Utility modules
- Error types
- Constants and feature flags

This serves as the authoritative API reference for developers using the platform.