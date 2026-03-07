 # Phase 0: Development Environment Setup Plan

**Timeline:** Week 0 (5-7 days)  
**Goal:** Establish a solid foundation for development with proper tooling, structure, and initial configuration

---

## Prerequisites

### Required Software

**Rust Development:**
- [ ] Rust 1.75+ (install via rustup)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update
  rustc --version  # Verify 1.75+
  ```

- [ ] Cargo tools
  ```bash
  cargo install cargo-watch      # Auto-rebuild on file changes
  cargo install cargo-audit       # Security vulnerability scanning
  cargo install cargo-tarpaulin   # Code coverage
  cargo install cargo-edit        # cargo add/rm commands
  ```

**Frontend Development:**
- [ ] Node.js 18+ and npm
  ```bash
  node --version  # Verify 18+
  npm --version
  ```

- [ ] Tauri CLI
  ```bash
  npm install -g @tauri-apps/cli
  ```

**Python (for future analytics):**
- [ ] Python 3.11+
  ```bash
  python3 --version  # Verify 3.11+
  pip install --upgrade pip
  ```

**Database Tools:**
- [ ] SQLite CLI (for database inspection)
  ```bash
  sqlite3 --version
  ```

**Development Tools:**
- [ ] Git
- [ ] VS Code or RustRover (recommended IDEs)
- [ ] Postman or similar (for API testing)

---

## Project Structure Setup

### Directory Layout

```
daytrader/
├── Cargo.toml                 # Rust workspace configuration
├── Cargo.lock                 # Dependency lock file
├── .gitignore                 # Git ignore rules
├── README.md                  # Project documentation
├── LICENSE                    # License file
├── .github/
│   └── workflows/
│       ├── ci.yml             # Continuous integration
│       └── security.yml       # Security scanning
├── docs/                      # Documentation
│   ├── architecture-analysis.md
│   ├── project-knowledge-base.md
│   ├── technical-architecture.md
│   └── api/                   # API documentation
├── src/                       # Rust backend source
│   ├── main.rs                # Application entry point
│   ├── lib.rs                 # Library root
│   ├── brokers/               # Broker abstraction layer
│   │   ├── mod.rs
│   │   ├── traits.rs          # Broker traits
│   │   ├── alpaca.rs          # Alpaca implementation
│   │   └── mock.rs            # Mock broker for testing
│   ├── data/                  # Market data management
│   │   ├── mod.rs
│   │   ├── ingestion.rs       # WebSocket data ingestion
│   │   ├── storage.rs         # Database operations
│   │   ├── cache.rs           # In-memory caching
│   │   └── validation.rs      # Data validation
│   ├── patterns/              # Pattern recognition
│   │   ├── mod.rs
│   │   ├── candlestick.rs     # Candlestick patterns
│   │   ├── indicators.rs      # Technical indicators
│   │   └── signals.rs         # Signal generation
│   ├── risk/                  # Risk management
│   │   ├── mod.rs
│   │   ├── manager.rs         # Risk manager
│   │   ├── rules.rs           # Risk rules
│   │   └── audit.rs           # Audit logging
│   ├── execution/             # Order execution
│   │   ├── mod.rs
│   │   ├── engine.rs          # Execution engine
│   │   ├── orders.rs          # Order management
│   │   └── positions.rs       # Position tracking
│   ├── events/                # Event bus
│   │   ├── mod.rs
│   │   └── bus.rs             # Event bus implementation
│   ├── config/                # Configuration
│   │   ├── mod.rs
│   │   └── settings.rs        # Settings management
│   ├── db/                    # Database
│   │   ├── mod.rs
│   │   ├── schema.rs          # Database schema
│   │   └── migrations/        # SQL migrations
│   └── utils/                 # Utilities
│       ├── mod.rs
│       ├── logging.rs         # Logging setup
│       └── errors.rs          # Error types
├── src-tauri/                 # Tauri configuration
│   ├── tauri.conf.json        # Tauri settings
│   ├── Cargo.toml             # Tauri dependencies
│   ├── src/
│   │   └── main.rs            # Tauri entry point
│   └── icons/                 # Application icons
├── src-ui/                    # Frontend source
│   ├── package.json           # Node dependencies
│   ├── tsconfig.json          # TypeScript config
│   ├── vite.config.ts         # Vite configuration
│   ├── index.html             # HTML entry point
│   ├── src/
│   │   ├── main.tsx           # React entry point
│   │   ├── App.tsx            # Main app component
│   │   ├── components/        # React components
│   │   ├── hooks/             # Custom hooks
│   │   ├── stores/            # State management
│   │   ├── types/             # TypeScript types
│   │   └── utils/             # Utilities
│   └── public/                # Static assets
├── python/                    # Python analytics (future)
│   ├── requirements.txt       # Python dependencies
│   ├── indicators/            # Technical indicators
│   └── backtest/              # Backtesting engine
├── tests/                     # Integration tests
│   ├── integration/
│   ├── fixtures/              # Test data
│   └── mocks/                 # Mock implementations
├── benches/                   # Performance benchmarks
│   └── pattern_detection.rs
├── scripts/                   # Utility scripts
│   ├── setup_db.sh            # Database initialization
│   └── run_tests.sh           # Test runner
└── data/                      # Local data (gitignored)
    ├── db/                    # Database files
    ├── logs/                  # Log files
    └── config/                # User configuration
```

---

## Step-by-Step Setup

### Step 1: Initialize Project

```bash
# Create project directory
mkdir daytrader
cd daytrader

# Initialize Git repository
git init
git branch -M main

# Create initial Cargo workspace
cargo init --name daytrader

# Fix Cargo.toml edition
# Edit Cargo.toml and change edition = "2024" to edition = "2021"
```

### Step 2: Configure Cargo.toml

Create the complete [`Cargo.toml`](Cargo.toml:1) with all dependencies:

```toml
[package]
name = "daytrader"
version = "0.1.0"
edition = "2021"
authors = ["Mike"]
description = "Pattern-based day trading platform"
license = "MIT"

[workspace]
members = [".", "src-tauri"]

[dependencies]
# Core Framework
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Networking
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
tokio-tungstenite = { version = "0.21", features = ["rustls-tls-webpki-roots"] }

# Database
rusqlite = { version = "0.30", features = ["bundled", "chrono"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono", "decimal"] }

# Data Structures
rust_decimal = { version = "1.33", features = ["serde"] }
chrono = { version = "0.4", features = ["serde"] }
dashmap = "5.5"
uuid = { version = "1.6", features = ["v4", "serde"] }

# Technical Analysis
ta = "0.5"

# Concurrency
crossbeam = "0.8"
async-trait = "0.1"
futures = "0.3"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# Configuration
config = "0.13"
toml = "0.8"

# Rate Limiting
tower = { version = "0.4", features = ["limit", "timeout"] }

# Security
keyring = "2.0"
sha2 = "0.10"

# Utilities
once_cell = "1.19"
parking_lot = "0.12"

[dev-dependencies]
mockall = "0.12"
criterion = { version = "0.5", features = ["html_reports"] }
tokio-test = "0.4"
tempfile = "3.8"
proptest = "1.4"

[[bench]]
name = "pattern_detection"
harness = false

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true

[profile.dev]
opt-level = 0
debug = true
```

### Step 3: Create .gitignore

```bash
cat > .gitignore << 'EOF'
# Rust
/target/
Cargo.lock
**/*.rs.bk
*.pdb

# Tauri
/src-tauri/target/

# Node
node_modules/
dist/
dist-ssr/
*.local

# Python
__pycache__/
*.py[cod]
*$py.class
.Python
venv/
env/

# Database
*.db
*.db-shm
*.db-wal
*.sqlite
*.sqlite3

# Logs
*.log
logs/

# Configuration (contains API keys)
data/config/*.toml
!data/config/default.toml

# IDE
.vscode/
.idea/
*.swp
*.swo
*~
.DS_Store

# OS
Thumbs.db

# Test artifacts
/coverage/
*.profraw
*.profdata

# Benchmarks
/target/criterion/
EOF
```

### Step 4: Initialize Tauri

```bash
# Create Tauri project structure
npm create tauri-app@latest

# When prompted:
# - Project name: daytrader
# - Choose: React + TypeScript
# - Package manager: npm

# This creates src-tauri/ and src-ui/ directories
```

### Step 5: Configure Database Schema

Create [`src/db/schema.sql`](src/db/schema.sql:1):

```sql
-- Database schema for day trading platform
-- SQLite version

-- Accounts
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    broker TEXT NOT NULL,
    account_number TEXT NOT NULL UNIQUE,
    account_type TEXT NOT NULL CHECK(account_type IN ('paper', 'live')),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Market Data (1-min candles)
CREATE TABLE IF NOT EXISTS candles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume INTEGER NOT NULL,
    broker TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, timestamp, broker)
);

CREATE INDEX IF NOT EXISTS idx_candles_symbol_time ON candles(symbol, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_candles_timestamp ON candles(timestamp DESC);

-- Positions
CREATE TABLE IF NOT EXISTS positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
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
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    broker_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    order_type TEXT NOT NULL CHECK(order_type IN ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop')),
    quantity INTEGER NOT NULL,
    filled_quantity INTEGER DEFAULT 0,
    limit_price REAL,
    stop_price REAL,
    trailing_percent REAL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'filled', 'partial', 'cancelled', 'rejected')),
    time_in_force TEXT DEFAULT 'day' CHECK(time_in_force IN ('day', 'gtc', 'ioc', 'fok')),
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMP,
    filled_avg_price REAL,
    commission REAL DEFAULT 0.0,
    error_message TEXT,
    is_pattern_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_orders_account_time ON orders(account_id, submitted_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);

-- Trades (Filled Orders)
CREATE TABLE IF NOT EXISTS trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    quantity INTEGER NOT NULL,
    price REAL NOT NULL,
    commission REAL DEFAULT 0.0,
    realized_pl REAL,
    executed_at TIMESTAMP NOT NULL,
    is_day_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX IF NOT EXISTS idx_trades_account_time ON trades(account_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);

-- Pattern Signals
CREATE TABLE IF NOT EXISTS pattern_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    pattern_name TEXT NOT NULL,
    signal_type TEXT NOT NULL CHECK(signal_type IN ('buy', 'sell')),
    confidence REAL CHECK(confidence >= 0 AND confidence <= 1),
    detected_at TIMESTAMP NOT NULL,
    candle_timestamp TIMESTAMP NOT NULL,
    price_at_detection REAL NOT NULL,
    acted_upon BOOLEAN DEFAULT FALSE,
    order_id INTEGER,
    outcome TEXT CHECK(outcome IN ('win', 'loss', 'pending', 'ignored')),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX IF NOT EXISTS idx_signals_symbol_time ON pattern_signals(symbol, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_pattern ON pattern_signals(pattern_name);

-- Pattern Performance Tracking
CREATE TABLE IF NOT EXISTS pattern_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_name TEXT NOT NULL UNIQUE,
    total_signals INTEGER DEFAULT 0,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit REAL DEFAULT 0.0,
    win_rate REAL DEFAULT 0.0,
    avg_profit_per_trade REAL DEFAULT 0.0,
    consecutive_losses INTEGER DEFAULT 0,
    is_enabled BOOLEAN DEFAULT TRUE,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily Risk Tracking
CREATE TABLE IF NOT EXISTS daily_risk_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
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

CREATE INDEX IF NOT EXISTS idx_risk_metrics_date ON daily_risk_metrics(trading_date DESC);

-- Watchlists
CREATE TABLE IF NOT EXISTS watchlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS watchlist_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    watchlist_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id) ON DELETE CASCADE,
    UNIQUE(watchlist_id, symbol)
);

-- Price Alerts
CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('price_above', 'price_below', 'pattern_detected')),
    threshold_value REAL,
    pattern_name TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    triggered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Audit Log
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity IN ('info', 'warning', 'error', 'critical')),
    user_id TEXT,
    account_id INTEGER,
    description TEXT NOT NULL,
    metadata TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_severity ON audit_log(severity);

-- Application Settings
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert default settings
INSERT OR IGNORE INTO app_settings (key, value) VALUES
    ('risk.max_daily_loss_dollars', '500.00'),
    ('risk.max_daily_loss_percent', '2.00'),
    ('risk.max_position_size_dollars', '5000.00'),
    ('risk.max_position_size_percent', '10.00'),
    ('risk.max_concurrent_positions', '5'),
    ('risk.enforce_pdt_rule', 'true'),
    ('risk.pdt_min_equity', '25000.00'),
    ('risk.max_day_trades_under_pdt', '3'),
    ('risk.max_trades_per_pattern_per_day', '5'),
    ('risk.stop_pattern_after_consecutive_losses', '3'),
    ('risk.required_win_rate_for_auto', '60.00'),
    ('risk.min_pattern_sample_size', '20'),
    ('risk.min_account_balance', '1000.00'),
    ('risk.emergency_liquidate_threshold', '500.00');
```

### Step 6: Create Configuration Files

Create [`config/default.toml`](config/default.toml:1):

```toml
# Default configuration for day trading platform
# DO NOT store API keys here - use environment variables or OS keychain

[app]
name = "DayTrader"
version = "0.1.0"
environment = "development"  # development, paper, production

[broker]
provider = "alpaca"
is_paper = true
base_url = "https://paper-api.alpaca.markets"
ws_url = "wss://stream.data.alpaca.markets/v2/iex"

[database]
path = "data/db/daytrader.db"
max_connections = 10
enable_wal = true

[logging]
level = "info"  # trace, debug, info, warn, error
file_path = "data/logs/daytrader.log"
max_file_size_mb = 100
max_files = 10

[risk]
# Daily loss limits
max_daily_loss_dollars = 500.00
max_daily_loss_percent = 2.00
halt_trading_on_limit = true

# Position sizing
max_position_size_dollars = 5000.00
max_position_size_percent = 10.00
max_concurrent_positions = 5

# PDT protection
enforce_pdt_rule = true
pdt_min_equity = 25000.00
max_day_trades_under_pdt = 3

# Pattern-specific rules
max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.00
min_pattern_sample_size = 20

# Account safeguards
min_account_balance = 1000.00
emergency_liquidate_threshold = 500.00

[patterns]
enabled = [
    "hammer",
    "shooting_star",
    "doji",
    "bullish_engulfing",
    "bearish_engulfing"
]

[indicators]
sma_periods = [20, 50]
ema_periods = [12, 26]
rsi_period = 14
macd_fast = 12
macd_slow = 26
macd_signal = 9

[performance]
max_symbols_tracked = 50
candle_cache_size = 100  # Per symbol
pattern_detection_workers = 4
websocket_reconnect_max_attempts = 10
websocket_reconnect_delay_ms = 1000
order_timeout_seconds = 30
```

### Step 7: Set Up CI/CD

Create [`.github/workflows/ci.yml`](.github/workflows/ci.yml:1):

```yaml
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-git-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache target directory
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-target-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        run: cargo test --all-features --verbose
      
      - name: Run doc tests
        run: cargo test --doc --verbose

  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Check formatting
        run: cargo fmt -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
      
      - name: Run security audit
        run: cargo audit

  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      
      - name: Generate coverage
        run: cargo tarpaulin --out Xml --verbose
      
      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
          fail_ci_if_error: false
```

### Step 8: Create Initial Source Files

Create [`src/lib.rs`](src/lib.rs:1):

```rust
//! Day Trading Platform
//! 
//! A comprehensive pattern-based day trading system with automated execution capabilities.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    clippy::all,
    clippy::pedantic
)]

pub mod brokers;
pub mod config;
pub mod data;
pub mod db;
pub mod events;
pub mod execution;
pub mod patterns;
pub mod risk;
pub mod utils;

pub use config::Settings;
pub use utils::errors::Result;
```

Create [`src/utils/errors.rs`](src/utils/errors.rs:1):

```rust
//! Error types for the application

use thiserror::Error;

/// Application result type
pub type Result<T> = std::result::Result<T, AppError>;

/// Application error types
#[derive(Debug, Error)]
pub enum AppError {
    /// Broker-related errors
    #[error("Broker error: {0}")]
    Broker(#[from] crate::brokers::BrokerError),
    
    /// Database errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    /// Risk management errors
    #[error("Risk violation: {0}")]
    Risk(String),
    
    /// Network errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    /// WebSocket errors
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// Generic errors
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
```

### Step 9: Initialize Database

Create [`scripts/setup_db.sh`](scripts/setup_db.sh:1):

```bash
#!/bin/bash
# Database initialization script

set -e

DB_DIR="data/db"
DB_FILE="$DB_DIR/daytrader.db"
SCHEMA_FILE="src/db/schema.sql"

echo "Setting up database..."

# Create directory if it doesn't exist
mkdir -p "$DB_DIR"

# Check if database already exists
if [ -f "$DB_FILE" ]; then
    read -p "Database already exists. Recreate? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm "$DB_FILE"
        echo "Removed existing database"
    else
        echo "Keeping existing database"
        exit 0
    fi
fi

# Create database and apply schema
echo "Creating database and applying schema..."
sqlite3 "$DB_FILE" < "$SCHEMA_FILE"

echo "Database setup complete: $DB_FILE"
echo "You can inspect it with: sqlite3 $DB_FILE"
```

Make it executable:
```bash
chmod +x scripts/setup_db.sh
```

### Step 10: Create README

Create [`README.md`](README.md:1):

```markdown
# DayTrader - Pattern-Based Day Trading Platform

A comprehensive day trading platform focused on candlestick pattern recognition with automated execution capabilities.

## Features

- Real-time market data ingestion (1-minute candles)
- Candlestick pattern detection
- Technical indicator analysis
- Comprehensive risk management
- Manual and automated order execution
- Paper trading support
- Desktop application (Tauri)

## Prerequisites

- Rust 1.75+
- Node.js 18+
- Python 3.11+ (for analytics)
- SQLite 3

## Quick Start

### 1. Clone and Setup

```bash
git clone <repository-url>
cd daytrader
./scripts/setup_db.sh
```

### 2. Configure API Keys

```bash
# Set Alpaca API credentials (paper trading)
export ALPACA_API_KEY="your_api_key"
export ALPACA_API_SECRET="your_api_secret"
```

### 3. Run Development Server

```bash
# Terminal 1: Run Rust backend
cargo run

# Terminal 2: Run frontend
cd src-ui
npm install
npm run dev
```

### 4. Run Tests

```bash
cargo test
cargo clippy
cargo fmt --check
```

## Project Structure

See [docs/technical-architecture.md](docs/technical-architecture.md) for detailed architecture.

## Development Roadmap

- [x] Phase 0: Setup (Week 0)
- [ ] Phase 1: Core Infrastructure (Weeks 1-4)
- [ ] Phase 2: Risk Management (Weeks 5-8)
- [ ] Phase 3: Pattern Recognition (Weeks 9-12)
- [ ] Phase 4: Order Execution (Weeks 13-16)
- [ ] Phase 5: User Interface (Weeks 17-20)
- [ ] Phase 6: Testing & Validation (Weeks 21-24)

## Documentation

- [Project Knowledge Base](docs/project-knowledge-base.md)
- [Technical Architecture](docs/technical-architecture.md)
- [Architecture Analysis](docs/architecture-analysis.md)
- [Phase 0 Setup Plan](docs/phase-0-setup-plan.md)

## License

MIT

## Disclaimer

This software is for educational purposes only. Trading involves substantial risk of loss. Use at your own risk.
```

---

## Verification Checklist

After completing setup, verify everything works:

### Rust Build
```bash
cd daytrader
cargo build
cargo test
cargo clippy
```

Expected: All commands succeed without errors

### Database
```bash
./scripts/setup_db.sh
sqlite3 data/db/daytrader.db ".tables"
```

Expected: List of tables displayed

### Frontend (if Tauri initialized)
```bash
cd src-ui
npm install
npm run dev
```

Expected: Development server starts

### Git
```bash
git status
git add .
git commit -m "Initial project setup"
```

Expected: Clean commit

---

## Next Steps

After Phase 0 completion:

1. **Review setup** with team
2. **Begin Phase 1** - Core Infrastructure
3. **Set up project management** (GitHub Projects, Jira, etc.)
4. **Schedule weekly reviews**

---

## Troubleshooting

### Rust compilation errors
- Verify Rust version: `rustc --version` (should be 1.75+)
- Update Rust: `rustup update`
- Clean build: `cargo clean && cargo build`

### Database errors
- Check SQLite installation: `sqlite3 --version`
- Verify schema file exists: `ls src/db/schema.sql`
- Check permissions: `ls -la data/db/`

### Node/npm errors
- Verify Node version: `node --version` (should be 18+)
- Clear cache: `npm cache clean --force`
- Delete node_modules: `rm -rf node_modules && npm install`

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Estimated Completion Time:** 5-7 days
