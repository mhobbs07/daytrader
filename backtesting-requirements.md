# Backtesting Requirements & Strategy

**Priority:** CRITICAL - Must complete before any live trading  
**Timeline:** Integrated into Phase 6 (Weeks 21-26)  
**Gate to Live Trading:** Mandatory validation step

---

## Overview

Comprehensive backtesting is **mandatory** before transitioning to live trading. This document outlines the requirements, strategy, and success criteria for validating trading patterns against historical data.

---

## Backtesting Objectives

### Primary Goals

1. **Validate Pattern Effectiveness**
   - Prove patterns generate profitable signals
   - Identify which patterns work in which market conditions
   - Determine optimal confidence thresholds

2. **Risk Management Validation**
   - Verify risk rules prevent catastrophic losses
   - Test daily loss limits under various scenarios
   - Validate position sizing logic

3. **Performance Metrics**
   - Calculate expected returns
   - Measure maximum drawdown
   - Determine win rate per pattern
   - Assess risk-adjusted returns (Sharpe ratio)

4. **Market Condition Analysis**
   - Test across bull markets
   - Test across bear markets
   - Test across sideways/choppy markets
   - Test during high volatility periods

---

## Backtesting Requirements

### Data Requirements

**Historical Data Scope:**
- **Minimum:** 1 year of 1-minute candle data
- **Recommended:** 2-3 years for robust validation
- **Critical Periods:** Must include:
  - 2020 COVID crash (Feb-Mar 2020)
  - 2022 bear market
  - Recent bull market periods
  - High volatility events

**Data Quality:**
- Clean OHLCV data with no gaps
- Validated candle relationships (O/H/L/C)
- Adjusted for splits and dividends
- Volume data included

**Symbol Coverage:**
- Test on 20-50 liquid stocks
- Include various sectors
- Mix of volatility profiles (low, medium, high)
- Include both trending and range-bound stocks

### Backtesting Engine Requirements

**Core Functionality:**
```rust
pub struct BacktestEngine {
    // Historical data
    data_provider: Box<dyn HistoricalDataProvider>,
    
    // Strategy configuration
    patterns: Vec<PatternConfig>,
    indicators: IndicatorConfig,
    
    // Risk management
    risk_config: RiskConfig,
    
    // Execution simulation
    slippage_model: SlippageModel,
    commission_model: CommissionModel,
    
    // Results tracking
    results: BacktestResults,
}

pub struct BacktestResults {
    // Performance metrics
    total_return: Decimal,
    annualized_return: Decimal,
    sharpe_ratio: Decimal,
    sortino_ratio: Decimal,
    max_drawdown: Decimal,
    
    // Trade statistics
    total_trades: u32,
    winning_trades: u32,
    losing_trades: u32,
    win_rate: Decimal,
    avg_win: Decimal,
    avg_loss: Decimal,
    profit_factor: Decimal,
    
    // Pattern-specific results
    pattern_performance: HashMap<String, PatternPerformance>,
    
    // Time-based analysis
    monthly_returns: Vec<MonthlyReturn>,
    drawdown_periods: Vec<DrawdownPeriod>,
    
    // Trade log
    trades: Vec<BacktestTrade>,
}
```

**Realistic Simulation:**
- **Slippage Model:** 0.05-0.10% per trade (market orders)
- **Commission:** $0 (Alpaca commission-free)
- **Fill Assumptions:** 
  - Market orders: Fill at next candle open
  - Limit orders: Fill only if price reached
- **Latency Simulation:** 100-500ms delay between signal and order
- **Market Hours:** Only trade during regular hours (9:30 AM - 4:00 PM ET)

---

## Backtesting Strategy

### Phase 6A: Backtesting Engine Development (Weeks 21-22)

**Week 21: Core Engine**
- [ ] Historical data retrieval from Alpaca
- [ ] Data storage in DuckDB (optimized for analytics)
- [ ] Candle replay mechanism
- [ ] Pattern detection on historical data
- [ ] Indicator calculation pipeline
- [ ] Signal generation logic

**Week 22: Execution Simulation**
- [ ] Order simulation (market, limit)
- [ ] Position tracking
- [ ] P&L calculation
- [ ] Risk rule enforcement
- [ ] Slippage and commission modeling
- [ ] Results aggregation

**Deliverable:** Working backtesting engine

---

### Phase 6B: Comprehensive Backtesting (Weeks 23-24)

**Week 23: Pattern Validation**

**Test Matrix:**

| Pattern | Bull Market | Bear Market | Sideways | High Vol | Overall |
|---------|-------------|-------------|----------|----------|---------|
| Hammer | Test | Test | Test | Test | Aggregate |
| Shooting Star | Test | Test | Test | Test | Aggregate |
| Doji | Test | Test | Test | Test | Aggregate |
| Bullish Engulfing | Test | Test | Test | Test | Aggregate |
| Bearish Engulfing | Test | Test | Test | Test | Aggregate |

**For Each Pattern:**
- [ ] Run backtest on 1+ year data
- [ ] Calculate win rate
- [ ] Calculate average profit per trade
- [ ] Identify best market conditions
- [ ] Determine optimal confidence threshold
- [ ] Document failure modes

**Week 24: Strategy Optimization**
- [ ] Test indicator combinations
- [ ] Optimize entry/exit rules
- [ ] Test different position sizing strategies
- [ ] Validate risk management rules
- [ ] Compare pattern performance
- [ ] Generate comprehensive report

**Deliverable:** Validated patterns with documented performance

---

### Phase 6C: Paper Trading Validation (Weeks 25-26)

**Week 25-26: Live Paper Trading**

**Objective:** Verify backtest results translate to real-time trading

**Requirements:**
- [ ] Run paper trading for minimum 30 days
- [ ] Track all pattern signals
- [ ] Execute trades in paper account
- [ ] Compare results to backtest expectations
- [ ] Monitor for unexpected behaviors
- [ ] Document any discrepancies

**Success Criteria:**
- Paper trading win rate within ±10% of backtest
- No critical bugs discovered
- Risk management functions correctly
- System handles market hours, holidays, halts
- Performance meets or exceeds expectations

**Deliverable:** Validated system ready for live trading consideration

---

## Success Criteria for Live Trading

### Mandatory Requirements (All Must Pass)

**Backtesting Results:**
- ✅ Minimum 1 year of backtesting completed
- ✅ Overall strategy shows positive returns OR acceptable risk/reward
- ✅ Maximum drawdown < 20%
- ✅ Sharpe ratio > 1.0 (preferably > 1.5)
- ✅ Win rate > 55% across all patterns
- ✅ Profit factor > 1.5 (gross profit / gross loss)

**Pattern Performance:**
- ✅ Each enabled pattern has >55% win rate in backtest
- ✅ Each pattern profitable in at least 2 of 3 market conditions
- ✅ No pattern shows catastrophic losses in any condition
- ✅ Pattern performance documented and understood

**Paper Trading Validation:**
- ✅ 30+ days successful paper trading
- ✅ Paper trading results align with backtest (±10%)
- ✅ Zero critical bugs in risk management
- ✅ All risk rules enforced correctly
- ✅ PDT compliance verified
- ✅ Order execution reliability >95%

**System Quality:**
- ✅ All tests passing (unit, integration, system)
- ✅ Security audit complete
- ✅ Performance benchmarks met
- ✅ Documentation complete
- ✅ Monitoring and alerting operational

**Risk Management:**
- ✅ Daily loss limits never exceeded in paper trading
- ✅ Position sizing rules enforced
- ✅ Emergency stop mechanism tested
- ✅ Audit trail complete and accurate

---

## Backtesting Metrics & Reporting

### Key Performance Indicators

**Returns:**
- Total return (%)
- Annualized return (%)
- Monthly returns (distribution)
- Best/worst month

**Risk Metrics:**
- Maximum drawdown (%)
- Average drawdown (%)
- Drawdown duration (days)
- Volatility (standard deviation)
- Downside deviation

**Risk-Adjusted Returns:**
- Sharpe ratio (return / volatility)
- Sortino ratio (return / downside deviation)
- Calmar ratio (return / max drawdown)

**Trade Statistics:**
- Total trades
- Win rate (%)
- Average win ($)
- Average loss ($)
- Profit factor (gross profit / gross loss)
- Expectancy (average profit per trade)
- Largest win/loss

**Pattern-Specific:**
- Win rate per pattern
- Profit per pattern
- Best/worst performing patterns
- Pattern frequency
- Pattern reliability by market condition

### Reporting Format

**Backtest Summary Report:**
```
=== BACKTEST SUMMARY ===
Period: 2023-01-01 to 2024-12-31 (2 years)
Initial Capital: $10,000
Final Value: $12,500
Total Return: 25.0%
Annualized Return: 11.8%

Risk Metrics:
- Max Drawdown: -8.5%
- Sharpe Ratio: 1.85
- Sortino Ratio: 2.34

Trade Statistics:
- Total Trades: 247
- Win Rate: 58.3%
- Profit Factor: 1.82
- Avg Win: $125
- Avg Loss: $68

Pattern Performance:
1. Bullish Engulfing: 62% win rate, $3,200 profit
2. Hammer: 59% win rate, $2,800 profit
3. Bearish Engulfing: 57% win rate, $1,900 profit
4. Shooting Star: 55% win rate, $1,200 profit
5. Doji: 52% win rate, $400 profit

Market Condition Analysis:
- Bull Market (60% of period): +18% return
- Bear Market (25% of period): -3% return
- Sideways (15% of period): +2% return

Recommendation: APPROVED for paper trading validation
```

---

## Backtesting Workflow

### Step-by-Step Process

**1. Data Preparation**
```bash
# Download historical data
python scripts/download_historical_data.py \
  --symbols SPY,AAPL,MSFT,GOOGL,AMZN,TSLA \
  --start-date 2023-01-01 \
  --end-date 2024-12-31 \
  --timeframe 1min

# Validate data quality
python scripts/validate_data.py --check-gaps --check-ohlc
```

**2. Configure Backtest**
```toml
# config/backtest.toml
[backtest]
start_date = "2023-01-01"
end_date = "2024-12-31"
initial_capital = 10000.00
symbols = ["SPY", "AAPL", "MSFT", "GOOGL", "AMZN"]

[execution]
slippage_percent = 0.05
commission_per_trade = 0.00
latency_ms = 250

[patterns]
enabled = ["hammer", "shooting_star", "doji", "bullish_engulfing", "bearish_engulfing"]
min_confidence = 0.7

[risk]
max_position_size_percent = 10.0
max_concurrent_positions = 5
max_daily_loss_percent = 2.0
```

**3. Run Backtest**
```bash
cargo run --release --bin backtest -- \
  --config config/backtest.toml \
  --output results/backtest_2023_2024.json
```

**4. Analyze Results**
```bash
# Generate report
python scripts/analyze_backtest.py \
  --input results/backtest_2023_2024.json \
  --output reports/backtest_report.html

# Compare patterns
python scripts/compare_patterns.py \
  --input results/backtest_2023_2024.json
```

**5. Visualize Performance**
- Equity curve
- Drawdown chart
- Monthly returns heatmap
- Pattern performance comparison
- Trade distribution

**6. Document Findings**
- What worked well
- What didn't work
- Market conditions impact
- Recommended adjustments
- Go/no-go decision

---

## Risk Scenarios to Test

### Stress Testing

**1. Market Crash Scenario**
- Test on 2020 COVID crash (Feb-Mar 2020)
- Verify risk limits prevent catastrophic losses
- Ensure system doesn't overtrade during volatility

**2. Extended Bear Market**
- Test on 2022 bear market
- Verify patterns adapt or stop trading
- Check drawdown stays within limits

**3. Low Volatility Period**
- Test on low-volatility sideways market
- Verify system doesn't generate false signals
- Check profitability in choppy conditions

**4. Gap Events**
- Test behavior on overnight gaps
- Verify stop-losses work correctly
- Check position sizing adjusts

**5. Rapid Reversals**
- Test on whipsaw markets
- Verify pattern detection accuracy
- Check for overtrading

---

## Continuous Backtesting

### Ongoing Validation

**After Live Trading Begins:**
- Run weekly backtests on recent data
- Compare live results to backtest expectations
- Identify pattern degradation early
- Adjust strategies based on market changes

**Monthly Review:**
- Backtest new patterns before enabling
- Re-validate existing patterns
- Update confidence thresholds
- Document performance changes

---

## Tools & Infrastructure

### Backtesting Stack

**Data Storage:**
- DuckDB for historical candles (fast analytics)
- Parquet files for long-term storage

**Execution:**
- Rust backtesting engine (performance)
- Python for analysis and visualization

**Visualization:**
- Plotly for interactive charts
- Jupyter notebooks for exploration
- HTML reports for sharing

**Version Control:**
- Git for code
- Track backtest configurations
- Version backtest results

---

## Gate to Live Trading Decision

### Decision Framework

**After completing all backtesting and paper trading:**

**GREEN LIGHT (Proceed to Live Trading):**
- ✅ All success criteria met
- ✅ Backtest shows consistent profitability
- ✅ Paper trading validates backtest
- ✅ Risk management proven effective
- ✅ Team confident in system

**YELLOW LIGHT (More Testing Needed):**
- ⚠️ Some criteria met, others marginal
- ⚠️ Paper trading shows discrepancies
- ⚠️ Patterns work but need refinement
- **Action:** Extend paper trading, refine patterns

**RED LIGHT (Do Not Proceed):**
- ❌ Backtest shows losses or high risk
- ❌ Paper trading fails to validate
- ❌ Critical bugs discovered
- ❌ Risk management failures
- **Action:** Major redesign required

---

## Revised Timeline with Backtesting

### Updated Phase 6: Backtesting & Validation (Weeks 21-26)

**Weeks 21-22: Backtesting Engine**
- Build historical data pipeline
- Implement backtest execution engine
- Create results analysis tools

**Weeks 23-24: Comprehensive Backtesting**
- Run 1+ year backtests on all patterns
- Test across market conditions
- Optimize and refine strategies
- Generate performance reports

**Weeks 25-26: Paper Trading Validation**
- 30-day minimum paper trading
- Compare to backtest expectations
- Final bug fixes and refinements
- Go/no-go decision for live trading

**Week 27+: Live Trading (If Approved)**
- Start with small position sizes
- Monitor closely for first 30 days
- Gradually increase based on performance
- Continuous validation against backtest

---

## Conclusion

Backtesting is **not optional** - it is a critical validation step that must be completed before any live trading. The comprehensive approach outlined here ensures:

1. Patterns are proven profitable
2. Risk management is effective
3. System performs as expected
4. Team has confidence in the strategy

**No live trading will occur until all backtesting and paper trading requirements are met.**

---

**Document Version:** 1.0  
**Last Updated:** December 16, 2025  
**Status:** Mandatory Requirement
