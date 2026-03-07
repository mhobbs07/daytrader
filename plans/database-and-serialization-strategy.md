o# Database & Serialization Strategy

**Version:** 1.0  
**Date:** December 16, 2025  
**Status:** Architectural Decision

---

## Executive Summary

This document outlines the recommended database architecture and serialization strategy for the day trading platform, including analysis of Protocol Buffers and Cap'n Proto for API communication.

---

## Database Architecture: Hybrid Approach

### Recommended Strategy

**Use both SQLite and DuckDB for optimal performance:**

```
┌─────────────────────────────────────────────────────┐
│           Database Architecture                      │
├─────────────────────────────────────────────────────┤
│                                                      │
│  SQLite (Transactional/OLTP)                        │
│  ├── Orders (real-time writes, frequent updates)    │
│  ├── Positions (concurrent access, live tracking)   │
│  ├── Trades (immediate recording)                   │
│  ├── Daily risk metrics (real-time calculations)    │
│  ├── Watchlists (user management)                   │
│  ├── Alerts (active monitoring)                     │
│  ├── Audit log (compliance tracking)                │
│  └── App settings (configuration)                   │
│                                                      │
│  DuckDB (Analytical/OLAP)                           │
│  ├── Historical candles (bulk queries, analytics)   │
│  ├── Pattern signals (historical analysis)          │
│  ├── Pattern stats (aggregations)                   │
│  ├── Backtest results (complex queries)             │
│  ├── Backtest trades (large datasets)               │
│  └── Performance reports (analytics)                │
│                                                      │
└─────────────────────────────────────────────────────┘
```

### Rationale

**SQLite for Transactional Data:**
- ✅ Battle-tested, extremely reliable
- ✅ Excellent for concurrent reads/writes
- ✅ ACID compliance for critical data
- ✅ Mature ecosystem and tooling
- ✅ Perfect for real-time order/position tracking
- ✅ Low latency for frequent updates

**DuckDB for Analytical Data:**
- ✅ 10-100x faster for analytical queries
- ✅ Optimized for time-series aggregations
- ✅ Excellent for backtesting large datasets
- ✅ Efficient storage with Parquet format
- ✅ SQL interface (easy migration from SQLite)
- ✅ Perfect for historical analysis

**Why Not Just One?**
- SQLite alone: Slow for large analytical queries (backtesting)
- DuckDB alone: Less mature for concurrent transactional workloads
- Hybrid: Best of both worlds

---

## Database Implementation

### SQLite Configuration

```rust
// src/db/sqlite.rs

use rusqlite::{Connection, params};
use std::path::Path;

pub struct SqliteDb {
    conn: Connection,
}

impl SqliteDb {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Enable WAL mode for better concurrency
        conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -64000;  -- 64MB cache
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 30000000000;  -- 30GB mmap
        ")?;
        
        Ok(Self { conn })
    }
    
    pub fn init_schema(&self) -> Result<()> {
        // Create transactional tables
        self.conn.execute_batch(include_str!("schema_sqlite.sql"))?;
        Ok(())
    }
}
```

**SQLite Schema (Transactional Tables):**
```sql
-- src/db/schema_sqlite.sql

-- Orders (real-time tracking)
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    broker_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    order_type TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    filled_quantity INTEGER DEFAULT 0,
    limit_price REAL,
    stop_price REAL,
    status TEXT NOT NULL,
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMP,
    filled_avg_price REAL,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX idx_orders_status ON orders(status) WHERE status IN ('pending', 'partial');
CREATE INDEX idx_orders_symbol ON orders(symbol);

-- Positions (live tracking)
CREATE TABLE IF NOT EXISTS positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_entry_price REAL NOT NULL,
    current_price REAL,
    market_value REAL,
    unrealized_pl REAL,
    opened_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, symbol)
);

-- Trades (immediate recording)
CREATE TABLE IF NOT EXISTS trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    price REAL NOT NULL,
    executed_at TIMESTAMP NOT NULL,
    is_day_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX idx_trades_account_time ON trades(account_id, executed_at DESC);

-- Daily risk metrics (real-time calculations)
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
    is_trading_halted BOOLEAN DEFAULT FALSE,
    halt_reason TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, trading_date)
);

-- Audit log (compliance)
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    account_id INTEGER,
    description TEXT NOT NULL,
    metadata TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX idx_audit_timestamp ON audit_log(timestamp DESC);
```

### DuckDB Configuration

```rust
// src/db/duckdb.rs

use duckdb::{Connection, params};
use std::path::Path;

pub struct DuckDbAnalytics {
    conn: Connection,
}

impl DuckDbAnalytics {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Optimize for analytics
        conn.execute_batch("
            SET memory_limit='4GB';
            SET threads=4;
            SET enable_progress_bar=true;
        ")?;
        
        Ok(Self { conn })
    }
    
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("schema_duckdb.sql"))?;
        Ok(())
    }
}
```

**DuckDB Schema (Analytical Tables):**
```sql
-- src/db/schema_duckdb.sql

-- Historical candles (optimized for analytics)
CREATE TABLE IF NOT EXISTS candles (
    symbol VARCHAR NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open DECIMAL(18,4) NOT NULL,
    high DECIMAL(18,4) NOT NULL,
    low DECIMAL(18,4) NOT NULL,
    close DECIMAL(18,4) NOT NULL,
    volume BIGINT NOT NULL,
    broker VARCHAR NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Partition by symbol and date for faster queries
CREATE INDEX idx_candles_symbol_time ON candles(symbol, timestamp DESC);

-- Pattern signals (historical analysis)
CREATE TABLE IF NOT EXISTS pattern_signals (
    id BIGINT PRIMARY KEY,
    symbol VARCHAR NOT NULL,
    pattern_name VARCHAR NOT NULL,
    signal_type VARCHAR NOT NULL,
    confidence DECIMAL(5,4),
    detected_at TIMESTAMP NOT NULL,
    candle_timestamp TIMESTAMP NOT NULL,
    price_at_detection DECIMAL(18,4) NOT NULL,
    acted_upon BOOLEAN DEFAULT FALSE,
    outcome VARCHAR
);

CREATE INDEX idx_signals_pattern ON pattern_signals(pattern_name, detected_at DESC);

-- Pattern statistics (aggregations)
CREATE TABLE IF NOT EXISTS pattern_stats (
    pattern_name VARCHAR PRIMARY KEY,
    total_signals INTEGER DEFAULT 0,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit DECIMAL(18,4) DEFAULT 0.0,
    win_rate DECIMAL(5,4) DEFAULT 0.0,
    avg_profit_per_trade DECIMAL(18,4) DEFAULT 0.0,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Backtest results (complex queries)
CREATE TABLE IF NOT EXISTS backtest_runs (
    id BIGINT PRIMARY KEY,
    name VARCHAR NOT NULL,
    strategy_config TEXT NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    initial_capital DECIMAL(18,4) NOT NULL,
    final_value DECIMAL(18,4) NOT NULL,
    total_return DECIMAL(10,4) NOT NULL,
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    win_rate DECIMAL(5,4) NOT NULL,
    max_drawdown DECIMAL(10,4) NOT NULL,
    sharpe_ratio DECIMAL(10,4),
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Backtest trades (large datasets)
CREATE TABLE IF NOT EXISTS backtest_trades (
    id BIGINT PRIMARY KEY,
    backtest_run_id BIGINT NOT NULL,
    symbol VARCHAR NOT NULL,
    side VARCHAR NOT NULL,
    quantity INTEGER NOT NULL,
    entry_price DECIMAL(18,4) NOT NULL,
    exit_price DECIMAL(18,4),
    entry_time TIMESTAMP NOT NULL,
    exit_time TIMESTAMP,
    profit_loss DECIMAL(18,4),
    pattern_name VARCHAR,
    FOREIGN KEY (backtest_run_id) REFERENCES backtest_runs(id)
);

CREATE INDEX idx_backtest_trades_run ON backtest_trades(backtest_run_id);
```

### Data Synchronization

**Strategy:** Keep databases separate, sync only when needed

```rust
// src/db/sync.rs

pub struct DatabaseSync {
    sqlite: SqliteDb,
    duckdb: DuckDbAnalytics,
}

impl DatabaseSync {
    /// Sync completed trades to DuckDB for analysis
    pub async fn sync_trades_to_analytics(&self) -> Result<()> {
        // Query recent trades from SQLite
        let trades = self.sqlite.get_recent_trades()?;
        
        // Bulk insert into DuckDB
        self.duckdb.bulk_insert_trades(trades)?;
        
        Ok(())
    }
    
    /// Sync pattern signals for historical analysis
    pub async fn sync_pattern_signals(&self) -> Result<()> {
        // Similar approach
        Ok(())
    }
}
```

**Sync Schedule:**
- Real-time: Orders, positions, trades → SQLite only
- Hourly: Sync completed trades to DuckDB
- Daily: Sync pattern signals and stats
- On-demand: Backtest data

---

## Serialization: Protocol Buffers vs Cap'n Proto vs JSON

### Analysis

#### Current Approach: JSON (serde_json)

**Pros:**
- ✅ Human-readable (easy debugging)
- ✅ Excellent Rust support (serde)
- ✅ Flexible schema evolution
- ✅ Wide ecosystem compatibility
- ✅ No code generation needed

**Cons:**
- ❌ Slower serialization/deserialization
- ❌ Larger message size
- ❌ No schema validation
- ❌ Higher CPU usage

**Performance:**
- Serialization: ~500-1000 ns per message
- Size: ~200-500 bytes per candle

---

#### Protocol Buffers (protobuf)

**Pros:**
- ✅ Fast serialization (~100-200 ns)
- ✅ Compact binary format (50-70% smaller than JSON)
- ✅ Strong schema validation
- ✅ Good Rust support (prost crate)
- ✅ Backward/forward compatibility
- ✅ Wide industry adoption

**Cons:**
- ❌ Requires code generation
- ❌ Not human-readable
- ❌ More complex setup
- ❌ Schema changes require recompilation

**Use Cases:**
- Internal service communication
- High-frequency data streams
- Large message volumes

**Rust Implementation:**
```rust
// Cargo.toml
[dependencies]
prost = "0.12"
prost-types = "0.12"

[build-dependencies]
prost-build = "0.12"

// build.rs
fn main() {
    prost_build::compile_protos(&["src/proto/market_data.proto"], &["src/proto/"]).unwrap();
}

// src/proto/market_data.proto
syntax = "proto3";

package trading;

message Candle {
    string symbol = 1;
    int64 timestamp = 2;  // Unix timestamp
    double open = 3;
    double high = 4;
    double low = 5;
    double close = 6;
    int64 volume = 7;
}

message Quote {
    string symbol = 1;
    double bid = 2;
    double ask = 3;
    double last = 4;
    int64 timestamp = 5;
}

// Usage
use prost::Message;

let candle = Candle {
    symbol: "AAPL".to_string(),
    timestamp: 1234567890,
    open: 150.0,
    high: 151.0,
    low: 149.5,
    close: 150.5,
    volume: 1000000,
};

// Serialize
let bytes = candle.encode_to_vec();

// Deserialize
let decoded = Candle::decode(&bytes[..])?;
```

**Performance Benchmark:**
- Serialization: ~100-200 ns (5-10x faster than JSON)
- Deserialization: ~150-300 ns (3-5x faster than JSON)
- Size: ~50-100 bytes per candle (50-70% smaller than JSON)

---

#### Cap'n Proto

**Pros:**
- ✅ **Zero-copy deserialization** (fastest possible)
- ✅ Extremely fast (~10-50 ns)
- ✅ Compact binary format
- ✅ Schema evolution support
- ✅ No parsing step required

**Cons:**
- ❌ Less mature Rust support (capnp crate)
- ❌ Smaller ecosystem
- ❌ More complex API
- ❌ Requires code generation
- ❌ Not human-readable

**Use Cases:**
- Ultra-low latency requirements
- Memory-mapped data structures
- High-frequency trading systems

**Rust Implementation:**
```rust
// Cargo.toml
[dependencies]
capnp = "0.18"

[build-dependencies]
capnpc = "0.18"

// build.rs
fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("src/schema")
        .file("src/schema/market_data.capnp")
        .run()
        .expect("schema compiler command");
}

// src/schema/market_data.capnp
@0x85150b117366d14b;

struct Candle {
    symbol @0 :Text;
    timestamp @1 :Int64;
    open @2 :Float64;
    high @3 :Float64;
    low @4 :Float64;
    close @5 :Float64;
    volume @6 :Int64;
}

// Usage
use capnp::message::{Builder, ReaderOptions};

let mut message = Builder::new_default();
let mut candle = message.init_root::<candle::Builder>();
candle.set_symbol("AAPL");
candle.set_timestamp(1234567890);
candle.set_open(150.0);
// ... set other fields

// Serialize (zero-copy)
let bytes = capnp::serialize::write_message_to_words(&message);

// Deserialize (zero-copy)
let reader = capnp::serialize::read_message(&bytes[..], ReaderOptions::new())?;
let candle = reader.get_root::<candle::Reader>()?;
```

**Performance Benchmark:**
- Serialization: ~50-100 ns (10-20x faster than JSON)
- Deserialization: ~10-50 ns (20-50x faster than JSON, **zero-copy**)
- Size: ~50-100 bytes per candle (similar to protobuf)

---

### Recommendation: Hybrid Approach

**Use different serialization for different use cases:**

```
┌─────────────────────────────────────────────────────┐
│         Serialization Strategy                       │
├─────────────────────────────────────────────────────┤
│                                                      │
│  JSON (serde_json)                                  │
│  ├── Configuration files (human-readable)           │
│  ├── API responses to frontend (compatibility)      │
│  ├── Logging and debugging (readability)            │
│  └── External API communication (Alpaca uses JSON)  │
│                                                      │
│  Protocol Buffers (prost)                           │
│  ├── Internal event bus messages                    │
│  ├── Database storage (compact)                     │
│  ├── Inter-process communication                    │
│  └── Cache serialization                            │
│                                                      │
│  Cap'n Proto (optional, future optimization)        │
│  ├── Ultra-low latency paths (if needed)            │
│  ├── Memory-mapped data structures                  │
│  └── High-frequency data streams                    │
│                                                      │
└─────────────────────────────────────────────────────┘
```

### Implementation Strategy

**Phase 1 (MVP): JSON Only**
- Use JSON for everything
- Simple, fast development
- Easy debugging
- Sufficient for 1-minute candles

**Phase 2 (Optimization): Add Protocol Buffers**
- Migrate internal event bus to protobuf
- Use protobuf for database storage
- Keep JSON for external APIs
- Measure performance improvement

**Phase 3 (Advanced): Cap'n Proto (If Needed)**
- Only if latency becomes an issue
- Use for critical hot paths
- Requires careful profiling

---

## Performance Analysis

### Latency Budget Breakdown

**Target: 100ms-1s for order execution**

```
Component                    | JSON    | Protobuf | Cap'n Proto
-----------------------------|---------|----------|-------------
WebSocket receive            | 10ms    | 10ms     | 10ms
Deserialize candle           | 1ms     | 0.2ms    | 0.05ms
Pattern detection            | 20ms    | 20ms     | 20ms
Risk validation              | 10ms    | 10ms     | 10ms
Serialize order              | 1ms     | 0.2ms    | 0.05ms
Broker API call              | 50ms    | 50ms     | 50ms
-----------------------------|---------|----------|-------------
TOTAL                        | 92ms    | 90.4ms   | 90.1ms
```

**Conclusion:** JSON is fast enough for 1-minute candles. Protobuf provides marginal improvement (~2ms). Cap'n Proto provides minimal additional benefit (~0.3ms).

**Recommendation:** Start with JSON, optimize later if needed.

---

### When to Use Each Serialization Format

**Use JSON when:**
- ✅ Human readability is important
- ✅ Debugging and development
- ✅ External API communication
- ✅ Configuration files
- ✅ Latency is not critical (<1s acceptable)

**Use Protocol Buffers when:**
- ✅ Internal service communication
- ✅ High message volume (>1000/sec)
- ✅ Storage efficiency matters
- ✅ Schema validation needed
- ✅ Latency is important (<100ms target)

**Use Cap'n Proto when:**
- ✅ Ultra-low latency required (<10ms)
- ✅ Zero-copy deserialization needed
- ✅ Memory-mapped data structures
- ✅ High-frequency trading (sub-millisecond)

---

## Alpaca API Communication

### Current Alpaca API

**Alpaca uses JSON for both REST and WebSocket:**
- REST API: JSON over HTTPS
- WebSocket: JSON messages

**You cannot change Alpaca's serialization format.**

**Options:**
1. **Keep JSON for Alpaca communication** (recommended)
   - No choice for external API
   - Use protobuf internally only

2. **Convert at the boundary**
   - Receive JSON from Alpaca
   - Convert to protobuf for internal use
   - Convert back to JSON for responses

**Recommendation:** Keep JSON for Alpaca, use protobuf internally if needed.

---

## Implementation Roadmap

### Phase 1: MVP (Weeks 1-20)

**Database:**
- ✅ SQLite for all data (simple, proven)
- ✅ Single database file
- ✅ WAL mode for concurrency

**Serialization:**
- ✅ JSON for everything (serde_json)
- ✅ Simple, fast development
- ✅ Easy debugging

**Rationale:** Prove the system works first, optimize later.

---

### Phase 2: Optimization (Post-MVP)

**Database:**
- ✅ Add DuckDB for analytics
- ✅ Migrate historical candles to DuckDB
- ✅ Keep SQLite for transactional data
- ✅ Implement sync mechanism

**Serialization:**
- ✅ Add Protocol Buffers for internal events
- ✅ Keep JSON for external APIs
- ✅ Benchmark performance improvement

**Rationale:** Optimize based on real-world usage patterns.

---

### Phase 3: Advanced (If Needed)

**Database:**
- ✅ Optimize DuckDB queries
- ✅ Add Parquet export for long-term storage
- ✅ Implement data retention policies

**Serialization:**
- ✅ Evaluate Cap'n Proto for hot paths
- ✅ Profile and optimize critical sections
- ✅ Measure actual performance gains

**Rationale:** Only optimize if profiling shows bottlenecks.

---

## Updated Cargo.toml

```toml
[dependencies]
# Database
rusqlite = { version = "0.30", features = ["bundled", "chrono"] }
duckdb = { version = "0.9", optional = true }  # Add in Phase 2

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
prost = { version = "0.12", optional = true }  # Add in Phase 2
prost-types = { version = "0.12", optional = true }

# Cap'n Proto (optional, Phase 3)
capnp = { version = "0.18", optional = true }

[build-dependencies]
prost-build = { version = "0.12", optional = true }
capnpc = { version = "0.18", optional = true }

[features]
default = []
analytics = ["duckdb"]  # Enable DuckDB for analytics
protobuf = ["prost", "prost-types", "prost-build"]  # Enable protobuf
capnproto = ["capnp", "capnpc"]  # Enable Cap'n Proto
```

---

## Benchmarking Plan

### Performance Testing

**Create benchmarks to measure actual performance:**

```rust
// benches/serialization.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_json_serialization(c: &mut Criterion) {
    let candle = Candle {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        open: Decimal::new(15000, 2),
        high: Decimal::new(15100, 2),
        low: Decimal::new(14950, 2),
        close: Decimal::new(15050, 2),
        volume: 1000000,
    };
    
    c.bench_function("json_serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&candle).unwrap();
            black_box(json);
        });
    });
}

fn benchmark_protobuf_serialization(c: &mut Criterion) {
    // Similar benchmark for protobuf
}

criterion_group!(benches, benchmark_json_serialization, benchmark_protobuf_serialization);
criterion_main!(benches);
```

**Run benchmarks:**
```bash
cargo bench
```

**Decision criteria:**
- If JSON is <1ms per operation → Keep JSON
- If protobuf is >5x faster → Consider migration
- If Cap'n Proto is >10x faster → Consider for hot paths

---

## Conclusion

### Recommended Approach

**Database:**
- **Phase 1 (MVP):** SQLite only
- **Phase 2 (Optimization):** SQLite + DuckDB hybrid
- **Rationale:** Proven reliability first, optimize later

**Serialization:**
- **Phase 1 (MVP):** JSON only
- **Phase 2 (Optimization):** JSON + Protocol Buffers
- **Phase 3 (Advanced):** Add Cap'n Proto if needed
- **Rationale:** Simple first, optimize based on profiling

**Alpaca API:**
- **Always JSON** (no choice)
- Convert at boundary if using protobuf internally

### Key Takeaways

1. **Start simple** - JSON + SQLite is sufficient for MVP
2. **Measure first** - Profile before optimizing
3. **Optimize strategically** - Focus on actual bottlenecks
4. **Hybrid approach** - Use the right tool for each job
5. **Don't over-engineer** - 1-minute candles don't need sub-millisecond latency

The recommended hybrid database approach and phased serialization strategy provide a clear path from MVP to optimized production system.

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Next Review:** After Phase 1 completion
