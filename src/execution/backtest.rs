//! Historical replay (backtesting) engine
//!
//! Replays historical candles through the *production* trading pipeline —
//! pattern detection, signal generation, strategy evaluation, risk-gated
//! order placement, and simulated fills — and produces a performance report
//! with per-pattern attribution.
//!
//! # Design
//!
//! The replay is fully synchronous per candle to stay deterministic: no
//! event-bus races between detection, execution, and fills. Time is driven
//! by a [`SteppedClock`] set to each candle's timestamp, so market-hours
//! rules and order timestamps observe replay time.
//!
//! Fills are simulated by the [`MockBroker`] in auto-fill mode (limit orders
//! fill at their limit price on the next status poll). Slippage and
//! commissions are applied in the accounting layer per side.
//!
//! # v1 limitations (documented, intentional)
//!
//! - Long-only: sell signals with no open position are counted and skipped,
//!   not shorted.
//! - The risk engine's asynchronous event task is not started; static
//!   pre-trade checks (buying power, halt state, pattern limits, market
//!   hours) still run inside the order engine, but day-trade counting does
//!   not accrue across the replay.
//! - Positions still open at the end of the run are marked to the last
//!   close rather than force-liquidated.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::brokers::mock::{MockBroker, MockConfig};
use crate::brokers::traits::{Candle, OrderExecutionProvider, Quote};
use crate::brokers::types::{PatternName, Price, Symbol};
use crate::config::RiskConfig;
use crate::events::bus::EventBus;
use crate::events::types::{MarketEvent, SignalType};
use crate::execution::ExecutionError;
use crate::execution::engine::{ExecutionConfig, ExecutionEngine};
use crate::execution::strategy::ExecutionStrategy;
use crate::orders::broker_adapter::AlpacaAdapter;
use crate::orders::engine::OrderEngine;
use crate::orders::types::{FillInfo, ManagedOrder, OrderConfig};
use crate::patterns::registry::PatternRegistry;
use crate::patterns::signals::{SignalConfig, SignalEngine};
use crate::risk::RiskEngine;
use crate::risk::types::AccountId;
use crate::utils::clock::SteppedClock;

/// Configuration for a backtest run
#[derive(Clone, Debug)]
pub struct BacktestConfig {
    /// Starting account equity
    pub initial_equity: Decimal,
    /// Flat commission charged per fill
    pub commission_per_fill: Decimal,
    /// Slippage applied per side, in basis points of the fill price
    pub slippage_bps: u32,
    /// Risk engine configuration for the replay account
    pub risk_config: RiskConfig,
    /// Execution engine configuration (automation is forced ON for replay)
    pub execution_config: ExecutionConfig,
    /// How many recent candles feed each detection pass
    pub detection_window: usize,
}

impl BacktestConfig {
    /// Creates a config with sensible replay defaults on top of the given
    /// risk configuration: $100k equity, $0 commission, 5 bps slippage,
    /// automation on with every pattern enabled.
    pub fn new(risk_config: RiskConfig, pattern_names: &[&str]) -> Self {
        let execution_config = ExecutionConfig {
            auto_execution_enabled: true,
            pattern_auto_enabled: pattern_names
                .iter()
                .map(|name| ((*name).to_string(), true))
                .collect(),
            ..ExecutionConfig::default()
        };

        Self {
            initial_equity: Decimal::new(10_000_000, 2), // $100,000.00
            commission_per_fill: Decimal::ZERO,
            slippage_bps: 5,
            risk_config,
            execution_config,
            detection_window: 50,
        }
    }
}

/// Outcome statistics for a single pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternOutcome {
    /// Pattern that opened the round trips
    pub pattern_name: String,
    /// Completed round trips attributed to this pattern
    pub round_trips: usize,
    /// Profitable round trips
    pub wins: usize,
    /// Losing round trips
    pub losses: usize,
    /// Net realized P&L across the pattern's round trips
    pub net_pnl: Decimal,
}

/// Result of a backtest run
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestReport {
    /// Candles fed into the replay
    pub candles_processed: usize,
    /// Raw pattern detections across the run
    pub patterns_detected: usize,
    /// Trading signals generated from detections
    pub signals_generated: usize,
    /// Sell signals skipped because nothing was open (long-only v1)
    pub short_signals_skipped: usize,
    /// Orders accepted by the order engine
    pub orders_placed: usize,
    /// Fills applied
    pub orders_filled: usize,
    /// Completed round trips (entry lot fully closed)
    pub round_trips: usize,
    /// Profitable round trips
    pub wins: usize,
    /// Losing round trips
    pub losses: usize,
    /// wins / round_trips, 0.0 when no trades completed
    pub win_rate: f64,
    /// Realized P&L before commissions (slippage already applied per side)
    pub gross_realized_pnl: Decimal,
    /// Total commissions charged
    pub commissions: Decimal,
    /// Realized P&L net of commissions
    pub net_realized_pnl: Decimal,
    /// Unrealized P&L of positions still open at the end, marked to last close
    pub open_position_pnl: Decimal,
    /// initial_equity + net realized + open position mark
    pub ending_equity: Decimal,
    /// Per-pattern attribution, sorted by net P&L descending
    pub per_pattern: Vec<PatternOutcome>,
    /// (timestamp, equity) sampled at every candle
    pub equity_curve: Vec<(DateTime<Utc>, Decimal)>,
}

/// A FIFO entry lot opened by a buy fill
#[derive(Clone, Debug)]
struct OpenLot {
    quantity: u32,
    /// Entry price with buy-side slippage already applied
    entry_price: Decimal,
    entry_time: DateTime<Utc>,
    pattern: Option<PatternName>,
}

/// Replays historical candles through the production trading pipeline
pub struct BacktestEngine {
    config: BacktestConfig,
    pattern_registry: Arc<PatternRegistry>,
    strategy: Arc<dyn ExecutionStrategy>,
}

impl BacktestEngine {
    /// Creates a backtest engine
    ///
    /// `pattern_registry` decides which detectors run; `strategy` is the
    /// same [`ExecutionStrategy`] type the live execution engine uses.
    pub fn new(
        config: BacktestConfig,
        pattern_registry: Arc<PatternRegistry>,
        strategy: Arc<dyn ExecutionStrategy>,
    ) -> Self {
        Self {
            config,
            pattern_registry,
            strategy,
        }
    }

    /// Runs the replay over `candles` (any symbol mix; sorted internally)
    ///
    /// # Errors
    ///
    /// Returns an error if the isolated infrastructure (in-memory database)
    /// cannot be set up or an engine call fails unrecoverably. Strategy and
    /// risk rejections of individual signals are not errors — they are
    /// counted in the report.
    pub async fn run(&self, mut candles: Vec<Candle>) -> Result<BacktestReport, ExecutionError> {
        candles.sort_by_key(|c| c.timestamp);

        let Some(first) = candles.first() else {
            return Ok(Self::empty_report());
        };

        // ---- Isolated infrastructure for this run ----
        let event_bus = Arc::new(EventBus::new(65_536));
        let clock = SteppedClock::new(first.timestamp);

        let broker = Arc::new(MockBroker::new(MockConfig {
            auto_fill_orders: true,
            ..MockConfig::default()
        }));

        let db_pool = Arc::new(
            sqlx::SqlitePool::connect(":memory:")
                .await
                .map_err(ExecutionError::DatabaseError)?,
        );
        sqlx::query(include_str!("../db/schema.sql"))
            .execute(&*db_pool)
            .await
            .map_err(ExecutionError::DatabaseError)?;
        sqlx::query(
            "INSERT INTO accounts (broker, account_number, account_type) \
             VALUES ('backtest', 'backtest-account', 'paper')",
        )
        .execute(&*db_pool)
        .await
        .map_err(ExecutionError::DatabaseError)?;

        let risk_engine = Arc::new(RiskEngine::new(
            self.config.risk_config.clone(),
            Arc::clone(&event_bus),
            AccountId::new("backtest-account"),
            self.config.initial_equity,
        ));

        let order_engine = Arc::new(
            OrderEngine::new(
                broker.clone() as Arc<dyn OrderExecutionProvider>,
                Arc::clone(&risk_engine),
                Arc::new(AlpacaAdapter::new(
                    broker.clone() as Arc<dyn OrderExecutionProvider>
                )),
                Arc::clone(&event_bus),
                Arc::clone(&db_pool),
                1,
                OrderConfig::default(),
            )
            .with_clock(Arc::new(clock.clone())),
        );

        let exec_positions = Arc::new(crate::positions::PositionTracker::new(
            broker.clone() as Arc<dyn OrderExecutionProvider>,
            (*event_bus).clone(),
            Arc::clone(&db_pool),
            crate::positions::PnLConfig::default(),
            crate::positions::tracker::PositionConfig::default(),
        ));

        let execution_engine = ExecutionEngine::new(
            Arc::clone(&order_engine),
            Arc::clone(&exec_positions),
            Arc::clone(&risk_engine),
            Arc::clone(&event_bus),
            Arc::clone(&db_pool),
            Arc::clone(&self.strategy),
            self.config.execution_config.clone(),
        )
        .await?
        .with_clock(Arc::new(clock.clone()));
        execution_engine.enable_automation();

        let mut signal_engine = SignalEngine::new(SignalConfig::default());
        let mut fill_rx = event_bus.subscribe();

        // ---- Accounting state ----
        let mut history: HashMap<String, Vec<Candle>> = HashMap::new();
        let mut lots: HashMap<String, VecDeque<OpenLot>> = HashMap::new();
        let mut last_close: HashMap<String, Decimal> = HashMap::new();
        let mut per_pattern: HashMap<String, PatternOutcome> = HashMap::new();
        let mut report = Self::empty_report();

        let slippage = Decimal::from(self.config.slippage_bps) / Decimal::from(10_000u32);

        // ---- Replay loop ----
        for candle in &candles {
            clock.set(candle.timestamp);
            report.candles_processed += 1;

            let symbol_key = candle.symbol.as_str().to_string();
            last_close.insert(symbol_key.clone(), candle.close.value());
            broker.set_mock_quote(quote_from_candle(candle));

            let window = history.entry(symbol_key.clone()).or_default();
            window.push(candle.clone());
            if window.len() > self.config.detection_window {
                window.remove(0);
            }

            if window.len() >= 3 {
                let window_snapshot = window.clone();
                // detect_all wants newest-first; the window (and the signal
                // engine below) are chronological
                let detection_window: Vec<Candle> = window_snapshot.iter().rev().cloned().collect();
                let detections = self
                    .pattern_registry
                    .detect_all(&detection_window, None)
                    .await;
                report.patterns_detected += detections.len();

                if !detections.is_empty() {
                    let signals = signal_engine
                        .generate_signals(&detections, &window_snapshot)
                        .await
                        .unwrap_or_default();
                    report.signals_generated += signals.len();

                    for mut signal in signals {
                        // The signal engine does not carry the symbol through
                        // (known limitation); stamp the replay symbol on it.
                        signal.symbol = candle.symbol.clone();

                        // v1 is long-only: a sell with nothing open would be
                        // a short — count it and move on.
                        let has_open = lots.get(&symbol_key).is_some_and(|l| !l.is_empty());
                        if matches!(signal.signal_type, SignalType::Sell) && !has_open {
                            report.short_signals_skipped += 1;
                            continue;
                        }

                        match execution_engine.process_signal(signal).await {
                            Ok(Some(_)) => report.orders_placed += 1,
                            Ok(None) => {}
                            Err(e) => {
                                tracing::debug!("Replay signal rejected: {e}");
                            }
                        }
                    }
                }
            }

            // Surface simulated fills and account for them
            order_engine.sync_order_statuses().await?;
            loop {
                match fill_rx.try_recv() {
                    Ok(MarketEvent::OrderFilled { order_id, fill }) => {
                        let Ok(order) = order_engine.get_order(&order_id).await else {
                            continue;
                        };
                        report.orders_filled += 1;
                        report.commissions += self.config.commission_per_fill;

                        // Keep the strategy's position context honest
                        let fill_info = FillInfo {
                            filled_quantity: fill.filled_quantity,
                            filled_price: fill.filled_price,
                            commission: self.config.commission_per_fill,
                            timestamp: fill.filled_at,
                        };
                        if let Err(e) = exec_positions.update_from_fill(&order, &fill_info).await {
                            tracing::debug!("Replay position update failed: {e}");
                        }

                        self.account_fill(
                            &order,
                            fill.filled_price.value(),
                            fill.filled_quantity.value(),
                            fill.filled_at,
                            slippage,
                            &mut lots,
                            &mut per_pattern,
                            &mut report,
                            &execution_engine,
                        )
                        .await;
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }

            // Equity sample: realized net + open lots marked to last close
            let open_pnl = mark_open_lots(&lots, &last_close);
            report.equity_curve.push((
                candle.timestamp,
                self.config.initial_equity + report.gross_realized_pnl - report.commissions
                    + open_pnl,
            ));
        }

        // ---- Final marks ----
        report.open_position_pnl = mark_open_lots(&lots, &last_close);
        report.net_realized_pnl = report.gross_realized_pnl - report.commissions;
        report.ending_equity =
            self.config.initial_equity + report.net_realized_pnl + report.open_position_pnl;
        report.win_rate = if report.round_trips > 0 {
            report.wins as f64 / report.round_trips as f64
        } else {
            0.0
        };
        report.per_pattern = {
            let mut outcomes: Vec<PatternOutcome> = per_pattern.into_values().collect();
            outcomes.sort_by(|a, b| b.net_pnl.cmp(&a.net_pnl));
            outcomes
        };

        Ok(report)
    }

    /// Applies a fill to the FIFO lot books and round-trip statistics
    #[allow(clippy::too_many_arguments)]
    async fn account_fill(
        &self,
        order: &ManagedOrder,
        fill_price: Decimal,
        fill_quantity: u32,
        fill_time: DateTime<Utc>,
        slippage: Decimal,
        lots: &mut HashMap<String, VecDeque<OpenLot>>,
        per_pattern: &mut HashMap<String, PatternOutcome>,
        report: &mut BacktestReport,
        execution_engine: &ExecutionEngine,
    ) {
        let symbol_key = order.symbol.as_str().to_string();

        match order.side {
            crate::brokers::traits::OrderSide::Buy => {
                lots.entry(symbol_key).or_default().push_back(OpenLot {
                    quantity: fill_quantity,
                    entry_price: fill_price * (Decimal::ONE + slippage),
                    entry_time: fill_time,
                    pattern: order.pattern_name.clone(),
                });
            }
            crate::brokers::traits::OrderSide::Sell => {
                let exit_price = fill_price * (Decimal::ONE - slippage);
                let mut remaining = u64::from(fill_quantity);
                let book = lots.entry(symbol_key).or_default();

                while remaining > 0 {
                    let Some(mut lot) = book.pop_front() else {
                        // Over-sell beyond open lots — ignore the excess
                        // (long-only replay never shorts)
                        break;
                    };

                    let consumed = remaining.min(u64::from(lot.quantity));
                    let realized = (exit_price - lot.entry_price) * Decimal::from(consumed);
                    remaining -= consumed;

                    report.gross_realized_pnl += realized;
                    report.round_trips += 1;
                    if realized > Decimal::ZERO {
                        report.wins += 1;
                    } else {
                        report.losses += 1;
                    }

                    let holding_secs = (fill_time - lot.entry_time).num_seconds().max(0) as u64;
                    if let Some(pattern) = &lot.pattern {
                        let entry = per_pattern
                            .entry(pattern.as_str().to_string())
                            .or_insert_with(|| PatternOutcome {
                                pattern_name: pattern.as_str().to_string(),
                                round_trips: 0,
                                wins: 0,
                                losses: 0,
                                net_pnl: Decimal::ZERO,
                            });
                        entry.round_trips += 1;
                        if realized > Decimal::ZERO {
                            entry.wins += 1;
                        } else {
                            entry.losses += 1;
                        }
                        entry.net_pnl += realized;

                        // Feed the live per-pattern automation feedback loop
                        if let Err(e) = execution_engine
                            .record_trade_outcome(pattern, realized, holding_secs)
                            .await
                        {
                            tracing::debug!("record_trade_outcome failed in replay: {e}");
                        }
                    }

                    if u64::from(lot.quantity) > consumed {
                        lot.quantity -= consumed as u32;
                        book.push_front(lot);
                    }
                }
            }
        }
    }

    fn empty_report() -> BacktestReport {
        BacktestReport {
            candles_processed: 0,
            patterns_detected: 0,
            signals_generated: 0,
            short_signals_skipped: 0,
            orders_placed: 0,
            orders_filled: 0,
            round_trips: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            gross_realized_pnl: Decimal::ZERO,
            commissions: Decimal::ZERO,
            net_realized_pnl: Decimal::ZERO,
            open_position_pnl: Decimal::ZERO,
            ending_equity: Decimal::ZERO,
            per_pattern: Vec::new(),
            equity_curve: Vec::new(),
        }
    }
}

/// Marks all open lots to the most recent close per symbol
fn mark_open_lots(
    lots: &HashMap<String, VecDeque<OpenLot>>,
    last_close: &HashMap<String, Decimal>,
) -> Decimal {
    lots.iter()
        .map(|(symbol, book)| {
            let Some(close) = last_close.get(symbol) else {
                return Decimal::ZERO;
            };
            book.iter()
                .map(|lot| (*close - lot.entry_price) * Decimal::from(lot.quantity))
                .sum::<Decimal>()
        })
        .sum()
}

/// Builds a quote around a candle's close (1 cent synthetic spread)
fn quote_from_candle(candle: &Candle) -> Quote {
    let close = candle.close.value();
    let tick = Decimal::new(1, 2);
    Quote {
        symbol: candle.symbol.clone(),
        bid: Price::new(close - tick).unwrap_or(candle.close),
        ask: Price::new(close + tick).unwrap_or(candle.close),
        last: candle.close,
        bid_size: 100,
        ask_size: 100,
        timestamp: candle.timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn candle(
        symbol: &Symbol,
        ts: DateTime<Utc>,
        open: i64,
        high: i64,
        low: i64,
        close: i64,
        volume: u64,
    ) -> Candle {
        Candle {
            symbol: symbol.clone(),
            timestamp: ts,
            open: Price::new(Decimal::new(open, 2)).unwrap(),
            high: Price::new(Decimal::new(high, 2)).unwrap(),
            low: Price::new(Decimal::new(low, 2)).unwrap(),
            close: Price::new(Decimal::new(close, 2)).unwrap(),
            volume,
        }
    }

    fn test_risk_config() -> RiskConfig {
        RiskConfig {
            max_daily_loss_dollars: 10_000.0,
            max_daily_loss_percent: 10.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 50_000.0,
            max_position_size_percent: 50.0,
            max_concurrent_positions: 10,
            enforce_pdt_rule: true,
            pdt_min_equity: 25_000.0,
            max_day_trades_under_pdt: 3,
            max_trades_per_pattern_per_day: 100,
            stop_pattern_after_consecutive_losses: 100,
            required_win_rate_for_auto: 0.0,
            min_pattern_sample_size: 1,
            min_account_balance: 1_000.0,
            emergency_liquidate_threshold: 500.0,
            pattern_profiles: std::collections::HashMap::new(),
            confidence_multipliers: vec![],
            pattern_performance_thresholds: Default::default(),
        }
    }

    #[test]
    fn test_quote_from_candle_wraps_close() {
        let symbol = Symbol::new("AAPL").unwrap();
        let ts = Utc.with_ymd_and_hms(2024, 6, 12, 15, 0, 0).unwrap();
        let c = candle(&symbol, ts, 15000, 15100, 14900, 15050, 1000);

        let quote = quote_from_candle(&c);
        assert_eq!(quote.last, c.close);
        assert!(quote.bid.value() < quote.ask.value());
    }

    #[test]
    fn test_mark_open_lots_uses_last_close() {
        let mut lots: HashMap<String, VecDeque<OpenLot>> = HashMap::new();
        lots.entry("AAPL".to_string())
            .or_default()
            .push_back(OpenLot {
                quantity: 10,
                entry_price: Decimal::new(15000, 2), // $150.00
                entry_time: Utc::now(),
                pattern: None,
            });

        let mut closes = HashMap::new();
        closes.insert("AAPL".to_string(), Decimal::new(15100, 2)); // $151.00

        // (151 - 150) * 10 = $10.00
        assert_eq!(mark_open_lots(&lots, &closes), Decimal::new(1000, 2));
    }

    #[tokio::test]
    async fn test_run_with_no_candles_returns_empty_report() {
        let config = BacktestConfig::new(test_risk_config(), &["hammer"]);
        let engine = BacktestEngine::new(
            config,
            Arc::new(PatternRegistry::new()),
            Arc::new(crate::execution::strategy::ConservativeStrategy::new(
                crate::execution::strategy::ConservativeConfig::default(),
            )),
        );

        let report = engine.run(vec![]).await.unwrap();
        assert_eq!(report.candles_processed, 0);
        assert_eq!(report.round_trips, 0);
        assert!(report.equity_curve.is_empty());
    }

    /// Strategy with no gating: fixed 10-share limit orders at the signal
    /// price. Keeps the replay test deterministic — the production
    /// strategies embed wall-clock/threshold rules that are exercised by
    /// their own tests.
    #[derive(Debug)]
    struct PermissiveStrategy;

    #[async_trait::async_trait]
    impl ExecutionStrategy for PermissiveStrategy {
        fn should_execute(
            &self,
            _signal: &crate::patterns::signals::TradingSignal,
            _context: &crate::execution::strategy::ExecutionContext,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn calculate_position_size(
            &self,
            _signal: &crate::patterns::signals::TradingSignal,
            _account: &crate::brokers::traits::Account,
            _risk_config: &RiskConfig,
        ) -> Result<crate::brokers::types::Quantity, ExecutionError> {
            crate::brokers::types::Quantity::new(10)
                .map_err(|e| ExecutionError::PositionSizingError(e.to_string()))
        }

        fn determine_order_type(
            &self,
            _signal: &crate::patterns::signals::TradingSignal,
            _current_price: Price,
        ) -> crate::brokers::traits::OrderType {
            crate::brokers::traits::OrderType::Limit
        }

        fn determine_time_in_force(
            &self,
            _signal: &crate::patterns::signals::TradingSignal,
        ) -> crate::brokers::traits::TimeInForce {
            crate::brokers::traits::TimeInForce::Day
        }

        fn calculate_limit_price(
            &self,
            _signal: &crate::patterns::signals::TradingSignal,
            current_price: Price,
        ) -> Option<Price> {
            Some(current_price)
        }

        fn name(&self) -> &str {
            "permissive-test"
        }
    }

    #[tokio::test]
    async fn test_run_replays_candles_and_samples_equity_curve() {
        let mut registry = PatternRegistry::new();
        registry
            .register("hammer", Box::new(crate::patterns::HammerDetector::new()))
            .unwrap();
        registry
            .register(
                "shooting_star",
                Box::new(crate::patterns::ShootingStarDetector::new()),
            )
            .unwrap();

        let mut config = BacktestConfig::new(test_risk_config(), &["hammer", "shooting_star"]);
        // Single-pattern signals compose to moderate confidence; drop the
        // auto threshold so the pipeline trades in this fixture
        config.execution_config.min_auto_confidence = Decimal::new(1, 2); // 0.01
        config.execution_config.manual_confirm_below = Decimal::ZERO;

        let engine = BacktestEngine::new(
            config.clone(),
            Arc::new(registry),
            Arc::new(PermissiveStrategy),
        );

        // A morning of in-session 1-minute bars (Wed 2024-06-12, 14:30 UTC =
        // 10:30 ET): downtrend, a textbook hammer (long lower shadow, small
        // body at the top), a rally, then a shooting star (long upper
        // shadow) to close the position
        let symbol = Symbol::new("AAPL").unwrap();
        let start = Utc.with_ymd_and_hms(2024, 6, 12, 14, 30, 0).unwrap();
        let ts = |i: i64| start + chrono::Duration::minutes(i);
        let mut candles = Vec::new();

        // Downtrend context
        let mut price = 15_000i64; // cents
        for i in 0..8 {
            let open = price;
            price -= 40;
            candles.push(candle(
                &symbol,
                ts(i),
                open,
                open + 10,
                price - 15,
                price,
                10_000,
            ));
        }
        // Hammer: body 40c (28% of range), lower shadow 100c (2.5x body),
        // upper shadow 5c
        candles.push(candle(
            &symbol,
            ts(8),
            14_640,
            14_685,
            14_540,
            14_680,
            30_000,
        ));
        // Rally
        let mut price = 14_700i64;
        for i in 9..16 {
            let open = price;
            price += 60;
            candles.push(candle(
                &symbol,
                ts(i),
                open,
                price + 15,
                open - 10,
                price,
                12_000,
            ));
        }
        // Shooting star: body 40c (28% of range), upper shadow 100c (2.5x
        // body), lower shadow 5c
        candles.push(candle(
            &symbol,
            ts(16),
            15_120,
            15_220,
            15_075,
            15_080,
            30_000,
        ));
        // A few settle bars so the sell order fills and marks update
        let mut price = 15_100i64;
        for i in 17..22 {
            let open = price;
            price -= 10;
            candles.push(candle(
                &symbol,
                ts(i),
                open,
                open + 15,
                price - 15,
                price,
                9_000,
            ));
        }

        let total = candles.len();
        let report = engine.run(candles).await.unwrap();

        assert_eq!(report.candles_processed, total);
        assert_eq!(report.equity_curve.len(), total);
        assert!(
            report.patterns_detected >= 2,
            "hammer and shooting star must both fire (got {})",
            report.patterns_detected
        );
        assert!(report.signals_generated >= 2);
        assert!(
            report.orders_placed >= 2,
            "buy and sell orders must be placed (got {})",
            report.orders_placed
        );
        assert!(report.orders_filled >= 2);
        assert!(
            report.round_trips >= 1,
            "the shooting-star sell must close the hammer entry"
        );
        assert!(
            report
                .per_pattern
                .iter()
                .any(|p| p.pattern_name == "hammer"),
            "round trip must be attributed to the opening pattern"
        );
        // The rally between entry (~147) and exit (~151) makes this a winner
        assert!(report.gross_realized_pnl > Decimal::ZERO);

        // Every equity sample starts from initial equity; the first sample
        // (before any trade can complete) must equal it exactly
        assert_eq!(report.equity_curve[0].1, config.initial_equity);
        // Accounting identity holds regardless of how many trades fired
        assert_eq!(
            report.ending_equity,
            config.initial_equity + report.net_realized_pnl + report.open_position_pnl
        );
        assert_eq!(report.round_trips, report.wins + report.losses);
    }
}
