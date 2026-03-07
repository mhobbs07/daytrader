//! Integration tests for Position Workflows (Phase 4B)
//!
//! Tests the end-to-end position tracking workflows:
//! 1. Position creation from order fills
//! 2. Position updates (quantity, prices, P&L)
//! 3. Position closure with realized P&L
//! 4. Database persistence and retrieval
//! 5. Pattern integration
//! 6. Multi-position scenarios

use daytrader::brokers::types::{OrderId, PatternName, Price, Quantity, Symbol};
use daytrader::brokers::traits::OrderSide;
use daytrader::db::repositories::PositionRepository;
use daytrader::orders::types::{FillInfo, ManagedOrder, OrderRequest};
use daytrader::positions::pnl::{PnLCalculator, PnLConfig, TaxLotMethod};
use daytrader::positions::tracker::{CloseReason, Position, PositionSide, PositionTracker};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::SqlitePool;
use std::sync::Arc;

// ============================================================================
// Test Utilities
// ============================================================================

/// Creates a test database pool for integration tests
async fn create_test_db() -> SqlitePool {
    // Use in-memory SQLite database for testing
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    pool
}

/// Helper to create a test fill info
fn create_fill_info(quantity: u32, price: Decimal, commission: Decimal) -> FillInfo {
    FillInfo {
        filled_quantity: Quantity::new(quantity),
        filled_price: Price::new(price),
        commission,
        timestamp: Utc::now(),
    }
}

/// Helper to create a test order
fn create_test_order(
    symbol: &str,
    side: OrderSide,
    quantity: u32,
    pattern_name: Option<&str>,
) -> ManagedOrder {
    let request = OrderRequest::builder()
        .symbol(Symbol::new(symbol))
        .side(side)
        .order_type(daytrader::brokers::traits::OrderType::Market)
        .quantity(Quantity::new(quantity))
        .time_in_force(daytrader::brokers::traits::TimeInForce::Day)
        .build()
        .unwrap();
    
    let mut order = ManagedOrder::from_request(
        OrderId::new(format!("test-order-{}", symbol)),
        request,
    );
    
    if let Some(pattern) = pattern_name {
        order.pattern_name = Some(PatternName::new(pattern));
        order.signal_confidence = Some(dec!(0.85));
        order.is_pattern_trade = true;
    }
    
    order
}

// ============================================================================
// Position Lifecycle Tests
// ============================================================================

#[tokio::test]
async fn test_open_long_position_from_buy_order() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create a position directly (simulating what tracker would do)
    let position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    // Save to database
    let result = repo.save_position(&position, 1).await;
    assert!(result.is_ok(), "Failed to save position: {:?}", result.err());
    
    // Retrieve from database
    let retrieved = repo.get_position("AAPL", 1).await.unwrap();
    assert!(retrieved.is_some(), "Position not found");
    
    let pos_row = retrieved.unwrap();
    assert_eq!(pos_row.symbol, "AAPL");
    assert_eq!(pos_row.quantity, 100);
    assert_eq!(pos_row.side, "long");
    assert_eq!(pos_row.avg_entry_price, 150.00);
    
    pool.close().await;
}

#[tokio::test]
async fn test_position_quantity_and_average_price_update() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    let calculator = PnLCalculator::with_defaults();
    
    // Initial position: 100 shares @ $150
    let mut position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Add 50 more shares @ $155
    let new_avg = calculator.calculate_avg_entry(
        position.avg_entry_price,
        position.quantity.value(),
        Price::new(dec!(155.00)),
        50,
    );
    
    position.quantity = Quantity::new(150);
    position.avg_entry_price = new_avg;
    position.cost_basis = new_avg.value() * Decimal::from(150);
    position.entry_orders.push(OrderId::new("order-2"));
    position.updated_at = Utc::now();
    
    // Expected average: (100 * 150 + 50 * 155) / 150 = (15000 + 7750) / 150 = 151.67
    assert!(new_avg.value() > dec!(151.66) && new_avg.value() < dec!(151.67));
    
    // Update in database
    repo.update_position(&position, 1).await.unwrap();
    
    // Verify update
    let updated = repo.get_position("AAPL", 1).await.unwrap().unwrap();
    assert_eq!(updated.quantity, 150);
    assert!(updated.avg_entry_price > 151.66 && updated.avg_entry_price < 151.67);
    
    pool.close().await;
}

#[tokio::test]
async fn test_close_position_with_realized_pnl() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    let calculator = PnLCalculator::with_defaults();
    
    // Create position: 100 shares @ $150
    let mut position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Close at $160 (profit of $10/share = $1000 total)
    let exit_price = Price::new(dec!(160.00));
    let realized_pl = calculator.calculate_realized_pl(
        position.avg_entry_price,
        exit_price,
        position.quantity,
        dec!(0.00), // no commission
    );
    
    position.realized_pl = realized_pl;
    position.exit_orders.push(OrderId::new("order-2"));
    
    // Close position (archives to history and removes from active)
    let history_id = repo.close_position(&position, 1, &CloseReason::Manual, exit_price)
        .await
        .unwrap();
    
    assert!(history_id > 0);
    
    // Verify position is no longer active
    let active = repo.get_position("AAPL", 1).await.unwrap();
    assert!(active.is_none(), "Position should be removed from active positions");
    
    // Verify position is in history
    let history = repo.get_position_history(1, Utc::now() - Duration::hours(1), 10)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    
    let closed_pos = &history[0];
    assert_eq!(closed_pos.symbol, "AAPL");
    assert_eq!(closed_pos.quantity, 100);
    assert_eq!(closed_pos.exit_price, 160.00);
    assert_eq!(closed_pos.realized_pl, 1000.00);
    assert_eq!(closed_pos.close_reason, Some("manual".to_string()));
    
    pool.close().await;
}

// ============================================================================
// P&L Calculation Tests
// ============================================================================

#[test]
fn test_unrealized_pnl_profit() {
    let calculator = PnLCalculator::with_defaults();
    
    // Position: 100 shares @ $150, current price $155
    let cost_basis = dec!(15000.00); // 100 * 150
    let quantity = 100;
    let current_price = Price::new(dec!(155.00));
    
    let unrealized_pl = calculator.calculate_unrealized_pl(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl, dec!(500.00)); // (155 - 150) * 100
    
    let unrealized_pl_pct = calculator.calculate_unrealized_pl_percent(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl_pct, dec!(3.333333333333333333333333333)); // (500 / 15000) * 100
}

#[test]
fn test_unrealized_pnl_loss() {
    let calculator = PnLCalculator::with_defaults();
    
    // Position: 100 shares @ $150, current price $145
    let cost_basis = dec!(15000.00);
    let quantity = 100;
    let current_price = Price::new(dec!(145.00));
    
    let unrealized_pl = calculator.calculate_unrealized_pl(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl, dec!(-500.00)); // (145 - 150) * 100
    
    let unrealized_pl_pct = calculator.calculate_unrealized_pl_percent(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl_pct, dec!(-3.333333333333333333333333333)); // (-500 / 15000) * 100
}

#[test]
fn test_realized_pnl_with_commission() {
    let calculator = PnLCalculator::new(PnLConfig {
        include_commissions: true,
        tax_lot_method: TaxLotMethod::FIFO,
    });
    
    let entry_price = Price::new(dec!(150.00));
    let exit_price = Price::new(dec!(160.00));
    let quantity = Quantity::new(100);
    let commission = dec!(2.00);
    
    let realized_pl = calculator.calculate_realized_pl(entry_price, exit_price, quantity, commission);
    // (160 - 150) * 100 - 2 = 1000 - 2 = 998
    assert_eq!(realized_pl, dec!(998.00));
}

#[test]
fn test_realized_pnl_without_commission() {
    let calculator = PnLCalculator::new(PnLConfig {
        include_commissions: false,
        tax_lot_method: TaxLotMethod::FIFO,
    });
    
    let entry_price = Price::new(dec!(150.00));
    let exit_price = Price::new(dec!(160.00));
    let quantity = Quantity::new(100);
    let commission = dec!(2.00);
    
    let realized_pl = calculator.calculate_realized_pl(entry_price, exit_price, quantity, commission);
    // (160 - 150) * 100 = 1000 (commission ignored)
    assert_eq!(realized_pl, dec!(1000.00));
}

#[test]
fn test_portfolio_pnl_aggregation() {
    let calculator = PnLCalculator::with_defaults();
    
    let positions = vec![
        (dec!(10000.00), 100, Price::new(dec!(105.00))), // +500 unrealized
        (dec!(20000.00), 200, Price::new(dec!(95.00))),  // -1000 unrealized
        (dec!(15000.00), 150, Price::new(dec!(102.00))), // +300 unrealized
    ];
    
    let portfolio_pl = calculator.calculate_portfolio_pl(positions.into_iter());
    
    assert_eq!(portfolio_pl.total_cost, dec!(45000.00));
    assert_eq!(portfolio_pl.total_value, dec!(44700.00)); // 10500 + 19000 + 15300
    assert_eq!(portfolio_pl.unrealized_pl, dec!(-300.00));
    assert_eq!(portfolio_pl.position_count, 3);
}

// ============================================================================
// Pattern Integration Tests
// ============================================================================

#[tokio::test]
async fn test_position_with_pattern_metadata() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create position from pattern signal
    let position = Position::new(
        Symbol::new("TSLA"),
        PositionSide::Long,
        Quantity::new(50),
        Price::new(dec!(250.00)),
        OrderId::new("order-1"),
        Some(PatternName::new("Hammer")),
        Some(dec!(0.85)),
    );
    
    assert!(position.is_pattern_trade());
    assert_eq!(position.pattern_name.as_ref().unwrap().as_str(), "Hammer");
    assert_eq!(position.signal_confidence.unwrap(), dec!(0.85));
    
    // Save and retrieve
    repo.save_position(&position, 1).await.unwrap();
    
    let retrieved = repo.get_position("TSLA", 1).await.unwrap().unwrap();
    assert_eq!(retrieved.pattern_name, Some("Hammer".to_string()));
    assert_eq!(retrieved.signal_confidence, Some(0.85));
    
    pool.close().await;
}

#[tokio::test]
async fn test_get_positions_by_pattern() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create multiple positions with same pattern
    let symbols = vec!["AAPL", "GOOGL", "MSFT"];
    for symbol in &symbols {
        let position = Position::new(
            Symbol::new(symbol),
            PositionSide::Long,
            Quantity::new(100),
            Price::new(dec!(150.00)),
            OrderId::new(format!("order-{}", symbol)),
            Some(PatternName::new("Hammer")),
            Some(dec!(0.80)),
        );
        repo.save_position(&position, 1).await.unwrap();
    }
    
    // Query by pattern
    let (active, _historical) = repo.get_positions_by_pattern("Hammer", 1).await.unwrap();
    assert_eq!(active.len(), 3);
    
    // All should be Hammer pattern positions
    for pos in &active {
        assert_eq!(pos.pattern_name, Some("Hammer".to_string()));
    }
    
    pool.close().await;
}

#[tokio::test]
async fn test_pattern_position_history_filtering() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create and close a pattern position
    let position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        Some(PatternName::new("BullishEngulfing")),
        Some(dec!(0.90)),
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Close position
    repo.close_position(
        &position,
        1,
        &CloseReason::TakeProfit,
        Price::new(dec!(160.00)),
    ).await.unwrap();
    
    // Query historical positions by pattern
    let (_active, historical) = repo.get_positions_by_pattern("BullishEngulfing", 1).await.unwrap();
    assert_eq!(historical.len(), 1);
    
    let hist_pos = &historical[0];
    assert_eq!(hist_pos.pattern_name, Some("BullishEngulfing".to_string()));
    assert_eq!(hist_pos.close_reason, Some("take_profit".to_string()));
    
    pool.close().await;
}

// ============================================================================
// Multi-Position Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_positions_per_account() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create positions for multiple symbols
    let symbols = vec!["AAPL", "GOOGL", "TSLA", "MSFT", "AMZN"];
    for (i, symbol) in symbols.iter().enumerate() {
        let position = Position::new(
            Symbol::new(symbol),
            PositionSide::Long,
            Quantity::new((100 + i as u32 * 10)),
            Price::new(dec!(150.00) + Decimal::from(i * 10)),
            OrderId::new(format!("order-{}", symbol)),
            None,
            None,
        );
        repo.save_position(&position, 1).await.unwrap();
    }
    
    // Retrieve all active positions
    let positions = repo.get_active_positions(1).await.unwrap();
    assert_eq!(positions.len(), 5);
    
    // Verify each symbol is present
    let symbols_found: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
    for symbol in &symbols {
        assert!(symbols_found.contains(&symbol.to_string()));
    }
    
    pool.close().await;
}

#[tokio::test]
async fn test_multiple_positions_same_symbol_different_accounts() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create same symbol position for different accounts
    for account_id in 1..=3 {
        let position = Position::new(
            Symbol::new("AAPL"),
            PositionSide::Long,
            Quantity::new(100 * account_id as u32),
            Price::new(dec!(150.00)),
            OrderId::new(format!("order-acc-{}", account_id)),
            None,
            None,
        );
        repo.save_position(&position, account_id).await.unwrap();
    }
    
    // Query by symbol (across all accounts)
    let positions = repo.get_positions_by_symbol("AAPL").await.unwrap();
    assert_eq!(positions.len(), 3);
    
    // Verify quantities
    let mut quantities: Vec<i64> = positions.iter().map(|p| p.quantity).collect();
    quantities.sort();
    assert_eq!(quantities, vec![100, 200, 300]);
    
    pool.close().await;
}

#[tokio::test]
async fn test_position_aggregation_and_filtering() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create mix of pattern and non-pattern positions
    let pattern_symbols = vec!["AAPL", "GOOGL"];
    let non_pattern_symbols = vec!["TSLA", "MSFT"];
    
    for symbol in &pattern_symbols {
        let position = Position::new(
            Symbol::new(symbol),
            PositionSide::Long,
            Quantity::new(100),
            Price::new(dec!(150.00)),
            OrderId::new(format!("order-{}", symbol)),
            Some(PatternName::new("Hammer")),
            Some(dec!(0.80)),
        );
        repo.save_position(&position, 1).await.unwrap();
    }
    
    for symbol in &non_pattern_symbols {
        let position = Position::new(
            Symbol::new(symbol),
            PositionSide::Long,
            Quantity::new(100),
            Price::new(dec!(150.00)),
            OrderId::new(format!("order-{}", symbol)),
            None,
            None,
        );
        repo.save_position(&position, 1).await.unwrap();
    }
    
    // Get all positions
    let all_positions = repo.get_active_positions(1).await.unwrap();
    assert_eq!(all_positions.len(), 4);
    
    // Count pattern positions
    let pattern_count = all_positions.iter()
        .filter(|p| p.pattern_name.is_some())
        .count();
    assert_eq!(pattern_count, 2);
    
    // Query by specific pattern
    let (hammer_positions, _) = repo.get_positions_by_pattern("Hammer", 1).await.unwrap();
    assert_eq!(hammer_positions.len(), 2);
    
    pool.close().await;
}

// ============================================================================
// Database Performance and Analytics Tests
// ============================================================================

#[tokio::test]
async fn test_position_performance_analytics() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create and close multiple positions with varying P&L
    let test_cases = vec![
        ("AAPL", dec!(150.00), dec!(160.00), true),  // +$1000 profit
        ("GOOGL", dec!(100.00), dec!(95.00), false), // -$500 loss
        ("TSLA", dec!(250.00), dec!(260.00), true),  // +$1000 profit
        ("MSFT", dec!(200.00), dec!(190.00), false), // -$1000 loss
        ("AMZN", dec!(180.00), dec!(185.00), true),  // +$500 profit
    ];
    
    for (symbol, entry, exit, _is_win) in test_cases {
        let mut position = Position::new(
            Symbol::new(symbol),
            PositionSide::Long,
            Quantity::new(100),
            Price::new(entry),
            OrderId::new(format!("order-{}", symbol)),
            None,
            None,
        );
        
        // Calculate realized P&L
        let calculator = PnLCalculator::with_defaults();
        let realized_pl = calculator.calculate_realized_pl(
            Price::new(entry),
            Price::new(exit),
            Quantity::new(100),
            dec!(0.00),
        );
        position.realized_pl = realized_pl;
        
        repo.save_position(&position, 1).await.unwrap();
        repo.close_position(&position, 1, &CloseReason::Manual, Price::new(exit))
            .await.unwrap();
    }
    
    // Get performance analytics
    let (winning_count, losing_count, total_pl, avg_pl) = repo
        .get_position_performance(1, Utc::now() - Duration::hours(1))
        .await
        .unwrap();
    
    assert_eq!(winning_count, 3); // AAPL, TSLA, AMZN
    assert_eq!(losing_count, 2);  // GOOGL, MSFT
    assert_eq!(total_pl, 1000.00); // +1000 +1000 +500 -500 -1000 = 1000
    assert_eq!(avg_pl, 200.00);    // 1000 / 5 = 200
    
    pool.close().await;
}

#[tokio::test]
async fn test_position_history_by_symbol() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create and close multiple positions for same symbol
    for i in 1..=3 {
        let mut position = Position::new(
            Symbol::new("AAPL"),
            PositionSide::Long,
            Quantity::new(100),
            Price::new(dec!(150.00) + Decimal::from(i)),
            OrderId::new(format!("order-{}", i)),
            None,
            None,
        );
        
        position.realized_pl = Decimal::from(i * 100);
        
        repo.save_position(&position, 1).await.unwrap();
        repo.close_position(
            &position,
            1,
            &CloseReason::Manual,
            Price::new(dec!(160.00)),
        ).await.unwrap();
    }
    
    // Get history for symbol
    let history = repo.get_position_history_by_symbol("AAPL", 1, 10)
        .await
        .unwrap();
    
    assert_eq!(history.len(), 3);
    
    // Verify all are AAPL
    for pos in &history {
        assert_eq!(pos.symbol, "AAPL");
    }
    
    pool.close().await;
}

#[tokio::test]
async fn test_partial_position_close() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    let calculator = PnLCalculator::with_defaults();
    
    // Create position: 100 shares @ $150
    let mut position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Sell 50 shares @ $160
    let partial_realized = calculator.calculate_realized_pl(
        position.avg_entry_price,
        Price::new(dec!(160.00)),
        Quantity::new(50),
        dec!(0.00),
    );
    
    // Update position: 50 shares remaining
    position.quantity = Quantity::new(50);
    position.realized_pl = partial_realized;
    position.cost_basis = position.avg_entry_price.value() * Decimal::from(50);
    position.exit_orders.push(OrderId::new("order-2"));
    position.updated_at = Utc::now();
    
    repo.update_position(&position, 1).await.unwrap();
    
    // Verify partial position remains active
    let updated = repo.get_position("AAPL", 1).await.unwrap().unwrap();
    assert_eq!(updated.quantity, 50);
    assert_eq!(updated.realized_pl, 500.00); // (160 - 150) * 50
    
    // Close remaining 50 shares @ $165
    let final_realized = calculator.calculate_realized_pl(
        position.avg_entry_price,
        Price::new(dec!(165.00)),
        Quantity::new(50),
        dec!(0.00),
    );
    
    position.quantity = Quantity::new(50);
    position.realized_pl += final_realized;
    
    repo.close_position(&position, 1, &CloseReason::TakeProfit, Price::new(dec!(165.00)))
        .await.unwrap();
    
    // Verify fully closed
    let active = repo.get_position("AAPL", 1).await.unwrap();
    assert!(active.is_none());
    
    // Check history
    let history = repo.get_position_history_by_symbol("AAPL", 1, 10)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    
    // Total realized P&L should be from both partial closes
    // First: (160 - 150) * 50 = 500
    // Second: (165 - 150) * 50 = 750
    // Total: 1250
    assert_eq!(history[0].realized_pl, 1250.00);
    
    pool.close().await;
}

// ============================================================================
// Edge Case and Error Tests
// ============================================================================

#[tokio::test]
async fn test_position_with_zero_unrealized_pl() {
    let calculator = PnLCalculator::with_defaults();
    
    // Position at cost (no P&L)
    let cost_basis = dec!(15000.00);
    let quantity = 100;
    let current_price = Price::new(dec!(150.00)); // Same as entry
    
    let unrealized_pl = calculator.calculate_unrealized_pl(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl, dec!(0.00));
    
    let unrealized_pl_pct = calculator.calculate_unrealized_pl_percent(cost_basis, quantity, current_price);
    assert_eq!(unrealized_pl_pct, dec!(0.00));
}

#[tokio::test]
async fn test_position_delete_without_archiving() {
    let pool = create_test_db().await;
    let repo = PositionRepository::new(pool.clone());
    
    // Create position
    let position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Delete without archiving
    let rows_affected = repo.delete_position("AAPL", 1).await.unwrap();
    assert_eq!(rows_affected, 1);
    
    // Verify deleted
    let result = repo.get_position("AAPL", 1).await.unwrap();
    assert!(result.is_none());
    
    // Verify NOT in history
    let history = repo.get_position_history_by_symbol("AAPL", 1, 10)
        .await
        .unwrap();
    assert_eq!(history.len(), 0);
    
    pool.close().await;
}

#[test]
fn test_position_holding_period() {
    let position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    // Holding period should be >= 0
    let duration = position.holding_period();
    assert!(duration.num_milliseconds() >= 0);
}

#[test]
fn test_close_reason_variants() {
    // Test all close reason variants for Debug impl
    let reasons = vec![
        CloseReason::Manual,
        CloseReason::StopLoss,
        CloseReason::TakeProfit,
        CloseReason::RiskViolation,
        CloseReason::PatternExit,
        CloseReason::EndOfDay,
    ];
    
    for reason in reasons {
        let _ = format!("{:?}", reason);
    }
}

#[test]
fn test_position_is_pattern_trade_detection() {
    // Non-pattern position
    let position1 = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    assert!(!position1.is_pattern_trade());
    
    // Pattern position
    let position2 = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        Some(PatternName::new("Hammer")),
        Some(dec!(0.85)),
    );
    assert!(position2.is_pattern_trade());
}

#[tokio::test]
async fn test_concurrent_position_updates() {
    let pool = create_test_db().await;
    let repo = Arc::new(PositionRepository::new(pool.clone()));
    
    // Create initial position
    let position = Position::new(
        Symbol::new("AAPL"),
        PositionSide::Long,
        Quantity::new(100),
        Price::new(dec!(150.00)),
        OrderId::new("order-1"),
        None,
        None,
    );
    
    repo.save_position(&position, 1).await.unwrap();
    
    // Spawn multiple concurrent updates
    let mut handles = vec![];
    for i in 0..5 {
        let repo_clone = repo.clone();
        let mut pos_clone = position.clone();
        
        let handle = tokio::spawn(async move {
            pos_clone.current_price = Price::new(dec!(150.00) + Decimal::from(i));
            pos_clone.market_value = pos_clone.current_price.value() * Decimal::from(100);
            pos_clone.updated_at = Utc::now();
            
            repo_clone.update_position(&pos_clone, 1).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all updates
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    // Verify position still exists and is valid
    let final_pos = repo.get_position("AAPL", 1).await.unwrap();
    assert!(final_pos.is_some());
    
    pool.close().await;
}
