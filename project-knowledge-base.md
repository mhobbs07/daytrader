# Day Trading Platform - Project Knowledge Base

**Project Name:** Day Trading Application  
**Owner:** Mike  
**Status:** Requirements Complete, Architecture Defined  
**Phase:** Pre-Development  
**Timeline:** 4-6 Month MVP Development  
**Last Updated:** November 15, 2025

---

## Project Overview

### Purpose
A comprehensive day trading platform designed as a bespoke solution for Mike and up to 10 people maximum. The system focuses on pattern recognition using candlestick analysis to generate buy/sell signals with both manual and automated execution capabilities.

### Core Value Proposition
- Pattern-based trading with automated signal generation
- Comprehensive risk management with regulatory compliance
- Cost-optimized approach using broker APIs
- Flexible architecture enabling future extensibility
- Desktop application with professional-grade features

---

## Requirements Summary

### Markets & Trading Scope

**Phase 1 (MVP): Stocks**
- US equities (NYSE, NASDAQ)
- Real-time 1-minute candlestick data
- Pattern-based day trading strategies
- Commission-free trading via Alpaca

**Phase 2 (Feature Flag): Options**
- Options chain data
- Greeks calculations (delta, gamma, theta, vega, rho)
- Options-specific order types
- Multi-leg strategies
- IV tracking

**Rationale:** Start with simpler stock trading, add options complexity later once foundation is proven

---

### Core Features (MVP)

**Essential Trading Features:**
1. **Real-time Stock Quotes** - 1-minute candles from Alpaca
2. **Order Execution** - Market, limit, stop-loss, trailing stops, bracket orders
3. **Position Tracking** - Real-time P&L, portfolio value, position management
4. **Basic Charting** - Candlestick charts with volume
5. **Technical Indicators** - SMA, EMA, RSI, MACD, Bollinger Bands, ATR
6. **Price Alerts** - Threshold-based and pattern-based notifications
7. **Watchlists** - Symbol tracking and organization
8. **Order History** - Complete trade log with filtering
9. **Trade Journal** - Notes, outcomes, pattern tracking
10. **Backtesting** - Historical pattern validation
11. **Advanced Order Types** - Stop-loss, trailing stops, bracket orders

**Pattern Recognition:**
- Candlestick pattern detection (Hammer, Doji, Shooting Star, Engulfing, Morning/Evening Star, Three White Soldiers, etc.)
- Technical indicator confirmation
- Signal generation (buy/sell prompts)
- Confidence scoring

**Automation Capabilities:**
- Manual execution mode (with confirmation)
- Automated execution mode (based on pattern + indicators)
- Configurable auto-execution rules
- Win rate thresholds before enabling auto-execution

---

### Scale & Performance Requirements

**User Base:**
- Primary: Mike (1 user)
- Maximum: 10 users total
- Bespoke solution, not a commercial platform

**Performance Targets:**
- **Order Execution Latency:** 100ms - 1 second
- **Pattern Detection:** < 100ms per symbol
- **Chart Rendering:** < 200ms
- **Data Update Frequency:** 1-minute candles (real-time via WebSocket)

**Rationale:** Pattern-based trading on 1-minute candles doesn't require HFT-level latency. 100ms-1s is fast enough to capture opportunities without expensive infrastructure.

---

### Technology Stack Decisions

**Platform: Tauri Desktop Application**
- Rust backend for performance-critical components
- Web frontend (React + TypeScript) for UI
- Small binary size (~3-5MB)
- Native performance using system webview
- Multi-monitor support for traders
- Cross-platform (Windows, macOS, Linux)

**Backend: Rust**
- Market data processing
- Order execution engine
- Risk management system
- Pattern detection core
- Broker abstraction layer

**Analysis Layer: Python**
- Technical indicator calculations
- Advanced pattern analysis
- LLM integration for data interpretation
- Backtesting engine

**Data Storage:**
- DuckDB or SQLite for local storage
- Time-series optimized for OHLCV data
- Fast analytical queries for backtesting

**Broker Integration:**
- **Primary:** Alpaca Markets
  - Free paper trading
  - Real-time market data included
  - Commission-free stock trading
  - Good API documentation
  - WebSocket support for real-time data
- **Future:** Abstraction layer enables switching to Interactive Brokers, TD Ameritrade, or others

**Rationale:** Tauri provides native performance with small footprint. Rust handles high-frequency data processing efficiently. Python enables rapid development of analytical features and LLM integration. Alpaca minimizes costs by bundling data + execution.

---

### Regulatory & Compliance

**Geography:** United States

**Key Regulatory Requirements:**

**Pattern Day Trader (PDT) Rule:**
- Accounts < $25,000: Maximum 3 day trades in rolling 5-day period
- Day trade = Buy and sell (or sell and buy) same security on same day
- Violation results in account restrictions
- **System Must:** Track day trades, warn before 3rd trade, block 4th trade

**SEC Compliance:**
- Real-time reporting requirements
- Trade logging and audit trails
- Position limits awareness

**Data Privacy:**
- GDPR not applicable (US-only)
- Secure storage of user credentials
- API key encryption

---

### Risk Management Requirements

**CRITICAL:** These are hard requirements that MUST be enforced by the system.

**1. Daily Loss Limits**
- **Max Daily Loss (Dollars):** Configurable (e.g., $500)
- **Max Daily Loss (Percent):** Configurable (e.g., 2% of portfolio)
- **Action on Breach:** Halt all trading immediately
- **Alert Threshold:** Warn at 80% of limit

**2. Position Size Limits**
- **Max Position Size (Dollars):** Configurable (e.g., $5,000)
- **Max Position Size (Percent):** Configurable (e.g., 10% of portfolio)
- **Max Concurrent Positions:** Configurable (e.g., 5)
- **Action on Breach:** Reject order

**3. PDT Protection**
- **Enforce PDT Rule:** Yes
- **Track Day Trades:** Automatic across rolling 5-day window
- **Action at 3 Trades (if <$25k):** Warn user, block 4th trade
- **Override:** Require manual confirmation to continue

**4. Pattern-Specific Trading Rules**
- **Max Trades Per Pattern Per Day:** Configurable (e.g., 5)
- **Stop After Consecutive Losses:** Configurable (e.g., 3)
- **Action:** Disable pattern for remainder of day

**5. Required Win Rate for Auto-Execution**
- **Minimum Sample Size:** Configurable (e.g., 20 trades)
- **Required Win Rate:** Configurable (e.g., 60%)
- **Action:** Disable auto-execution if below threshold
- **Manual Mode:** Always available regardless of win rate

**6. Account Balance Safeguards**
- **Minimum Account Balance:** Configurable (e.g., $1,000)
- **Action Below Minimum:** Halt trading, send alert
- **Emergency Liquidate Threshold:** Configurable (e.g., $500)

**7. Alert System**
- Real-time notifications for:
  - Approaching daily loss limits (80%, 90%, 100%)
  - PDT warning (before 3rd day trade)
  - Pattern consecutive losses
  - Low account balance
  - Risk violations
  - Order rejections

**Rationale:** Risk management is non-negotiable in automated trading. These rules protect against catastrophic losses, regulatory violations, and pattern overfitting.

---

### Development Approach

**Timeline:** 4-6 months

**Phase Strategy:**
1. **Paper Trading First** (Months 1-3)
   - Build and test all features in paper trading environment
   - Validate pattern recognition accuracy
   - Ensure risk management works correctly
   - No real money at risk during development

2. **Backtesting Validation** (Ongoing)
   - Test patterns against historical data
   - Validate win rates and profitability
   - Refine pattern detection algorithms

3. **Live Trading** (Month 4+)
   - Switch to live trading only after paper trading success
   - Start with small position sizes
   - Gradually increase based on proven results

**Development Priorities:**
1. Core infrastructure and data pipeline (Weeks 1-4)
2. Risk management system (Weeks 5-8)
3. Pattern recognition (Weeks 9-12)
4. Order execution and automation (Weeks 13-16)
5. User interface (Weeks 17-20)
6. Backtesting and testing (Weeks 21-24)

---

## Architecture Decisions

### Broker Abstraction Layer

**Decision:** Create trait-based abstraction for broker APIs

**Rationale:**
- Avoid vendor lock-in
- Enable switching providers based on cost/features
- Support multi-broker routing in future
- Isolate business logic from broker-specific implementation

**Implementation:**
- `MarketDataProvider` trait for quotes and candles
- `OrderExecutionProvider` trait for orders and positions
- Adapter pattern for each broker (Alpaca, IBKR, etc.)
- Configuration-based provider selection

**Trade-offs:**
- ✅ Flexibility to change providers
- ✅ Cleaner business logic
- ✅ Easier testing (mock providers)
- ❌ More initial development time
- ❌ Abstraction overhead (minimal in Rust)

**Example Future Scenarios:**
- Switch from Alpaca to Interactive Brokers for better options support
- Add Polygon.io for data only, keep Alpaca for execution
- Use different brokers for different account types

---

### Data Storage: DuckDB vs SQLite

**Decision:** Use DuckDB for primary storage

**Rationale:**
- **DuckDB Advantages:**
  - Optimized for analytical queries (OLAP)
  - Fast aggregations for backtesting
  - Parquet file format for efficiency
  - Better performance on time-series data
  - SQL interface familiar to developers

- **SQLite Advantages:**
  - More mature and battle-tested
  - Simpler for transactional workloads
  - Better tooling ecosystem

**Chosen:** DuckDB for time-series data, SQLite as fallback if issues arise

**Trade-offs:**
- ✅ 10-100x faster for analytical queries
- ✅ Better compression (smaller storage)
- ✅ Excellent for backtesting large datasets
- ❌ Less mature than SQLite
- ❌ Smaller community

---

### Python Integration Strategy

**Decision:** Use subprocess-based bridge (not PyO3 embedded)

**Rationale:**
- **Subprocess Approach:**
  - Simpler to implement
  - Better isolation (Python crashes don't kill Rust)
  - Easier debugging
  - Can use any Python version
  - Simpler deployment

- **PyO3 Embedded:**
  - Faster (no serialization overhead)
  - Tighter integration
  - More complex build process
  - Harder to debug

**Chosen:** Subprocess bridge for MVP, evaluate PyO3 if performance issues arise

**Implementation:**
- Rust spawns Python scripts as needed
- JSON for data serialization
- Python handles: indicators, backtesting, LLM calls
- Rust handles: real-time data, orders, risk checks

---

### Cost Optimization Strategy

**Primary Cost Driver:** Market data subscriptions

**Optimization Approach:**
1. **Use Broker-Bundled Data:**
   - Alpaca includes real-time IEX data
   - Eliminates separate data subscription ($200-400/month savings)
   - Single WebSocket feed serves all 10 users

2. **Efficient Data Storage:**
   - Only store 1-minute candles (not tick data)
   - Compress historical data with Parquet
   - Prune old data after X months

3. **Infrastructure:**
   - Desktop app (no server hosting required)
   - Local database (no cloud database costs)
   - Minimal cloud usage (only for backups)

**Cost Projection:**
- **Development:** $0/month (local)
- **Paper Trading:** $0/month (Alpaca free)
- **Live Trading:** ~$50-150/month (optional cloud backups + monitoring)

**Future Cost Flexibility:**
- If Alpaca data quality insufficient: Add Polygon ($200-400/month)
- If need options data: Evaluate cost/benefit
- If need more features: Switch to Interactive Brokers

---

## Key Learnings & Principles

### 1. Risk Management is Non-Negotiable
- Build risk checks BEFORE building trading features
- Hard stops on daily losses prevent catastrophic failures
- PDT compliance is legally required, not optional
- Pattern-specific limits prevent overfitting exploitation
- Always track and log risk violations

### 2. Start Simple, Add Complexity Gradually
- Stocks before options (simpler data model)
- Paper trading before live trading (prove it works)
- Manual execution before automation (understand the system)
- Single broker before multi-broker (reduce integration complexity)

### 3. Architectural Flexibility Pays Off
- Broker abstraction enables future provider changes
- Feature flags allow incremental rollout
- Modular design supports independent testing
- Clear interfaces reduce coupling

### 4. Performance Where It Matters
- Rust for real-time data processing (market data, order routing)
- Python for analysis and research (indicators, backtesting, ML)
- Don't over-optimize: 100ms-1s latency is sufficient for pattern trading

### 5. Cost Consciousness
- Broker APIs that bundle data + execution save money
- Single market data feed serves multiple users
- Local-first architecture avoids cloud costs
- Start cheap, scale only when needed

---

## Technical Constraints & Decisions

### Latency Budget (100ms - 1 second target)

**Breakdown:**
- Market Data Ingestion: 10-50ms (WebSocket)
- Pattern Detection: 20-50ms (Rust processing)
- Risk Validation: 10-20ms (database query + checks)
- Order Submission: 50-200ms (broker API call)
- **Total: 90-320ms** (well within target)

**Bottlenecks to Watch:**
- Database queries (use indexes)
- Network latency to broker API
- Pattern detection on many symbols simultaneously

### Data Flow Architecture

```
Market Data (Alpaca WebSocket)
    ↓
[Rust: Data Normalization & Storage]
    ↓
[Rust: Pattern Detection] ← [Python: Technical Indicators]
    ↓
[Signal Generation]
    ↓
[Risk Validation]
    ↓
[Order Execution] (if approved)
    ↓
[Position & Trade Tracking]
```

### State Management

**Application State:**
- Real-time positions (in-memory + database)
- Active orders (in-memory + database sync)
- Risk metrics (calculated real-time, cached)
- Pattern signals (database + recent cache)

**Data Persistence:**
- All trades and orders (permanent)
- Daily risk metrics (permanent)
- Pattern performance stats (permanent)
- Market data: 1-minute candles (configurable retention)

---

## Pattern Trading Strategy

### Pattern Detection Pipeline

1. **Candlestick Pattern Recognition** (Primary Signal)
   - Detect patterns in recent candles (3-5 period lookback)
   - Assign confidence score (0.0 - 1.0)
   - Classify as bullish/bearish/neutral

2. **Technical Indicator Confirmation** (Secondary Signal)
   - Check RSI for overbought/oversold
   - Verify MACD crossovers
   - Confirm with moving average trends
   - Bollinger Band position

3. **Signal Generation**
   - Combine pattern + indicators
   - Calculate overall confidence
   - Determine buy/sell recommendation
   - Set suggested entry price, stop-loss, target

4. **Risk Check**
   - Validate against daily loss limits
   - Check position size constraints
   - Verify PDT compliance
   - Confirm pattern-specific rules

5. **Execution Decision**
   - **Manual Mode:** Present signal to user for approval
   - **Auto Mode:** Execute if confidence > threshold AND risk checks pass

### Pattern Performance Tracking

**Metrics Tracked Per Pattern:**
- Total signals generated
- Total trades executed
- Winning trades / Losing trades
- Win rate percentage
- Average profit per trade
- Largest win / Largest loss
- Consecutive wins / Consecutive losses
- Time to profit (average holding period)

**Auto-Execution Eligibility:**
- Minimum 20 trades recorded
- Win rate ≥ 60%
- No active consecutive loss streak (≥ 3)
- Pattern not at daily trade limit

---

## Security & Safety

### API Key Management
- **Never hardcode API keys**
- Store in OS keychain:
  - Windows: Credential Manager
  - macOS: Keychain
  - Linux: Secret Service API
- Encrypted at rest
- User enters on first launch

### Data Protection
- Local database encrypted (SQLCipher or disk encryption)
- All broker connections over HTTPS/WSS
- No sensitive data in logs
- Automatic logout after inactivity (optional)

### Code Security
- Regular `cargo audit` for dependency vulnerabilities
- Clippy for code quality
- Memory safety via Rust (no buffer overflows)
- Input validation on all user inputs
- Tauri sandboxing for frontend

---

## Monitoring & Observability

### Logging Strategy
- **Application Logs:** Startup, shutdown, configuration changes
- **Trading Activity:** All orders, fills, rejections
- **Risk Events:** Violations, warnings, limit approaches
- **Errors:** API failures, connection issues, exceptions
- **Performance:** Slow queries, high latency operations

### Metrics to Track
- Orders per minute
- Average order execution time
- Pattern detection latency
- Win rate per pattern (live tracking)
- Daily P&L
- Risk violations count
- API error rate
- WebSocket disconnections

### Alerting
- Critical: Risk violations, order rejections, PDT warnings
- Warning: Approaching limits (80%), connection issues
- Info: Pattern signals, fills, position updates

---

## Testing Strategy

### Unit Tests
- Pattern detection algorithms
- Risk validation logic
- Order builders and validators
- Technical indicator calculations
- Database operations

### Integration Tests
- Broker API integration (paper trading)
- Market data pipeline end-to-end
- Order execution flow
- Risk enforcement
- Database schema migrations

### System Tests
- Full trading workflow (signal → order → fill → tracking)
- Multi-symbol concurrent trading
- Risk limits under various scenarios
- UI responsiveness during market volatility

### Paper Trading Validation (Critical)
- Run for minimum 30 days before going live
- Track pattern performance
- Validate risk management
- Ensure profitability or break-even
- Test all edge cases (market open/close, holidays, halts)

### Backtesting
- Historical pattern validation (minimum 1 year data)
- Compare backtest results to paper trading
- Stress test with volatile periods (2020 COVID crash, etc.)
- Validate indicator calculations against known implementations

---

## Common Pitfalls to Avoid

### 1. Pattern Overfitting
- **Risk:** Pattern works in backtest but fails in live trading
- **Mitigation:** 
  - Require minimum 20 live trades before auto-execution
  - Track live win rate separately from backtest
  - Disable auto if live performance degrades

### 2. Risk Management Bypass
- **Risk:** Bug allows trading beyond limits
- **Mitigation:**
  - Risk checks are the FIRST step (before order submission)
  - Comprehensive unit tests for all risk scenarios
  - Log all risk violations
  - Manual override requires explicit user action

### 3. Data Quality Issues
- **Risk:** Bad data causes false signals
- **Mitigation:**
  - Validate candle data (OHLC relationship)
  - Detect and skip anomalies (huge spikes)
  - Monitor data freshness (disconnect detection)

### 4. Latency Degradation
- **Risk:** System slows down over time
- **Mitigation:**
  - Database indexes on time-series queries
  - Prune old data periodically
  - Monitor query performance
  - Profile and optimize hot paths

### 5. Order Execution Failures
- **Risk:** Order not filled, stuck in pending
- **Mitigation:**
  - Timeout on pending orders (cancel after X seconds)
  - Retry logic with exponential backoff
  - Alert user on repeated failures
  - Track fill rate metric

---

## Future Enhancements (Post-MVP)

### Phase 2: Options Trading
- Options chain data integration
- Greeks calculations (real-time)
- Multi-leg strategies (spreads, straddles)
- Implied volatility tracking
- Options-specific patterns

### Phase 3: Advanced Analytics
- Machine learning for pattern classification
- LLM-based market sentiment analysis
- Custom indicator development framework
- Portfolio optimization algorithms

### Phase 4: Social & Collaboration
- Share watchlists between users
- Pattern performance leaderboard
- Trade journal sharing
- Community pattern library

### Phase 5: Enhanced Risk Management
- Sector exposure limits
- Correlation analysis
- Value-at-Risk (VaR) calculations
- Scenario analysis tools

---

## Glossary

**Day Trade:** Buying and selling (or selling and buying) the same security on the same day.

**Pattern Day Trader (PDT):** A trader who executes 4+ day trades within 5 business days. Accounts < $25k are restricted.

**Candlestick Pattern:** A formation of 1-3 candles that historically indicates potential price movement direction.

**Technical Indicator:** A mathematical calculation based on price/volume data used to predict future price movements (RSI, MACD, etc.).

**Greeks:** Options-specific metrics measuring sensitivity to various factors (delta, gamma, theta, vega, rho).

**Paper Trading:** Simulated trading with fake money to test strategies without risk.

**Backtesting:** Testing a strategy against historical data to evaluate potential performance.

**Stop-Loss:** An order to automatically sell a position if price drops to a specified level (limits losses).

**Trailing Stop:** A stop-loss order that moves up as price increases, locking in profits.

**Bracket Order:** A set of three orders (entry, profit target, stop-loss) submitted simultaneously.

**Slippage:** The difference between expected price and actual execution price.

**Fill:** Execution of an order (partial fill = some shares executed, full fill = all shares executed).

---

## Quick Reference

### Key Numbers
- **Max Users:** 10
- **Target Latency:** 100ms - 1 second
- **Development Timeline:** 4-6 months
- **Data Frequency:** 1-minute candles
- **PDT Threshold:** $25,000
- **PDT Day Trade Limit:** 3 in 5 days (if < $25k)
- **Estimated Cost:** $50-150/month (live trading)

### Critical Files (Future)
- `/src/brokers/traits.rs` - Broker abstraction interfaces
- `/src/risk/manager.rs` - Risk management system
- `/src/patterns/detector.rs` - Pattern detection algorithms
- `/src/execution/engine.rs` - Order execution logic
- `/python/indicators/technical.py` - Technical indicators
- `/python/backtest/engine.py` - Backtesting system

### Useful Commands (Future)
- `cargo run` - Run application in dev mode
- `cargo test` - Run all tests
- `cargo clippy` - Lint Rust code
- `cargo audit` - Check for security vulnerabilities
- `npm run dev` - Run frontend in dev mode
- `npm run build` - Build frontend for production
- `tauri build` - Create distributable binaries

---

## Contact & Resources

**Primary Developer:** Mike

**Key External Resources:**
- Alpaca API Docs: https://alpaca.markets/docs/
- Tauri Docs: https://tauri.app/
- Rust Tokio Docs: https://tokio.rs/
- React Docs: https://react.dev/
- TradingView Lightweight Charts: https://tradingview.github.io/lightweight-charts/

**Community:**
- Alpaca Slack: https://alpaca.markets/slack
- Tauri Discord: https://discord.com/invite/tauri
- Rust Discord: https://discord.gg/rust-lang

---

**Last Updated:** November 15, 2025  
**Next Review:** After Phase 1 completion (Week 4)  
**Document Version:** 1.0
