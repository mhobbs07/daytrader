# Day Trading Platform - Architecture Analysis & Recommendations

**Date:** December 16, 2025  
**Status:** Pre-Development Analysis  
**Reviewer:** Architecture Review

---

## Executive Summary

This document provides a comprehensive analysis of the day trading platform architecture, identifying strengths, risks, and recommendations for successful implementation.

## 🎯 Overall Assessment

**Rating: Excellent Foundation (8.5/10)**

The project demonstrates exceptional planning with strong focus on:
- Risk management and regulatory compliance
- Architectural flexibility and future-proofing
- Realistic timeline and cost projections
- Technology stack alignment with requirements

---

## Critical Issues Requiring Immediate Attention

### 1. Cargo.toml Configuration Error

**Issue:** Invalid Rust edition specified
```toml
edition = "2024"  # ❌ Does not exist
```

**Fix Required:**
```toml
edition = "2021"  # ✅ Latest stable edition
```

**Impact:** Project won't compile without this fix.

---

### 2. Python Integration Strategy Ambiguity

**Current State:** Architecture document mentions both PyO3 and subprocess approaches without clear decision.

**Problem:** 
- PyO3 embedded: Complex build, tighter coupling, faster
- Subprocess: Simpler, isolated, adds latency (50-200ms per call)

**Recommendation:**

```
Phase 1 (MVP): Subprocess Bridge
├── Pros: Faster development, easier debugging
├── Cons: Latency for real-time indicators
└── Use for: Backtesting, advanced analysis only

Phase 2 (Optimization): Hybrid Approach
├── Rust: Core indicators (SMA, EMA, RSI, MACD) using 'ta' crate
├── Python: Advanced analysis, ML, LLM integration
└── Result: <10ms indicator calculation vs 50-200ms
```

**Action Items:**
- [ ] Move basic technical indicators to Rust using `ta` crate
- [ ] Reserve Python for backtesting and advanced analytics
- [ ] Document latency budget for each component

---

### 3. Database Architecture Concerns

**Current Plan:** DuckDB for all data

**Concerns:**
1. DuckDB optimized for OLAP (analytics), not OLTP (transactions)
2. Concurrent write performance for real-time orders
3. Less mature than SQLite for production use
4. Smaller ecosystem and community

**Recommended Hybrid Approach:**

```
┌─────────────────────────────────────────────────┐
│           Data Storage Strategy                  │
├─────────────────────────────────────────────────┤
│                                                  │
│  SQLite (Transactional Data)                    │
│  ├── Orders (real-time writes)                  │
│  ├── Positions (frequent updates)               │
│  ├── Trades (immediate recording)               │
│  ├── Risk metrics (real-time tracking)          │
│  └── Account state (concurrent access)          │
│                                                  │
│  DuckDB (Analytical Data)                       │
│  ├── Historical candles (bulk queries)          │
│  ├── Backtest results (aggregations)            │
│  ├── Pattern statistics (analytics)             │
│  └── Performance reports (complex queries)      │
│                                                  │
└─────────────────────────────────────────────────┘
```

**Benefits:**
- SQLite: Battle-tested, excellent for concurrent reads/writes
- DuckDB: 10-100x faster for analytical queries
- Clear separation of concerns
- Optimal performance for each use case

---

### 4. Real-Time Event Architecture Missing

**Gap:** No clear design for how components communicate in real-time.

**Problem Scenarios:**
- New candle arrives → How do pattern detector, UI, and database all get notified?
- Pattern detected → How does risk manager and execution engine receive signal?
- Order filled → How do position tracker and UI update?

**Recommended Event Bus Architecture:**

```rust
// Event-driven architecture using tokio channels

┌──────────────────────────────────────────────────────┐
│              WebSocket Data Stream                    │
└────────────────────┬─────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────┐
│         Central Event Bus (broadcast channel)         │
│                                                       │
│  Events:                                             │
│  - NewCandle(symbol, candle)                         │
│  - PatternDetected(signal)                           │
│  - OrderPlaced(order)                                │
│  - OrderFilled(fill)                                 │
│  - RiskViolation(violation)                          │
│  - PositionUpdated(position)                         │
└─┬────────┬────────┬────────┬────────┬───────────────┘
  │        │        │        │        │
  ▼        ▼        ▼        ▼        ▼
┌───┐   ┌────┐   ┌────┐   ┌────┐   ┌──┐
│Pat│   │Risk│   │Exec│   │ DB │   │UI│
│Det│   │Mgr │   │Eng │   │Wrt │   │  │
└───┘   └────┘   └────┘   └────┘   └──┘
```

**Implementation:**
```rust
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub enum MarketEvent {
    NewCandle { symbol: String, candle: Candle },
    PatternDetected { signal: PatternSignal },
    OrderPlaced { order: Order },
    OrderFilled { fill: Fill },
    RiskViolation { violation: RiskViolation },
    PositionUpdated { position: Position },
}

pub struct EventBus {
    sender: broadcast::Sender<MarketEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    pub fn publish(&self, event: MarketEvent) {
        let _ = self.sender.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<MarketEvent> {
        self.sender.subscribe()
    }
}
```

---

## Architecture Enhancements

### 5. Pattern Detection Performance Optimization

**Concern:** Detecting patterns on every 1-minute candle for 50+ symbols = potential bottleneck

**Current Approach:**
```
New Candle → Detect All Patterns → Generate Signals
(Synchronous, blocks on each symbol)
```

**Optimized Approach:**
```
New Candle → Queue → Worker Pool → Pattern Detection → Event Bus
(Async, parallel processing, prioritized)
```

**Implementation Strategy:**

```rust
use tokio::sync::mpsc;
use dashmap::DashMap;

pub struct PatternDetectionEngine {
    work_queue: mpsc::Sender<DetectionTask>,
    candle_cache: DashMap<String, Vec<Candle>>, // Last 100 candles per symbol
    worker_count: usize,
}

struct DetectionTask {
    symbol: String,
    priority: Priority,
}

enum Priority {
    High,    // Watchlist symbols
    Medium,  // Open positions
    Low,     // Other symbols
}

impl PatternDetectionEngine {
    pub async fn process_candle(&self, symbol: String, candle: Candle) {
        // Update cache
        self.candle_cache
            .entry(symbol.clone())
            .or_insert_with(Vec::new)
            .push(candle);
        
        // Queue for detection
        let priority = self.get_priority(&symbol);
        let task = DetectionTask { symbol, priority };
        let _ = self.work_queue.send(task).await;
    }
    
    async fn worker(&self, mut rx: mpsc::Receiver<DetectionTask>) {
        while let Some(task) = rx.recv().await {
            if let Some(candles) = self.candle_cache.get(&task.symbol) {
                let patterns = self.detect_patterns(&candles);
                // Publish to event bus
            }
        }
    }
}
```

**Benefits:**
- Non-blocking pattern detection
- Prioritized processing (watchlist first)
- Scalable to 100+ symbols
- Target: <50ms per symbol

---

### 6. Connection Resilience Strategy

**Missing:** Detailed WebSocket reconnection and state recovery logic

**Critical Scenarios:**
1. WebSocket disconnection during market hours
2. Missed candles during reconnection
3. Order state uncertainty after connection loss
4. Position drift due to missed fills

**Recommended Resilience Architecture:**

```rust
pub struct ResilientWebSocket {
    connection: Option<WebSocketStream>,
    reconnect_attempts: u32,
    max_reconnects: u32,
    backoff_strategy: ExponentialBackoff,
    state_recovery: StateRecovery,
}

impl ResilientWebSocket {
    pub async fn connect_with_retry(&mut self) -> Result<()> {
        let mut delay = Duration::from_secs(1);
        
        for attempt in 1..=self.max_reconnects {
            match self.connect().await {
                Ok(ws) => {
                    self.connection = Some(ws);
                    self.recover_state().await?;
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Connection attempt {} failed: {}", attempt, e);
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(60));
                }
            }
        }
        
        Err(anyhow!("Failed to connect after {} attempts", self.max_reconnects))
    }
    
    async fn recover_state(&self) -> Result<()> {
        // 1. Fetch missed candles from REST API
        // 2. Reconcile order states
        // 3. Verify position accuracy
        // 4. Resume normal operation
        Ok(())
    }
}

pub struct StateRecovery {
    last_candle_time: DashMap<String, DateTime<Utc>>,
    pending_orders: DashMap<String, Order>,
}

impl StateRecovery {
    pub async fn recover_missing_data(&self, broker: &dyn MarketDataProvider) -> Result<()> {
        for entry in self.last_candle_time.iter() {
            let symbol = entry.key();
            let last_time = *entry.value();
            let now = Utc::now();
            
            if now - last_time > Duration::minutes(2) {
                // Fetch missing candles
                let candles = broker.get_candles(
                    symbol,
                    last_time,
                    now,
                    Timeframe::OneMinute
                ).await?;
                
                // Process missed candles
                for candle in candles {
                    // Publish to event bus
                }
            }
        }
        Ok(())
    }
}
```

---

### 7. Enhanced Risk Management

**Addition:** Audit trail and emergency controls

**New Components:**

```rust
pub struct RiskAuditLog {
    db: Database,
}

impl RiskAuditLog {
    pub async fn log_violation(&self, violation: RiskViolation) {
        // Record all risk violations with full context
        let entry = AuditEntry {
            timestamp: Utc::now(),
            violation_type: violation.to_string(),
            account_state: self.capture_account_state().await,
            order_details: violation.order_details(),
            action_taken: "Order rejected".to_string(),
        };
        
        self.db.insert_audit_entry(entry).await;
    }
    
    pub async fn log_override(&self, user: &str, reason: &str, violation: RiskViolation) {
        // Log manual overrides for compliance
        let entry = AuditEntry {
            timestamp: Utc::now(),
            violation_type: format!("OVERRIDE: {}", violation),
            user: user.to_string(),
            reason: reason.to_string(),
            action_taken: "Manual override approved".to_string(),
        };
        
        self.db.insert_audit_entry(entry).await;
    }
}

pub struct EmergencyControls {
    is_halted: AtomicBool,
    halt_reason: RwLock<Option<String>>,
}

impl EmergencyControls {
    pub fn emergency_stop(&self, reason: String) {
        self.is_halted.store(true, Ordering::SeqCst);
        *self.halt_reason.write().unwrap() = Some(reason);
        
        // Cancel all pending orders
        // Close all positions (optional)
        // Notify user immediately
        
        tracing::error!("EMERGENCY STOP ACTIVATED: {}", reason);
    }
    
    pub fn is_trading_allowed(&self) -> Result<(), String> {
        if self.is_halted.load(Ordering::SeqCst) {
            let reason = self.halt_reason.read().unwrap();
            return Err(format!("Trading halted: {}", reason.as_ref().unwrap()));
        }
        Ok(())
    }
}
```

---

## Testing Strategy Enhancements

### Missing Test Categories

**1. Chaos Testing**
```rust
#[tokio::test]
async fn test_websocket_disconnection_during_trading() {
    // Simulate connection loss during active trading
    // Verify: No orders lost, positions reconciled, state recovered
}

#[tokio::test]
async fn test_broker_api_timeout() {
    // Simulate slow/timeout broker responses
    // Verify: Proper error handling, retry logic, user notification
}
```

**2. Time-Based Testing**
```rust
#[tokio::test]
async fn test_market_open_transition() {
    // Test behavior at 9:30 AM ET market open
    // Verify: Proper initialization, no race conditions
}

#[tokio::test]
async fn test_pdt_rule_across_5_day_window() {
    // Test PDT tracking across rolling 5-day period
    // Verify: Accurate counting, proper warnings, enforcement
}
```

**3. Regulatory Compliance Testing**
```rust
#[tokio::test]
async fn test_pdt_enforcement_scenarios() {
    // Scenario 1: Account < $25k, 3 day trades → 4th blocked
    // Scenario 2: Account crosses $25k threshold → restrictions lifted
    // Scenario 3: Day trade on Friday, Monday → proper counting
}
```

**4. Performance Benchmarks**
```rust
#[tokio::test]
async fn benchmark_pattern_detection_latency() {
    // Target: <100ms per symbol
    // Measure: 50 symbols simultaneously
}

#[tokio::test]
async fn benchmark_order_execution_latency() {
    // Target: 100ms-1s end-to-end
    // Measure: Signal → Risk Check → Order Submission
}
```

---

## Revised MVP Scope

### Core MVP Features (Must-Have)

**Data Pipeline:**
- ✅ Real-time 1-minute candle ingestion via WebSocket
- ✅ Connection resilience with auto-reconnect
- ✅ Historical data backfill (last 100 candles per symbol)
- ✅ Data validation (OHLC relationships)

**Pattern Recognition:**
- ✅ 5 core patterns: Hammer, Shooting Star, Doji, Bullish/Bearish Engulfing
- ✅ 4 basic indicators: SMA(20,50), EMA(12,26), RSI(14), MACD
- ✅ Pattern confidence scoring
- ✅ Signal generation with indicator confirmation

**Risk Management (All Rules):**
- ✅ Daily loss limits (dollar and percent)
- ✅ Position size limits
- ✅ PDT rule enforcement
- ✅ Pattern-specific limits
- ✅ Account balance safeguards
- ✅ Emergency stop mechanism

**Order Execution:**
- ✅ Manual order placement (market and limit orders)
- ✅ Order status tracking
- ✅ Position tracking with real-time P&L
- ✅ Pre-trade risk validation
- ✅ Order history

**User Interface:**
- ✅ Single-window layout
- ✅ Real-time candlestick chart (TradingView Lightweight Charts)
- ✅ Pattern alerts display
- ✅ Risk metrics dashboard
- ✅ Order entry form
- ✅ Position summary
- ✅ Watchlist (max 20 symbols)

**Paper Trading:**
- ✅ Alpaca paper trading integration
- ✅ Full feature parity with live trading
- ✅ Performance tracking

### Deferred to Post-MVP

**Automated Execution:**
- ❌ Auto-execution based on patterns (too risky for MVP)
- ❌ Win rate tracking for auto-enable
- **Rationale:** Requires extensive validation; manual mode proves system first

**Advanced Features:**
- ❌ Backtesting engine (complex, time-consuming)
- ❌ Trade journal with notes
- ❌ Multi-window UI
- ❌ Advanced order types (trailing stops, bracket orders)
- ❌ LLM integration
- ❌ Options trading

**Rationale:** Focus on core trading loop first, add enhancements after MVP validation

### Added to MVP (Critical Additions)

**Operational Safety:**
- ✅ Connection health monitoring dashboard
- ✅ Manual position reconciliation tool
- ✅ Emergency stop button (prominent UI placement)
- ✅ Risk violation history viewer
- ✅ Audit log for all trading actions

**Data Quality:**
- ✅ Stale data detection (alert if no update >2 minutes)
- ✅ Candle validation (reject invalid OHLC)
- ✅ Gap detection in time-series

---

## Technology Stack Refinements

### Updated Rust Dependencies

```toml
[package]
name = "daytrader"
version = "0.1.0"
edition = "2021"  # ✅ Fixed

[dependencies]
# Core Framework
tauri = { version = "1.5", features = ["api-all"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Networking
reqwest = { version = "0.11", features = ["json"] }
tokio-tungstenite = "0.21"

# Database
rusqlite = { version = "0.30", features = ["bundled"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }

# Data Structures
rust_decimal = { version = "1.33", features = ["serde"] }
chrono = { version = "0.4", features = ["serde"] }
dashmap = "5.5"  # Concurrent HashMap for caching

# Technical Analysis
ta = "0.5"  # ✅ NEW: Technical indicators in Rust

# Concurrency
crossbeam = "0.8"  # ✅ NEW: Better channels and sync primitives

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Configuration
config = "0.13"

# Async Utilities
async-trait = "0.1"
futures = "0.3"

# Rate Limiting
tower = { version = "0.4", features = ["limit"] }  # ✅ NEW

# Security
keyring = "2.0"  # OS keychain integration

[dev-dependencies]
mockall = "0.12"  # Mocking for tests
criterion = "0.5"  # Benchmarking
```

### Python Dependencies (Minimal for MVP)

```python
# requirements.txt
pandas==2.1.0
numpy==1.26.0
pandas-ta==0.3.14b  # Technical indicators (backup/validation)

# Deferred to Post-MVP:
# backtrader==1.9.78.123
# scikit-learn==1.3.0
# langchain==0.0.300
```

---

## Risk Assessment

### High-Risk Items

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| WebSocket disconnection during trading | High | Medium | Resilient reconnection + state recovery |
| Order execution failure | High | Low | Retry logic + timeout handling + alerts |
| Pattern overfitting (false signals) | High | High | 30+ days paper trading validation required |
| PDT rule violation | Critical | Low | Extensive testing + audit trail |
| Data quality issues | Medium | Medium | Validation + anomaly detection |
| Performance degradation | Medium | Medium | Benchmarks + profiling + optimization |

### Mitigation Strategies

**1. WebSocket Reliability**
```rust
// Implement heartbeat monitoring
pub struct WebSocketMonitor {
    last_message: Arc<RwLock<Instant>>,
    timeout: Duration,
}

impl WebSocketMonitor {
    pub async fn monitor(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            let last = *self.last_message.read().unwrap();
            if last.elapsed() > self.timeout {
                tracing::error!("WebSocket timeout detected");
                // Trigger reconnection
            }
        }
    }
}
```

**2. Order Execution Safety**
```rust
pub struct OrderExecutor {
    retry_policy: RetryPolicy,
    timeout: Duration,
}

impl OrderExecutor {
    pub async fn place_order_with_retry(&self, order: Order) -> Result<String> {
        let mut attempts = 0;
        let max_attempts = 3;
        
        while attempts < max_attempts {
            match tokio::time::timeout(
                self.timeout,
                self.broker.place_order(order.clone())
            ).await {
                Ok(Ok(order_id)) => return Ok(order_id),
                Ok(Err(e)) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }
                }
                Err(_) => {
                    tracing::warn!("Order placement timeout, attempt {}", attempts + 1);
                }
            }
            
            attempts += 1;
            tokio::time::sleep(Duration::from_millis(500 * attempts)).await;
        }
        
        Err(anyhow!("Order placement failed after {} attempts", max_attempts))
    }
}
```

**3. Pattern Validation**
- Require minimum 30 days paper trading
- Track pattern accuracy metrics
- Disable patterns with <50% win rate
- Manual review before enabling auto-execution

---

## Development Phases (Revised)

### Phase 0: Pre-Development (Week 0)

**Setup & Infrastructure:**
- [ ] Fix Cargo.toml edition
- [ ] Set up project structure
- [ ] Configure development environment
- [ ] Set up CI/CD pipeline (GitHub Actions)
- [ ] Create mock broker for testing
- [ ] Initialize SQLite database schema
- [ ] Set up logging infrastructure

**Deliverable:** Compilable project with test framework

---

### Phase 1: Core Infrastructure (Weeks 1-4)

**Week 1-2: Data Pipeline**
- [ ] Implement broker abstraction traits
- [ ] Create Alpaca adapter (REST + WebSocket)
- [ ] Build event bus architecture
- [ ] Implement candle ingestion and storage
- [ ] Add connection resilience
- [ ] Create data validation layer

**Week 3-4: Caching & Performance**
- [ ] Implement in-memory candle cache (DashMap)
- [ ] Add state recovery mechanism
- [ ] Build health monitoring system
- [ ] Create integration tests
- [ ] Performance benchmarks

**Deliverable:** Reliable real-time data pipeline with <100ms latency

---

### Phase 2: Risk Management (Weeks 5-8)

**Week 5-6: Core Risk Rules**
- [ ] Implement risk configuration system
- [ ] Daily loss tracking
- [ ] Position size validation
- [ ] PDT rule enforcement
- [ ] Account balance monitoring
- [ ] Risk violation alerts

**Week 7-8: Safety Features**
- [ ] Audit logging system
- [ ] Emergency stop mechanism
- [ ] Manual override workflow
- [ ] Risk violation history
- [ ] Comprehensive risk testing

**Deliverable:** Production-ready risk management system

---

### Phase 3: Pattern Recognition (Weeks 9-12)

**Week 9-10: Pattern Detection**
- [ ] Implement candlestick pattern detector
- [ ] Add 5 core patterns
- [ ] Pattern confidence scoring
- [ ] Pattern signal storage
- [ ] Event bus integration

**Week 11-12: Technical Indicators**
- [ ] Implement indicators in Rust (ta crate)
- [ ] Signal generation logic
- [ ] Pattern + indicator confirmation
- [ ] Alert system
- [ ] Performance optimization

**Deliverable:** Working pattern recognition with <50ms latency per symbol

---

### Phase 4: Order Execution (Weeks 13-16)

**Week 13-14: Order Management**
- [ ] Order placement (market, limit)
- [ ] Order cancellation
- [ ] Order status tracking
- [ ] Fill notifications
- [ ] Order history

**Week 15-16: Position Tracking**
- [ ] Real-time position updates
- [ ] P&L calculations
- [ ] Position reconciliation
- [ ] Trade recording
- [ ] Manual execution workflow

**Deliverable:** Complete manual trading system

---

### Phase 5: User Interface (Weeks 17-20)

**Week 17-18: Core UI**
- [ ] Real-time candlestick chart
- [ ] Order entry form
- [ ] Position dashboard
- [ ] Risk metrics display
- [ ] Pattern alerts panel

**Week 19-20: Polish & UX**
- [ ] Watchlist management
- [ ] Connection status indicator
- [ ] Emergency stop button
- [ ] Settings configuration
- [ ] UI responsiveness optimization

**Deliverable:** Complete desktop application

---

### Phase 6: Testing & Validation (Weeks 21-24)

**Week 21-22: Comprehensive Testing**
- [ ] Integration tests (full workflows)
- [ ] Chaos testing (disconnections, failures)
- [ ] Regulatory compliance tests (PDT scenarios)
- [ ] Performance benchmarks
- [ ] Security audit

**Week 23-24: Paper Trading Validation**
- [ ] 30-day paper trading period
- [ ] Pattern accuracy tracking
- [ ] Risk management validation
- [ ] Bug fixes and refinements
- [ ] Documentation

**Deliverable:** Production-ready system validated in paper trading

---

## Success Metrics

### Technical Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Order execution latency | <1s (target: 100-500ms) | End-to-end timing |
| Pattern detection latency | <100ms per symbol | Benchmark tests |
| WebSocket uptime | >99% during market hours | Health monitoring |
| Data freshness | <2s lag | Timestamp comparison |
| UI responsiveness | <200ms chart render | Performance profiling |

### Business Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Pattern win rate | >55% before auto-enable | Historical tracking |
| Daily loss limit violations | 0 | Audit log |
| PDT violations | 0 | Compliance tracking |
| Paper trading profitability | Break-even or positive | 30-day period |
| Order fill rate | >95% | Execution tracking |

### Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test coverage | >80% | cargo-tarpaulin |
| Critical bugs | 0 in risk management | Issue tracking |
| Security vulnerabilities | 0 high/critical | cargo audit |
| Code quality | Clippy warnings = 0 | CI/CD checks |

---

## Cost Analysis (Updated)

### Development Phase (Months 1-6)
- Infrastructure: $0 (local development)
- Alpaca API: $0 (paper trading)
- **Total: $0/month**

### Production Phase (Live Trading)
- Alpaca API: $0 (commission-free, data included)
- Cloud backup (optional): $10-20/month
- Monitoring (optional): $0-30/month
- **Total: $10-50/month**

### Scaling (10 users)
- Shared data feed: No additional cost
- Storage: +$5-10/month
- **Total: $15-60/month**

**Cost Optimization:**
- Use Alpaca's bundled IEX data (saves $200-400/month vs separate data provider)
- Local-first architecture (no server hosting)
- SQLite/DuckDB (no database hosting fees)

---

## Security Checklist

### API Key Management
- [ ] Store keys in OS keychain (keyring crate)
- [ ] Never log API keys
- [ ] Rotate keys periodically
- [ ] Separate paper/live keys

### Data Protection
- [ ] Encrypt database at rest (SQLCipher or disk encryption)
- [ ] Use HTTPS/WSS for all connections
- [ ] Validate all user inputs
- [ ] Sanitize logs (no sensitive data)

### Code Security
- [ ] Run `cargo audit` in CI/CD
- [ ] Enable all Clippy lints
- [ ] Use `#![forbid(unsafe_code)]` where possible
- [ ] Regular dependency updates

### Operational Security
- [ ] Rate limiting on order submissions
- [ ] Two-factor authentication for live trading
- [ ] Audit trail for all actions
- [ ] Emergency stop mechanism

---

## Next Steps & Recommendations

### Immediate Actions (This Week)

1. **Fix Cargo.toml**
   ```toml
   edition = "2021"
   ```

2. **Create Phase 0 Setup Plan**
   - Project structure
   - Development environment
   - CI/CD pipeline

3. **Build Proof-of-Concept**
   - Alpaca WebSocket connection
   - Event bus architecture
   - Basic pattern detection
   - Risk rule validation

4. **Validate Architecture**
   - Test latency assumptions
   - Verify broker API reliability
   - Confirm technology choices

### Decision Points

**Question 1: Database Strategy**
- Option A: SQLite only (simpler, proven)
- Option B: SQLite + DuckDB hybrid (optimal performance)
- **Recommendation:** Start with SQLite, add DuckDB in Phase 6 for backtesting

**Question 2: Python Integration**
- Option A: Subprocess bridge (simpler, MVP)
- Option B: PyO3 embedded (faster, complex)
- **Recommendation:** Subprocess for MVP, move indicators to Rust

**Question 3: MVP Scope**
- Option A: Full feature set (6 months)
- Option B: Reduced scope (4 months, manual trading only)
- **Recommendation:** Option B - prove core system first

### Success Criteria for MVP

**Before Live Trading:**
- [ ] 30+ days successful paper trading
- [ ] Zero risk management violations
- [ ] Pattern win rate >55%
- [ ] All tests passing
- [ ] Security audit complete
- [ ] Documentation complete

---

## Conclusion

This is an **exceptionally well-planned project** with strong fundamentals. The main recommendations are:

1. **Fix immediate issues** (Cargo.toml, clarify Python strategy)
2. **Add event bus architecture** for real-time communication
3. **Implement hybrid database approach** (SQLite + DuckDB)
4. **Enhance testing strategy** (chaos, compliance, performance)
5. **Add operational safety features** (emergency stop, audit trail)
6. **Reduce MVP scope** (defer automation to post-MVP)

With these adjustments, the project has an excellent chance of success within the 4-6 month timeline.

**Recommended Next Step:** Build a proof-of-concept to validate core assumptions before committing to full development.

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Next Review:** After Phase 0 completion
