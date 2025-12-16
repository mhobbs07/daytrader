# Day Trading Platform - Technical Architecture Document

**Version:** 1.0  
**Date:** November 15, 2025  
**Status:** Initial Architecture  
**Timeline:** 4-6 Month MVP Development

---

## Executive Summary

This document defines the technical architecture for a comprehensive day trading platform focused on pattern-based stock trading with automated execution capabilities. The system is designed for 1-10 users, prioritizing cost optimization, regulatory compliance, and future extensibility.

### Core Objectives
- Pattern recognition using candlestick analysis for buy/sell signal generation
- Support for both manual and automated trade execution
- Comprehensive risk management with regulatory compliance (SEC, PDT rules)
- Flexible architecture enabling future broker/data provider changes
- Cost-optimized approach using broker APIs for market data

### Key Constraints
- US market focus with SEC/FINRA compliance
- Pattern Day Trader (PDT) rule enforcement
- 100ms-1s latency target for order execution
- Budget-conscious design (starting ~$50/month)
- 4-6 month development timeline

---

## System Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Tauri Desktop Application                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Frontend (React/TypeScript)                  │  │
│  │  - Real-time Charts (TradingView/Lightweight Charts)     │  │
│  │  - Order Entry UI                                        │  │
│  │  - Pattern Alerts Dashboard                              │  │
│  │  - Risk Management Display                               │  │
│  │  - Watchlists & Portfolio Views                          │  │
│  └───────────────────────┬──────────────────────────────────┘  │
│                          │ Tauri IPC                            │
│  ┌───────────────────────▼──────────────────────────────────┐  │
│  │              Rust Backend Core                            │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │          Market Data Engine (Rust)                  │ │  │
│  │  │  - WebSocket connection management                  │ │  │
│  │  │  - Real-time quote processing (1-min candles)       │ │  │
│  │  │  - Data normalization & validation                  │ │  │
│  │  │  - Time-series data aggregation                     │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │       Pattern Recognition Engine (Rust + Python)    │ │  │
│  │  │  - Candlestick pattern detection                    │ │  │
│  │  │  - Technical indicator calculations                 │ │  │
│  │  │  - Signal generation logic                          │ │  │
│  │  │  - Python bridge for ML/LLM analysis                │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │          Risk Management System (Rust)              │ │  │
│  │  │  - Daily loss tracking                              │ │  │
│  │  │  - Position size validation                         │ │  │
│  │  │  - PDT rule enforcement                             │ │  │
│  │  │  - Pattern-specific limits                          │ │  │
│  │  │  - Account balance safeguards                       │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │         Order Execution Engine (Rust)               │ │  │
│  │  │  - Order routing & validation                       │ │  │
│  │  │  - Auto-execution logic                             │ │  │
│  │  │  - Position tracking                                │ │  │
│  │  │  - Order history management                         │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │           Broker Abstraction Layer (Rust)           │ │  │
│  │  │  - Trait definitions for market data & execution    │ │  │
│  │  │  - Provider adapter implementations                 │ │  │
│  │  │  - Configuration-based provider selection           │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  └───────────────────────┬──────────────────────────────────┘  │
└────────────────────────────┼───────────────────────────────────┘
                             │
                ┌────────────┴────────────┐
                │                         │
     ┌──────────▼─────────┐   ┌──────────▼──────────┐
     │  Broker API        │   │  Local Database     │
     │  (Alpaca)          │   │  (SQLite/DuckDB)    │
     │  - Market Data WS  │   │  - Positions        │
     │  - Order Execution │   │  - Trade History    │
     │  - Account Info    │   │  - Pattern Results  │
     │  - Paper Trading   │   │  - Backtests        │
     └────────────────────┘   │  - Risk Metrics     │
                              └─────────────────────┘
     ┌─────────────────────────────────────────┐
     │     Python Analytics Bridge              │
     │  - Advanced pattern analysis             │
     │  - LLM-based data interpretation         │
     │  - Custom indicator development          │
     │  - Backtesting execution                 │
     └─────────────────────────────────────────┘
```

---

## Technology Stack

### Core Technologies

**Desktop Application Framework:**
- **Tauri 1.5+** - Rust-based desktop application framework
  - Small binary size (~3-5MB)
  - Native performance using system webview
  - Strong security model
  - Cross-platform (Windows, macOS, Linux)

**Backend (Rust):**
- **Language:** Rust 1.75+ (2021 edition)
- **Async Runtime:** Tokio 1.35+ for async I/O
- **HTTP Client:** reqwest 0.11+ for REST API calls
- **WebSocket:** tokio-tungstenite 0.21+ for real-time data streams
- **Serialization:** serde 1.0+ with serde_json for JSON handling
- **Database:** 
  - SQLite via rusqlite 0.30+ for local storage
  - OR DuckDB 0.9+ for analytical queries (time-series)
- **Error Handling:** 
  - thiserror 1.0+ for custom error types
  - anyhow 1.0+ for error propagation
- **Logging:** tracing 0.1+ with tracing-subscriber
- **Configuration:** config 0.13+ for settings management

**Frontend:**
- **Framework:** React 18+ with TypeScript 5.0+
- **State Management:** Zustand 4.0+ or Jotai 2.0+ (lightweight)
- **Charting:** 
  - TradingView Lightweight Charts 4.0+ (primary)
  - Alternative: Recharts for simpler visualizations
- **UI Components:** 
  - shadcn/ui (Radix UI primitives + Tailwind)
  - Headless UI for accessible components
- **Styling:** Tailwind CSS 3.0+
- **Build Tool:** Vite 5.0+

**Python Integration:**
- **Python:** 3.11+
- **Bridge:** PyO3 0.20+ for Rust-Python interop OR subprocess-based bridge
- **Data Science:**
  - pandas 2.0+ for data manipulation
  - numpy 1.26+ for numerical computations
  - ta-lib OR pandas-ta for technical indicators
- **Machine Learning (Future):**
  - scikit-learn for pattern classification
  - LLM integration via langchain or direct API calls
- **Backtesting:** 
  - backtrader 1.9+ OR custom implementation
  - vectorbt 0.26+ for vectorized backtesting

### Broker Integration

**Primary Broker (Phase 1):**
- **Alpaca Markets API**
  - REST API for orders, account, positions
  - WebSocket for real-time quotes (IEX data)
  - Paper trading environment included
  - Commission-free trading
  - Good documentation and SDKs
  - API client: alpaca-trade-api (Python) or custom Rust implementation

**Alternative Brokers (Future):**
- Interactive Brokers (TWS API)
- TD Ameritrade (thinkorswim API) - now Schwab

### Data Storage Strategy

**Time-Series Data:**
```
Option 1: DuckDB (Recommended)
- Excellent for analytical queries on time-series data
- Fast aggregations for backtesting
- Embedded, no separate server
- Parquet file format for efficiency

Option 2: SQLite + Extension
- SQLite with time-series optimizations
- Simpler, more mature
- Good for smaller datasets
```

**Database Schema Design:**

```sql
-- Accounts Table
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    broker TEXT NOT NULL,
    account_number TEXT NOT NULL UNIQUE,
    account_type TEXT NOT NULL, -- 'paper' or 'live'
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Market Data (1-min candles)
CREATE TABLE candles (
    id INTEGER PRIMARY KEY,
    symbol TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume INTEGER NOT NULL,
    broker TEXT NOT NULL,
    UNIQUE(symbol, timestamp, broker)
);
CREATE INDEX idx_candles_symbol_time ON candles(symbol, timestamp DESC);

-- Positions
CREATE TABLE positions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_entry_price REAL NOT NULL,
    current_price REAL,
    market_value REAL,
    unrealized_pl REAL,
    unrealized_pl_percent REAL,
    opened_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, symbol)
);

-- Orders
CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    broker_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL, -- 'buy' or 'sell'
    order_type TEXT NOT NULL, -- 'market', 'limit', 'stop', etc.
    quantity INTEGER NOT NULL,
    filled_quantity INTEGER DEFAULT 0,
    limit_price REAL,
    stop_price REAL,
    trailing_percent REAL,
    status TEXT NOT NULL, -- 'pending', 'filled', 'partial', 'cancelled', 'rejected'
    time_in_force TEXT DEFAULT 'day', -- 'day', 'gtc', 'ioc', 'fok'
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMP,
    filled_avg_price REAL,
    commission REAL DEFAULT 0.0,
    error_message TEXT,
    is_pattern_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);
CREATE INDEX idx_orders_account_time ON orders(account_id, submitted_at DESC);

-- Trades (Filled Orders)
CREATE TABLE trades (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    price REAL NOT NULL,
    commission REAL DEFAULT 0.0,
    realized_pl REAL, -- For closing trades
    executed_at TIMESTAMP NOT NULL,
    is_day_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);
CREATE INDEX idx_trades_account_time ON trades(account_id, executed_at DESC);

-- Pattern Signals
CREATE TABLE pattern_signals (
    id INTEGER PRIMARY KEY,
    symbol TEXT NOT NULL,
    pattern_name TEXT NOT NULL,
    signal_type TEXT NOT NULL, -- 'buy' or 'sell'
    confidence REAL, -- 0.0 to 1.0
    detected_at TIMESTAMP NOT NULL,
    candle_timestamp TIMESTAMP NOT NULL,
    price_at_detection REAL NOT NULL,
    acted_upon BOOLEAN DEFAULT FALSE,
    order_id INTEGER,
    outcome TEXT, -- 'win', 'loss', 'pending', 'ignored'
    FOREIGN KEY (order_id) REFERENCES orders(id)
);
CREATE INDEX idx_signals_symbol_time ON pattern_signals(symbol, detected_at DESC);

-- Pattern Performance Tracking
CREATE TABLE pattern_stats (
    id INTEGER PRIMARY KEY,
    pattern_name TEXT NOT NULL UNIQUE,
    total_signals INTEGER DEFAULT 0,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit REAL DEFAULT 0.0,
    win_rate REAL DEFAULT 0.0,
    avg_profit_per_trade REAL DEFAULT 0.0,
    is_enabled BOOLEAN DEFAULT TRUE,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily Risk Tracking
CREATE TABLE daily_risk_metrics (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    trading_date DATE NOT NULL,
    starting_balance REAL NOT NULL,
    current_balance REAL NOT NULL,
    realized_pl REAL DEFAULT 0.0,
    unrealized_pl REAL DEFAULT 0.0,
    total_pl REAL DEFAULT 0.0,
    day_trades_count INTEGER DEFAULT 0,
    trades_count INTEGER DEFAULT 0,
    largest_loss REAL DEFAULT 0.0,
    is_trading_halted BOOLEAN DEFAULT FALSE,
    halt_reason TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, trading_date)
);

-- Watchlists
CREATE TABLE watchlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE watchlist_items (
    id INTEGER PRIMARY KEY,
    watchlist_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id),
    UNIQUE(watchlist_id, symbol)
);

-- Price Alerts
CREATE TABLE alerts (
    id INTEGER PRIMARY KEY,
    symbol TEXT NOT NULL,
    alert_type TEXT NOT NULL, -- 'price_above', 'price_below', 'pattern_detected'
    threshold_value REAL,
    pattern_name TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    triggered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Backtesting Results
CREATE TABLE backtest_runs (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    strategy_config TEXT NOT NULL, -- JSON configuration
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    initial_capital REAL NOT NULL,
    final_value REAL NOT NULL,
    total_return REAL NOT NULL,
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    win_rate REAL NOT NULL,
    max_drawdown REAL NOT NULL,
    sharpe_ratio REAL,
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE backtest_trades (
    id INTEGER PRIMARY KEY,
    backtest_run_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    entry_price REAL NOT NULL,
    exit_price REAL,
    entry_time TIMESTAMP NOT NULL,
    exit_time TIMESTAMP,
    profit_loss REAL,
    pattern_name TEXT,
    FOREIGN KEY (backtest_run_id) REFERENCES backtest_runs(id)
);

-- Configuration & Settings
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## Broker Abstraction Layer Design

### Architecture Goal
Create a pluggable architecture that allows swapping broker/data providers without changing business logic.

### Trait Definitions (Rust)

```rust
// src/brokers/traits.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: i32,
    pub avg_entry_price: Decimal,
    pub current_price: Decimal,
    pub market_value: Decimal,
    pub unrealized_pl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: u32,
    pub limit_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    TrailingStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Filled,
    PartiallyFilled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    Day,
    GTC,  // Good Till Cancelled
    IOC,  // Immediate Or Cancel
    FOK,  // Fill Or Kill
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_number: String,
    pub buying_power: Decimal,
    pub cash: Decimal,
    pub portfolio_value: Decimal,
    pub day_trade_count: u32,
    pub is_pattern_day_trader: bool,
}

// ============================================================================
// Trait Definitions
// ============================================================================

/// Market Data Provider trait
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// Subscribe to real-time quotes for given symbols
    async fn subscribe_quotes(&self, symbols: Vec<String>) -> Result<(), BrokerError>;
    
    /// Unsubscribe from quotes
    async fn unsubscribe_quotes(&self, symbols: Vec<String>) -> Result<(), BrokerError>;
    
    /// Get latest quote for a symbol
    async fn get_quote(&self, symbol: &str) -> Result<Quote, BrokerError>;
    
    /// Get historical candles
    async fn get_candles(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError>;
    
    /// Start real-time data stream
    async fn start_stream(&self) -> Result<(), BrokerError>;
    
    /// Stop real-time data stream
    async fn stop_stream(&self) -> Result<(), BrokerError>;
}

/// Order Execution Provider trait
#[async_trait]
pub trait OrderExecutionProvider: Send + Sync {
    /// Place a new order
    async fn place_order(&self, order: Order) -> Result<String, BrokerError>;
    
    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> Result<(), BrokerError>;
    
    /// Get order status
    async fn get_order(&self, order_id: &str) -> Result<Order, BrokerError>;
    
    /// Get all open orders
    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError>;
    
    /// Get account information
    async fn get_account(&self) -> Result<Account, BrokerError>;
    
    /// Get current positions
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;
    
    /// Get position for specific symbol
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Timeframe {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    OneDay,
}

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Order rejected: {0}")]
    OrderRejected(String),
    
    #[error("Insufficient funds")]
    InsufficientFunds,
    
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}
```

### Alpaca Implementation Example

```rust
// src/brokers/alpaca.rs

use super::traits::*;
use async_trait::async_trait;
use reqwest::Client;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct AlpacaProvider {
    api_key: String,
    api_secret: String,
    base_url: String,
    ws_url: String,
    client: Client,
    is_paper: bool,
}

impl AlpacaProvider {
    pub fn new(api_key: String, api_secret: String, is_paper: bool) -> Self {
        let base_url = if is_paper {
            "https://paper-api.alpaca.markets".to_string()
        } else {
            "https://api.alpaca.markets".to_string()
        };
        
        let ws_url = if is_paper {
            "wss://stream.data.alpaca.markets/v2/iex".to_string()
        } else {
            "wss://stream.data.alpaca.markets/v2/iex".to_string()
        };
        
        Self {
            api_key,
            api_secret,
            base_url,
            ws_url,
            client: Client::new(),
            is_paper,
        }
    }
}

#[async_trait]
impl MarketDataProvider for AlpacaProvider {
    async fn subscribe_quotes(&self, symbols: Vec<String>) -> Result<(), BrokerError> {
        // Implementation here
        Ok(())
    }
    
    async fn unsubscribe_quotes(&self, symbols: Vec<String>) -> Result<(), BrokerError> {
        Ok(())
    }
    
    async fn get_quote(&self, symbol: &str) -> Result<Quote, BrokerError> {
        let url = format!("{}/v2/stocks/{}/quotes/latest", self.base_url, symbol);
        // Implementation here
        todo!()
    }
    
    async fn get_candles(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError> {
        // Implementation here
        todo!()
    }
    
    async fn start_stream(&self) -> Result<(), BrokerError> {
        // WebSocket connection implementation
        todo!()
    }
    
    async fn stop_stream(&self) -> Result<(), BrokerError> {
        todo!()
    }
}

#[async_trait]
impl OrderExecutionProvider for AlpacaProvider {
    async fn place_order(&self, order: Order) -> Result<String, BrokerError> {
        let url = format!("{}/v2/orders", self.base_url);
        // Implementation here
        todo!()
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), BrokerError> {
        let url = format!("{}/v2/orders/{}", self.base_url, order_id);
        // Implementation here
        todo!()
    }
    
    async fn get_order(&self, order_id: &str) -> Result<Order, BrokerError> {
        todo!()
    }
    
    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        todo!()
    }
    
    async fn get_account(&self) -> Result<Account, BrokerError> {
        let url = format!("{}/v2/account", self.base_url);
        // Implementation here
        todo!()
    }
    
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let url = format!("{}/v2/positions", self.base_url);
        // Implementation here
        todo!()
    }
    
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError> {
        todo!()
    }
}
```

### Provider Configuration

```rust
// src/brokers/config.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    pub provider: BrokerProvider,
    pub api_key: String,
    pub api_secret: String,
    pub is_paper: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrokerProvider {
    Alpaca,
    InteractiveBrokers,
    // Future providers
}

// Factory pattern for creating providers
pub fn create_provider(config: BrokerConfig) -> Box<dyn OrderExecutionProvider> {
    match config.provider {
        BrokerProvider::Alpaca => {
            Box::new(AlpacaProvider::new(
                config.api_key,
                config.api_secret,
                config.is_paper,
            ))
        }
        BrokerProvider::InteractiveBrokers => {
            // Future implementation
            todo!()
        }
    }
}
```

---

## Risk Management System Architecture

### Core Risk Rules

```rust
// src/risk/rules.rs

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    // Daily loss limits
    pub max_daily_loss_dollars: Decimal,
    pub max_daily_loss_percent: Decimal,
    pub halt_trading_on_limit: bool,
    
    // Position sizing
    pub max_position_size_dollars: Decimal,
    pub max_position_size_percent: Decimal,
    pub max_concurrent_positions: u32,
    
    // PDT protection
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: Decimal, // $25,000
    pub max_day_trades_under_pdt: u32, // 3 in 5 days
    
    // Pattern-specific rules
    pub max_trades_per_pattern_per_day: u32,
    pub stop_pattern_after_consecutive_losses: u32,
    pub required_win_rate_for_auto: Decimal, // e.g., 0.60 for 60%
    pub min_pattern_sample_size: u32, // Min trades before enabling auto
    
    // Account safeguards
    pub min_account_balance: Decimal,
    pub emergency_liquidate_threshold: Decimal,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_daily_loss_dollars: Decimal::new(50000, 2), // $500
            max_daily_loss_percent: Decimal::new(200, 2), // 2%
            halt_trading_on_limit: true,
            
            max_position_size_dollars: Decimal::new(500000, 2), // $5,000
            max_position_size_percent: Decimal::new(1000, 2), // 10%
            max_concurrent_positions: 5,
            
            enforce_pdt_rule: true,
            pdt_min_equity: Decimal::new(2500000, 2), // $25,000
            max_day_trades_under_pdt: 3,
            
            max_trades_per_pattern_per_day: 5,
            stop_pattern_after_consecutive_losses: 3,
            required_win_rate_for_auto: Decimal::new(60, 2), // 60%
            min_pattern_sample_size: 20,
            
            min_account_balance: Decimal::new(100000, 2), // $1,000
            emergency_liquidate_threshold: Decimal::new(50000, 2), // $500
        }
    }
}

pub struct RiskManager {
    config: RiskConfig,
    // Database connection for tracking metrics
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }
    
    /// Check if order is allowed given current risk state
    pub async fn validate_order(
        &self,
        order: &Order,
        account: &Account,
        daily_metrics: &DailyRiskMetrics,
        pattern_stats: Option<&PatternStats>,
    ) -> Result<(), RiskViolation> {
        
        // 1. Check daily loss limits
        self.check_daily_loss_limits(daily_metrics)?;
        
        // 2. Check position size limits
        self.check_position_size(order, account)?;
        
        // 3. Check PDT rule
        self.check_pdt_compliance(account, daily_metrics)?;
        
        // 4. Check pattern-specific limits
        if let Some(stats) = pattern_stats {
            self.check_pattern_limits(stats, daily_metrics)?;
        }
        
        // 5. Check minimum balance
        self.check_min_balance(account)?;
        
        Ok(())
    }
    
    fn check_daily_loss_limits(&self, metrics: &DailyRiskMetrics) -> Result<(), RiskViolation> {
        if metrics.total_pl <= -self.config.max_daily_loss_dollars {
            return Err(RiskViolation::DailyLossLimitExceeded {
                current_loss: metrics.total_pl,
                limit: self.config.max_daily_loss_dollars,
            });
        }
        
        let loss_percent = (metrics.total_pl / metrics.starting_balance).abs();
        if loss_percent >= self.config.max_daily_loss_percent {
            return Err(RiskViolation::DailyLossPercentExceeded {
                current_percent: loss_percent,
                limit: self.config.max_daily_loss_percent,
            });
        }
        
        Ok(())
    }
    
    fn check_position_size(&self, order: &Order, account: &Account) -> Result<(), RiskViolation> {
        let order_value = order.quantity as i64 * order.limit_price.unwrap_or(Decimal::ZERO);
        
        if order_value > self.config.max_position_size_dollars {
            return Err(RiskViolation::PositionSizeTooLarge {
                order_size: order_value,
                limit: self.config.max_position_size_dollars,
            });
        }
        
        let position_percent = order_value / account.portfolio_value;
        if position_percent > self.config.max_position_size_percent {
            return Err(RiskViolation::PositionPercentTooLarge {
                percent: position_percent,
                limit: self.config.max_position_size_percent,
            });
        }
        
        Ok(())
    }
    
    fn check_pdt_compliance(
        &self,
        account: &Account,
        metrics: &DailyRiskMetrics,
    ) -> Result<(), RiskViolation> {
        if !self.config.enforce_pdt_rule {
            return Ok(());
        }
        
        // If account is above PDT minimum, no restrictions
        if account.portfolio_value >= self.config.pdt_min_equity {
            return Ok(());
        }
        
        // Check if already at day trade limit
        if metrics.day_trades_count >= self.config.max_day_trades_under_pdt {
            return Err(RiskViolation::PatternDayTraderLimitReached {
                current_count: metrics.day_trades_count,
                account_value: account.portfolio_value,
                required_value: self.config.pdt_min_equity,
            });
        }
        
        Ok(())
    }
    
    fn check_pattern_limits(
        &self,
        stats: &PatternStats,
        metrics: &DailyRiskMetrics,
    ) -> Result<(), RiskViolation> {
        // Check if pattern is enabled
        if !stats.is_enabled {
            return Err(RiskViolation::PatternDisabled {
                pattern: stats.pattern_name.clone(),
            });
        }
        
        // Check daily trade limit for this pattern
        let pattern_trades_today = metrics.get_pattern_trade_count(&stats.pattern_name);
        if pattern_trades_today >= self.config.max_trades_per_pattern_per_day {
            return Err(RiskViolation::PatternDailyLimitReached {
                pattern: stats.pattern_name.clone(),
                count: pattern_trades_today,
                limit: self.config.max_trades_per_pattern_per_day,
            });
        }
        
        // Check consecutive losses
        if stats.consecutive_losses >= self.config.stop_pattern_after_consecutive_losses {
            return Err(RiskViolation::PatternConsecutiveLosses {
                pattern: stats.pattern_name.clone(),
                losses: stats.consecutive_losses,
            });
        }
        
        // Check win rate for auto-execution
        if stats.total_trades >= self.config.min_pattern_sample_size {
            if stats.win_rate < self.config.required_win_rate_for_auto {
                return Err(RiskViolation::PatternWinRateTooLow {
                    pattern: stats.pattern_name.clone(),
                    win_rate: stats.win_rate,
                    required: self.config.required_win_rate_for_auto,
                });
            }
        }
        
        Ok(())
    }
    
    fn check_min_balance(&self, account: &Account) -> Result<(), RiskViolation> {
        if account.portfolio_value < self.config.min_account_balance {
            return Err(RiskViolation::BelowMinimumBalance {
                current: account.portfolio_value,
                minimum: self.config.min_account_balance,
            });
        }
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RiskViolation {
    #[error("Daily loss limit exceeded: ${current_loss} (limit: ${limit})")]
    DailyLossLimitExceeded {
        current_loss: Decimal,
        limit: Decimal,
    },
    
    #[error("Daily loss percent exceeded: {current_percent}% (limit: {limit}%)")]
    DailyLossPercentExceeded {
        current_percent: Decimal,
        limit: Decimal,
    },
    
    #[error("Position size too large: ${order_size} (limit: ${limit})")]
    PositionSizeTooLarge {
        order_size: Decimal,
        limit: Decimal,
    },
    
    #[error("Position percent too large: {percent}% (limit: {limit}%)")]
    PositionPercentTooLarge {
        percent: Decimal,
        limit: Decimal,
    },
    
    #[error("Pattern day trader limit reached: {current_count} trades. Need ${required_value} (current: ${account_value})")]
    PatternDayTraderLimitReached {
        current_count: u32,
        account_value: Decimal,
        required_value: Decimal,
    },
    
    #[error("Pattern disabled: {pattern}")]
    PatternDisabled {
        pattern: String,
    },
    
    #[error("Pattern daily limit reached: {pattern} ({count}/{limit} trades today)")]
    PatternDailyLimitReached {
        pattern: String,
        count: u32,
        limit: u32,
    },
    
    #[error("Pattern has {losses} consecutive losses: {pattern}")]
    PatternConsecutiveLosses {
        pattern: String,
        losses: u32,
    },
    
    #[error("Pattern win rate too low: {pattern} ({win_rate}%, required: {required}%)")]
    PatternWinRateTooLow {
        pattern: String,
        win_rate: Decimal,
        required: Decimal,
    },
    
    #[error("Account below minimum balance: ${current} (minimum: ${minimum})")]
    BelowMinimumBalance {
        current: Decimal,
        minimum: Decimal,
    },
}

#[derive(Debug, Clone)]
pub struct DailyRiskMetrics {
    pub starting_balance: Decimal,
    pub current_balance: Decimal,
    pub realized_pl: Decimal,
    pub unrealized_pl: Decimal,
    pub total_pl: Decimal,
    pub day_trades_count: u32,
    pub trades_count: u32,
    pub pattern_trade_counts: std::collections::HashMap<String, u32>,
}

impl DailyRiskMetrics {
    fn get_pattern_trade_count(&self, pattern: &str) -> u32 {
        *self.pattern_trade_counts.get(pattern).unwrap_or(&0)
    }
}

#[derive(Debug, Clone)]
pub struct PatternStats {
    pub pattern_name: String,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub consecutive_losses: u32,
    pub win_rate: Decimal,
    pub is_enabled: bool,
}
```

---

## Pattern Recognition Engine

### Candlestick Pattern Detection

```rust
// src/patterns/candlestick.rs

use rust_decimal::Decimal;

pub struct CandlestickPattern {
    pub name: String,
    pub signal_type: SignalType,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum SignalType {
    Bullish,
    Bearish,
    Neutral,
}

pub struct PatternDetector {
    // Configuration for pattern detection sensitivity
    min_body_percent: Decimal,
    min_wick_ratio: Decimal,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            min_body_percent: Decimal::new(5, 1), // 0.5%
            min_wick_ratio: Decimal::new(15, 1), // 1.5x body
        }
    }
    
    /// Detect patterns in recent candles
    pub fn detect_patterns(&self, candles: &[Candle]) -> Vec<CandlestickPattern> {
        let mut patterns = Vec::new();
        
        if candles.len() < 3 {
            return patterns;
        }
        
        let len = candles.len();
        
        // Single candlestick patterns
        if let Some(pattern) = self.detect_hammer(&candles[len-1]) {
            patterns.push(pattern);
        }
        if let Some(pattern) = self.detect_shooting_star(&candles[len-1]) {
            patterns.push(pattern);
        }
        if let Some(pattern) = self.detect_doji(&candles[len-1]) {
            patterns.push(pattern);
        }
        
        // Two candlestick patterns
        if len >= 2 {
            if let Some(pattern) = self.detect_engulfing(&candles[len-2..]) {
                patterns.push(pattern);
            }
        }
        
        // Three candlestick patterns
        if len >= 3 {
            if let Some(pattern) = self.detect_morning_star(&candles[len-3..]) {
                patterns.push(pattern);
            }
            if let Some(pattern) = self.detect_evening_star(&candles[len-3..]) {
                patterns.push(pattern);
            }
            if let Some(pattern) = self.detect_three_white_soldiers(&candles[len-3..]) {
                patterns.push(pattern);
            }
        }
        
        patterns
    }
    
    fn detect_hammer(&self, candle: &Candle) -> Option<CandlestickPattern> {
        let body_size = (candle.close - candle.open).abs();
        let lower_wick = candle.open.min(candle.close) - candle.low;
        let upper_wick = candle.high - candle.open.max(candle.close);
        let total_range = candle.high - candle.low;
        
        // Hammer criteria:
        // - Small body in upper part of range
        // - Long lower wick (at least 2x body)
        // - Little or no upper wick
        
        if body_size / total_range < Decimal::new(30, 2) // Body < 30% of range
            && lower_wick >= body_size * Decimal::new(2, 0) // Lower wick >= 2x body
            && upper_wick < body_size / Decimal::new(2, 0) // Upper wick < 0.5x body
        {
            return Some(CandlestickPattern {
                name: "Hammer".to_string(),
                signal_type: SignalType::Bullish,
                confidence: 0.7,
            });
        }
        
        None
    }
    
    fn detect_shooting_star(&self, candle: &Candle) -> Option<CandlestickPattern> {
        let body_size = (candle.close - candle.open).abs();
        let upper_wick = candle.high - candle.open.max(candle.close);
        let lower_wick = candle.open.min(candle.close) - candle.low;
        let total_range = candle.high - candle.low;
        
        // Shooting star: inverse of hammer (bearish)
        if body_size / total_range < Decimal::new(30, 2)
            && upper_wick >= body_size * Decimal::new(2, 0)
            && lower_wick < body_size / Decimal::new(2, 0)
        {
            return Some(CandlestickPattern {
                name: "Shooting Star".to_string(),
                signal_type: SignalType::Bearish,
                confidence: 0.7,
            });
        }
        
        None
    }
    
    fn detect_doji(&self, candle: &Candle) -> Option<CandlestickPattern> {
        let body_size = (candle.close - candle.open).abs();
        let total_range = candle.high - candle.low;
        
        // Doji: very small body (open ≈ close)
        if body_size / total_range < Decimal::new(10, 2) {
            return Some(CandlestickPattern {
                name: "Doji".to_string(),
                signal_type: SignalType::Neutral,
                confidence: 0.6,
            });
        }
        
        None
    }
    
    fn detect_engulfing(&self, candles: &[Candle]) -> Option<CandlestickPattern> {
        if candles.len() < 2 {
            return None;
        }
        
        let prev = &candles[0];
        let curr = &candles[1];
        
        let prev_body_top = prev.open.max(prev.close);
        let prev_body_bottom = prev.open.min(prev.close);
        let curr_body_top = curr.open.max(curr.close);
        let curr_body_bottom = curr.open.min(curr.close);
        
        // Bullish engulfing: previous bearish, current bullish and engulfs
        if prev.close < prev.open  // Previous bearish
            && curr.close > curr.open  // Current bullish
            && curr_body_bottom < prev_body_bottom  // Engulfs from below
            && curr_body_top > prev_body_top  // Engulfs from above
        {
            return Some(CandlestickPattern {
                name: "Bullish Engulfing".to_string(),
                signal_type: SignalType::Bullish,
                confidence: 0.8,
            });
        }
        
        // Bearish engulfing
        if prev.close > prev.open
            && curr.close < curr.open
            && curr_body_top > prev_body_top
            && curr_body_bottom < prev_body_bottom
        {
            return Some(CandlestickPattern {
                name: "Bearish Engulfing".to_string(),
                signal_type: SignalType::Bearish,
                confidence: 0.8,
            });
        }
        
        None
    }
    
    fn detect_morning_star(&self, candles: &[Candle]) -> Option<CandlestickPattern> {
        if candles.len() < 3 {
            return None;
        }
        
        let first = &candles[0];
        let second = &candles[1];
        let third = &candles[2];
        
        // Morning star pattern:
        // 1. Long bearish candle
        // 2. Small body (gap down)
        // 3. Long bullish candle
        
        let first_bearish = first.close < first.open;
        let second_small = (second.close - second.open).abs() < (first.close - first.open).abs() / Decimal::new(3, 0);
        let third_bullish = third.close > third.open;
        let third_recovers = third.close > (first.open + first.close) / Decimal::new(2, 0);
        
        if first_bearish && second_small && third_bullish && third_recovers {
            return Some(CandlestickPattern {
                name: "Morning Star".to_string(),
                signal_type: SignalType::Bullish,
                confidence: 0.85,
            });
        }
        
        None
    }
    
    fn detect_evening_star(&self, candles: &[Candle]) -> Option<CandlestickPattern> {
        if candles.len() < 3 {
            return None;
        }
        
        let first = &candles[0];
        let second = &candles[1];
        let third = &candles[2];
        
        // Evening star: inverse of morning star
        let first_bullish = first.close > first.open;
        let second_small = (second.close - second.open).abs() < (first.close - first.open).abs() / Decimal::new(3, 0);
        let third_bearish = third.close < third.open;
        let third_reverses = third.close < (first.open + first.close) / Decimal::new(2, 0);
        
        if first_bullish && second_small && third_bearish && third_reverses {
            return Some(CandlestickPattern {
                name: "Evening Star".to_string(),
                signal_type: SignalType::Bearish,
                confidence: 0.85,
            });
        }
        
        None
    }
    
    fn detect_three_white_soldiers(&self, candles: &[Candle]) -> Option<CandlestickPattern> {
        if candles.len() < 3 {
            return None;
        }
        
        // Three consecutive strong bullish candles
        let all_bullish = candles.iter().all(|c| c.close > c.open);
        let ascending = candles[1].close > candles[0].close && candles[2].close > candles[1].close;
        let strong_bodies = candles.iter().all(|c| {
            let body = (c.close - c.open).abs();
            let range = c.high - c.low;
            body / range > Decimal::new(60, 2) // Body > 60% of range
        });
        
        if all_bullish && ascending && strong_bodies {
            return Some(CandlestickPattern {
                name: "Three White Soldiers".to_string(),
                signal_type: SignalType::Bullish,
                confidence: 0.9,
            });
        }
        
        None
    }
}
```

### Technical Indicators (Integration with Python)

```python
# python/indicators/technical.py

import pandas as pd
import numpy as np
from typing import Dict, List, Optional

class TechnicalIndicators:
    """Calculate various technical indicators"""
    
    @staticmethod
    def calculate_sma(prices: pd.Series, period: int) -> pd.Series:
        """Simple Moving Average"""
        return prices.rolling(window=period).mean()
    
    @staticmethod
    def calculate_ema(prices: pd.Series, period: int) -> pd.Series:
        """Exponential Moving Average"""
        return prices.ewm(span=period, adjust=False).mean()
    
    @staticmethod
    def calculate_rsi(prices: pd.Series, period: int = 14) -> pd.Series:
        """Relative Strength Index"""
        delta = prices.diff()
        gain = (delta.where(delta > 0, 0)).rolling(window=period).mean()
        loss = (-delta.where(delta < 0, 0)).rolling(window=period).mean()
        
        rs = gain / loss
        rsi = 100 - (100 / (1 + rs))
        return rsi
    
    @staticmethod
    def calculate_macd(
        prices: pd.Series,
        fast_period: int = 12,
        slow_period: int = 26,
        signal_period: int = 9
    ) -> Dict[str, pd.Series]:
        """Moving Average Convergence Divergence"""
        fast_ema = TechnicalIndicators.calculate_ema(prices, fast_period)
        slow_ema = TechnicalIndicators.calculate_ema(prices, slow_period)
        
        macd_line = fast_ema - slow_ema
        signal_line = macd_line.ewm(span=signal_period, adjust=False).mean()
        histogram = macd_line - signal_line
        
        return {
            'macd': macd_line,
            'signal': signal_line,
            'histogram': histogram
        }
    
    @staticmethod
    def calculate_bollinger_bands(
        prices: pd.Series,
        period: int = 20,
        num_std: float = 2.0
    ) -> Dict[str, pd.Series]:
        """Bollinger Bands"""
        sma = TechnicalIndicators.calculate_sma(prices, period)
        std = prices.rolling(window=period).std()
        
        upper_band = sma + (std * num_std)
        lower_band = sma - (std * num_std)
        
        return {
            'upper': upper_band,
            'middle': sma,
            'lower': lower_band
        }
    
    @staticmethod
    def calculate_atr(
        high: pd.Series,
        low: pd.Series,
        close: pd.Series,
        period: int = 14
    ) -> pd.Series:
        """Average True Range"""
        high_low = high - low
        high_close = np.abs(high - close.shift())
        low_close = np.abs(low - close.shift())
        
        true_range = pd.concat([high_low, high_close, low_close], axis=1).max(axis=1)
        atr = true_range.rolling(window=period).mean()
        
        return atr
    
    @staticmethod
    def calculate_all_indicators(df: pd.DataFrame) -> pd.DataFrame:
        """Calculate all indicators for a dataframe of OHLCV data"""
        result = df.copy()
        
        # Moving averages
        result['sma_20'] = TechnicalIndicators.calculate_sma(df['close'], 20)
        result['sma_50'] = TechnicalIndicators.calculate_sma(df['close'], 50)
        result['ema_12'] = TechnicalIndicators.calculate_ema(df['close'], 12)
        result['ema_26'] = TechnicalIndicators.calculate_ema(df['close'], 26)
        
        # RSI
        result['rsi'] = TechnicalIndicators.calculate_rsi(df['close'])
        
        # MACD
        macd = TechnicalIndicators.calculate_macd(df['close'])
        result['macd'] = macd['macd']
        result['macd_signal'] = macd['signal']
        result['macd_histogram'] = macd['histogram']
        
        # Bollinger Bands
        bb = TechnicalIndicators.calculate_bollinger_bands(df['close'])
        result['bb_upper'] = bb['upper']
        result['bb_middle'] = bb['middle']
        result['bb_lower'] = bb['lower']
        
        # ATR
        result['atr'] = TechnicalIndicators.calculate_atr(
            df['high'], df['low'], df['close']
        )
        
        return result
```

---

## Development Roadmap (4-6 Months)

### Phase 1: Foundation (Weeks 1-4)

**Week 1-2: Project Setup & Core Infrastructure**
- [ ] Initialize Tauri project structure
- [ ] Set up Rust workspace with core modules
- [ ] Configure database (DuckDB/SQLite)
- [ ] Implement broker abstraction traits
- [ ] Create Alpaca adapter (basic connection)
- [ ] Set up Python bridge architecture
- [ ] Configure logging and error handling
- [ ] Create configuration management system

**Week 3-4: Market Data Pipeline**
- [ ] Implement WebSocket connection to Alpaca
- [ ] Real-time quote ingestion
- [ ] 1-minute candle aggregation
- [ ] Data normalization and validation
- [ ] Database storage for historical data
- [ ] Basic REST API endpoints for data retrieval

**Deliverable:** Working market data pipeline storing 1-min candles

---

### Phase 2: Risk Management & Account Management (Weeks 5-8)

**Week 5-6: Risk Management System**
- [ ] Implement risk configuration system
- [ ] Daily loss tracking
- [ ] Position size validation
- [ ] PDT rule enforcement
- [ ] Account balance monitoring
- [ ] Risk violation alerts
- [ ] Database schema for risk metrics

**Week 7-8: Account & Position Management**
- [ ] Account information retrieval
- [ ] Position tracking and updates
- [ ] Real-time P&L calculations
- [ ] Portfolio summary views
- [ ] Trade history recording

**Deliverable:** Complete risk management system with real-time monitoring

---

### Phase 3: Pattern Recognition & Signals (Weeks 9-12)

**Week 9-10: Candlestick Pattern Detection**
- [ ] Implement pattern detector in Rust
- [ ] Single-candle patterns (Hammer, Doji, Shooting Star)
- [ ] Two-candle patterns (Engulfing)
- [ ] Three-candle patterns (Morning/Evening Star)
- [ ] Pattern confidence scoring
- [ ] Pattern signal storage

**Week 11-12: Technical Indicators & Signal Generation**
- [ ] Python integration for indicators (SMA, EMA, RSI, MACD)
- [ ] Indicator calculation pipeline
- [ ] Signal generation logic
- [ ] Pattern + indicator confirmation
- [ ] Alert system for detected patterns

**Deliverable:** Working pattern recognition with technical indicator support

---

### Phase 4: Order Execution & Automation (Weeks 13-16)

**Week 13-14: Order Management**
- [ ] Order placement (market, limit orders)
- [ ] Order cancellation
- [ ] Order status tracking
- [ ] Fill notifications
- [ ] Order history
- [ ] Advanced order types (stop-loss, trailing stops, brackets)

**Week 15-16: Automated Execution**
- [ ] Manual execution mode (with confirmation)
- [ ] Pattern-based auto-execution
- [ ] Pre-trade risk validation
- [ ] Post-trade tracking
- [ ] Pattern performance tracking
- [ ] Auto-execution enable/disable logic based on win rate

**Deliverable:** Complete order execution system with manual and auto modes

---

### Phase 5: User Interface (Weeks 17-20)

**Week 17-18: Core UI Components**
- [ ] Real-time charting with TradingView Lightweight Charts
- [ ] Order entry forms
- [ ] Position dashboard
- [ ] Watchlist management
- [ ] Pattern alerts display
- [ ] Risk metrics dashboard

**Week 19-20: Advanced UI Features**
- [ ] Multi-window support (charts, watchlists, orders)
- [ ] Customizable layouts
- [ ] Trade journal interface
- [ ] Pattern performance visualizations
- [ ] Alert configuration UI
- [ ] Risk settings configuration

**Deliverable:** Complete desktop UI with all trading features

---

### Phase 6: Backtesting & Testing (Weeks 21-24)

**Week 21-22: Backtesting Engine**
- [ ] Historical data retrieval
- [ ] Backtest execution engine
- [ ] Pattern strategy implementation
- [ ] Performance metrics calculation
- [ ] Backtest result storage and visualization
- [ ] Strategy comparison tools

**Week 23-24: Comprehensive Testing & Refinement**
- [ ] Paper trading validation (full system)
- [ ] Pattern accuracy testing
- [ ] Risk management testing
- [ ] Performance optimization
- [ ] Bug fixes and edge cases
- [ ] UI/UX refinements
- [ ] Documentation

**Deliverable:** Fully tested system ready for paper trading validation

---

## Deployment & Distribution

### Build Configuration

```toml
# Cargo.toml
[package]
name = "daytrader"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "1.5", features = ["api-all"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["json"] }
tokio-tungstenite = "0.21"
rusqlite = { version = "0.30", features = ["bundled"] }
rust_decimal = "1.33"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
config = "0.13"

[dependencies.tauri]
version = "1.5"
features = ["api-all"]

[build-dependencies]
tauri-build = { version = "1.5", features = [] }
```

### Distribution Strategy

**Development:**
- Local builds for testing
- Paper trading environment

**Production:**
- Code signing for Windows/macOS
- Notarization for macOS
- Installer creation (MSI for Windows, DMG for macOS, AppImage for Linux)
- Auto-update mechanism via Tauri updater

---

## Security Considerations

### API Key Management
- Store API keys encrypted in OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service)
- Never commit API keys to version control
- Use environment variables for development

### Data Protection
- Encrypt sensitive data at rest
- Secure WebSocket connections (WSS)
- HTTPS for all REST API calls
- Input validation on all user inputs

### Code Security
- Regular dependency audits (`cargo audit`)
- Static analysis with Clippy
- Memory safety via Rust
- Sandboxed frontend via Tauri

---

## Monitoring & Observability

### Logging Strategy
- Structured logging with tracing
- Log levels: ERROR, WARN, INFO, DEBUG, TRACE
- Separate log files for:
  - Application events
  - Trading activity
  - Risk violations
  - API errors

### Metrics to Track
- Order execution latency
- Pattern detection accuracy
- Win/loss ratios per pattern
- Daily P&L
- Risk violations
- API error rates
- WebSocket connection stability

---

## Cost Breakdown (Monthly)

**Initial Phase (Paper Trading):**
- Infrastructure: $0 (local development)
- Alpaca API: $0 (paper trading free)
- **Total: $0/month**

**Production Phase (Live Trading, 1-10 users):**
- Server/Infrastructure: $50-100 (if cloud hosted for backups)
- Alpaca API: $0 (included with account)
- Data Storage: $10-20
- Monitoring/Logging: $0-30 (optional)
- **Total: $60-150/month**

**Scaling Considerations:**
- Costs scale minimally with users (1-10)
- Trading commissions based on volume (Alpaca is commission-free for stocks)
- Additional data feeds if needed: $200-400/month (optional)

---

## Success Metrics

### Technical Metrics
- Order execution latency < 1 second (target: 100ms-500ms)
- System uptime > 99% during market hours
- Pattern detection latency < 100ms per symbol
- Zero critical bugs in risk management system

### Business Metrics
- Pattern win rate > 55% before enabling auto-execution
- Daily loss limits never exceeded
- PDT violations: 0
- Profitable paper trading for 30+ days before going live

### User Experience Metrics
- Time to place order < 5 seconds
- Chart rendering < 200ms
- Alert notification < 1 second from detection
- UI responsiveness (no freezing during updates)

---

## Next Steps

1. **Review and approve this architecture**
2. **Set up development environment**
3. **Begin Phase 1: Foundation (Weeks 1-4)**
4. **Weekly progress reviews**
5. **Adjust timeline based on progress**

---

**Document Version:** 1.0  
**Last Updated:** November 15, 2025  
**Next Review:** After Phase 1 completion
