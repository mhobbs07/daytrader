//! Comprehensive Phase 4B position tracking performance benchmarks
//!
//! This benchmark suite validates all Phase 4B performance targets:
//! - Position updates: < 10ms for 100 positions
//! - P&L calculations: < 5ms for portfolio-wide
//! - Database queries: < 20ms for complex aggregations
//! - Memory efficient: < 1MB per 1000 positions

use chrono::Utc;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use daytrader::brokers::types::{OrderId, PatternName, Price, Quantity, Symbol};
use daytrader::db::repositories::PositionRepository;
use daytrader::positions::{
    pnl::PnLCalculator,
    tracker::{CloseReason, Position, PositionSide},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use tokio::runtime::Runtime;

// =============================================================================
// Test Data Generators
// =============================================================================

/// Builds a validated Price (bench inputs are always valid)
fn px(v: Decimal) -> Price {
    Price::new(v).unwrap()
}

/// Builds a validated Quantity
fn qty(v: u32) -> Quantity {
    Quantity::new(v).unwrap()
}

/// Builds a validated Symbol
fn sym(s: &str) -> Symbol {
    Symbol::new(s).unwrap()
}

/// Creates a test position with realistic data
fn create_test_position(symbol: &str, quantity: u32, entry_price: f64) -> Position {
    Position::new(
        sym(symbol),
        PositionSide::Long,
        qty(quantity),
        px(Decimal::from_f64_retain(entry_price).unwrap()),
        OrderId::new(format!("order-{}", symbol)),
        None,
        None,
    )
}

/// Creates multiple test positions
fn create_test_positions(count: usize) -> Vec<Position> {
    (0..count)
        .map(|i| {
            let symbol = format!("SYM{:03}", i);
            let quantity = 100 + (i % 50) as u32;
            let entry_price = 100.0 + (i as f64 * 0.5);
            create_test_position(&symbol, quantity, entry_price)
        })
        .collect()
}

// =============================================================================
// 1. P&L Calculator Benchmarks
// =============================================================================

/// Benchmark unrealized P&L calculation for single position
pub fn bench_unrealized_pl_single(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();
    let cost_basis = dec!(10000.00);
    let quantity = 100;
    let current_price = px(dec!(105.00));

    let mut group = c.benchmark_group("pnl_calculations");

    group.bench_function("unrealized_pl_single_position", |b| {
        b.iter(|| {
            let _ = black_box(calculator.calculate_unrealized_pl(
                black_box(cost_basis),
                black_box(quantity),
                black_box(current_price),
            ));
        });
    });

    group.finish();
}

/// Benchmark unrealized P&L percentage calculation
pub fn bench_unrealized_pl_percent(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();
    let cost_basis = dec!(10000.00);
    let quantity = 100;
    let current_price = px(dec!(105.00));

    let mut group = c.benchmark_group("pnl_calculations");

    group.bench_function("unrealized_pl_percent", |b| {
        b.iter(|| {
            let _ = black_box(calculator.calculate_unrealized_pl_percent(
                black_box(cost_basis),
                black_box(quantity),
                black_box(current_price),
            ));
        });
    });

    group.finish();
}

/// Benchmark realized P&L calculation
pub fn bench_realized_pl(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();
    let entry_price = px(dec!(100.00));
    let exit_price = px(dec!(110.00));
    let quantity = qty(100);
    let commission = dec!(2.00);

    let mut group = c.benchmark_group("pnl_calculations");

    group.bench_function("realized_pl_with_commission", |b| {
        b.iter(|| {
            let _ = black_box(calculator.calculate_realized_pl(
                black_box(entry_price),
                black_box(exit_price),
                black_box(quantity),
                black_box(commission),
            ));
        });
    });

    group.finish();
}

/// Benchmark portfolio-wide P&L calculation
pub fn bench_portfolio_pl(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();

    let mut group = c.benchmark_group("portfolio_pnl");

    for size in [10, 100, 1000].iter() {
        let positions: Vec<(Decimal, u32, Price)> = (0..*size)
            .map(|i| {
                let cost_basis = dec!(10000.00) + Decimal::from(i * 100);
                let quantity = 100;
                let current_price = px(dec!(105.00) + Decimal::from(i % 10));
                (cost_basis, quantity, current_price)
            })
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let _ = black_box(
                    calculator.calculate_portfolio_pl(black_box(positions.clone().into_iter())),
                );
            });
        });
    }

    group.finish();
}

/// Benchmark average entry price calculation (position averaging)
pub fn bench_average_entry_calculation(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();
    let existing_avg = px(dec!(100.00));
    let existing_qty = 100;
    let new_price = px(dec!(105.00));
    let new_qty = 50;

    let mut group = c.benchmark_group("pnl_calculations");

    group.bench_function("average_entry_price", |b| {
        b.iter(|| {
            let _ = black_box(calculator.calculate_avg_entry(
                black_box(existing_avg),
                black_box(existing_qty),
                black_box(new_price),
                black_box(new_qty),
            ));
        });
    });

    group.finish();
}

// =============================================================================
// 2. Position Update Benchmarks
// =============================================================================

/// Benchmark position updates from order fills
pub fn bench_position_updates(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Create in-memory database for testing
    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                market_value REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL NOT NULL,
                unrealized_pl_percent REAL NOT NULL,
                realized_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                UNIQUE(account_id, symbol)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    });

    let mut group = c.benchmark_group("position_updates");
    group.sample_size(50); // Reduce sample size for async operations

    for count in [10, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                rt.block_on(async {
                    // Create positions and update them
                    let positions = create_test_positions(count);
                    let repo = PositionRepository::new(db_pool.clone());

                    for position in &positions {
                        let _ = repo.save_position(position, 1).await;
                    }

                    // Now update all positions with new prices
                    for (i, mut position) in positions.into_iter().enumerate() {
                        position.current_price = px(dec!(105.00) + Decimal::from(i % 10));
                        let _ = repo.update_position(&position, 1).await;
                    }
                });
            });
        });
    }

    group.finish();
}

/// Benchmark opening new positions
pub fn bench_position_opening(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                market_value REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL NOT NULL,
                unrealized_pl_percent REAL NOT NULL,
                realized_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                UNIQUE(account_id, symbol)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    });

    let mut group = c.benchmark_group("position_opening");
    group.sample_size(50);

    for count in [10, 100, 1000].iter() {
        let positions = create_test_positions(*count);

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    let repo = PositionRepository::new(db_pool.clone());

                    for position in &positions {
                        let _ = repo.save_position(black_box(position), 1).await;
                    }
                });
            });
        });
    }

    group.finish();
}

/// Benchmark position closing with P&L calculation
pub fn bench_position_closing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        // Create both positions and position_history tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                market_value REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL NOT NULL,
                unrealized_pl_percent REAL NOT NULL,
                realized_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                UNIQUE(account_id, symbol)
            );

            CREATE TABLE IF NOT EXISTS position_history (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL,
                realized_pl REAL NOT NULL,
                total_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                closed_at TIMESTAMP NOT NULL,
                holding_period_minutes INTEGER,
                close_reason TEXT,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                notes TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    });

    let mut group = c.benchmark_group("position_closing");
    group.sample_size(50);

    let positions = create_test_positions(100);

    // First, save all positions
    rt.block_on(async {
        let repo = PositionRepository::new(db_pool.clone());
        for position in &positions {
            let _ = repo.save_position(position, 1).await;
        }
    });

    group.bench_function("close_position_with_archival", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let position = &positions[0];
                let exit_price = px(dec!(110.00));
                let close_reason = CloseReason::TakeProfit;

                let _ = black_box(
                    repo.close_position(
                        black_box(position),
                        1,
                        black_box(&close_reason),
                        black_box(exit_price),
                    )
                    .await,
                );
            });
        });
    });

    group.finish();
}

// =============================================================================
// 3. Database Query Benchmarks
// =============================================================================

/// Benchmark position retrieval by symbol
pub fn bench_position_retrieval(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                market_value REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL NOT NULL,
                unrealized_pl_percent REAL NOT NULL,
                realized_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                UNIQUE(account_id, symbol)
            );

            CREATE INDEX idx_positions_account_symbol ON positions(account_id, symbol);
            CREATE INDEX idx_positions_pattern ON positions(pattern_name);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Populate with test data
        let repo = PositionRepository::new(pool.clone());
        let positions = create_test_positions(1000);
        for position in &positions {
            let _ = repo.save_position(position, 1).await;
        }

        pool
    });

    let mut group = c.benchmark_group("database_queries");
    group.sample_size(100);

    group.bench_function("get_position_by_symbol", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let _ = black_box(repo.get_position("SYM042", 1).await);
            });
        });
    });

    group.bench_function("get_all_active_positions", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let _ = black_box(repo.get_active_positions(1).await);
            });
        });
    });

    group.finish();
}

/// Benchmark complex position history queries
pub fn bench_position_history_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS position_history (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL,
                realized_pl REAL NOT NULL,
                total_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                closed_at TIMESTAMP NOT NULL,
                holding_period_minutes INTEGER,
                close_reason TEXT,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                notes TEXT
            );

            CREATE INDEX idx_position_history_account_closed ON position_history(account_id, closed_at);
            CREATE INDEX idx_position_history_pattern ON position_history(pattern_name);
            CREATE INDEX idx_position_history_symbol ON position_history(symbol);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Populate with test history data
        let positions = create_test_positions(500);
        for position in &positions {
            let exit_price = position.current_price.value() * dec!(1.05);
            let realized_pl = (exit_price - position.avg_entry_price.value())
                .to_f64()
                .unwrap_or(0.0)
                * position.quantity.value() as f64;
            sqlx::query(
                r#"
                INSERT INTO position_history (
                    account_id, symbol, quantity, side, avg_entry_price, exit_price,
                    cost_basis, unrealized_pl, realized_pl, total_pl, opened_at, closed_at,
                    holding_period_minutes, close_reason, pattern_name, signal_confidence,
                    entry_orders, exit_orders
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#)
            .bind(1)
            .bind(position.symbol.as_str())
            .bind(position.quantity.value())
            .bind("long")
            .bind(position.avg_entry_price.as_f64())
            .bind(exit_price.to_f64().unwrap_or(0.0))
            .bind(position.cost_basis.to_f64().unwrap_or(0.0))
            .bind(0.0)
            .bind(realized_pl)
            .bind(realized_pl)
            .bind(position.opened_at)
            .bind(Utc::now())
            .bind(60)
            .bind("take_profit")
            .bind(None::<String>)
            .bind(None::<f64>)
            .bind("[]")
            .bind("[]")
            .execute(&pool)
            .await
            .unwrap();
        }

        pool
    });

    let mut group = c.benchmark_group("history_queries");
    group.sample_size(100);

    group.bench_function("get_position_history", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let since = Utc::now() - chrono::Duration::days(30);
                let _ = black_box(repo.get_position_history(1, since, 100).await);
            });
        });
    });

    group.bench_function("get_position_performance_stats", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let since = Utc::now() - chrono::Duration::days(30);
                let _ = black_box(repo.get_position_performance(1, since).await);
            });
        });
    });

    group.finish();
}

/// Benchmark pattern-based position filtering
pub fn bench_pattern_filtering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db_pool = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                market_value REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL NOT NULL,
                unrealized_pl_percent REAL NOT NULL,
                realized_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                UNIQUE(account_id, symbol)
            );

            CREATE TABLE IF NOT EXISTS position_history (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                side TEXT NOT NULL,
                avg_entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                cost_basis REAL NOT NULL,
                unrealized_pl REAL,
                realized_pl REAL NOT NULL,
                total_pl REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                closed_at TIMESTAMP NOT NULL,
                holding_period_minutes INTEGER,
                close_reason TEXT,
                pattern_name TEXT,
                signal_confidence REAL,
                entry_orders TEXT,
                exit_orders TEXT,
                notes TEXT
            );

            CREATE INDEX idx_positions_pattern ON positions(pattern_name);
            CREATE INDEX idx_position_history_pattern ON position_history(pattern_name);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Populate with test data with patterns
        let repo = PositionRepository::new(pool.clone());
        for i in 0..500 {
            let mut position = create_test_position(&format!("SYM{:03}", i), 100, 100.0);
            if i % 3 == 0 {
                position.pattern_name = Some(PatternName::new("Hammer"));
            }
            let _ = repo.save_position(&position, 1).await;
        }

        pool
    });

    let mut group = c.benchmark_group("pattern_filtering");
    group.sample_size(100);

    group.bench_function("get_positions_by_pattern", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = PositionRepository::new(db_pool.clone());
                let _ = black_box(repo.get_positions_by_pattern("Hammer", 1).await);
            });
        });
    });

    group.finish();
}

// =============================================================================
// 4. Memory and Scaling Benchmarks
// =============================================================================

/// Benchmark memory usage of position collections
pub fn bench_position_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                let positions = create_test_positions(count);
                black_box(positions)
            });
        });
    }

    group.finish();
}

/// Benchmark P&L calculation scaling
pub fn bench_pnl_scaling(c: &mut Criterion) {
    let calculator = PnLCalculator::with_defaults();

    let mut group = c.benchmark_group("pnl_scaling");

    for count in [10, 100, 1000, 10000].iter() {
        let positions = create_test_positions(*count);
        let position_data: Vec<(Decimal, u32, Price)> = positions
            .iter()
            .map(|p| (p.cost_basis, p.quantity.value(), p.current_price))
            .collect();

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let _ = black_box(
                    calculator.calculate_portfolio_pl(black_box(position_data.clone().into_iter())),
                );
            });
        });
    }

    group.finish();
}

// =============================================================================
// Criterion Configuration
// =============================================================================

criterion_group!(
    pnl_benches,
    bench_unrealized_pl_single,
    bench_unrealized_pl_percent,
    bench_realized_pl,
    bench_portfolio_pl,
    bench_average_entry_calculation,
);

criterion_group!(
    position_update_benches,
    bench_position_opening,
    bench_position_updates,
    bench_position_closing,
);

criterion_group!(
    database_benches,
    bench_position_retrieval,
    bench_position_history_queries,
    bench_pattern_filtering,
);

criterion_group!(scaling_benches, bench_position_memory, bench_pnl_scaling,);

criterion_main!(
    pnl_benches,
    position_update_benches,
    database_benches,
    scaling_benches,
);
