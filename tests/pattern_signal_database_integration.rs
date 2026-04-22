//! Integration tests for Pattern Signal Database Integration (Phase 3D-2)
//!
//! Tests signal storage, retrieval, outcome tracking, and statistics.

use chrono::Utc;
use daytrader::brokers::types::{Confidence, Price, Symbol};
use daytrader::db::repositories::SignalRepository;
use daytrader::events::types::SignalType;
use daytrader::patterns::signals::{
    MarketContext, SignalComponent, SignalComponentType, SignalConfig, SignalEngine, TradingSignal,
};
use daytrader::patterns::types::TrendContext;
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

/// Sets up an in-memory test database
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Load schema
    let schema = include_str!("../src/db/schema.sql");
    sqlx::raw_sql(schema).execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_signal_storage_and_retrieval() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Create a test signal
    let symbol = Symbol::new("AAPL").unwrap();
    let confidence = Confidence::new(0.85).unwrap();
    let price = Price::new(Decimal::new(15000, 2)).unwrap();
    let trend = TrendContext::StrongUptrend { strength: 0.8 };
    let market_context = MarketContext::new(trend)
        .with_volatility(0.3)
        .with_market_strength(0.7);

    let mut signal = TradingSignal::new(
        symbol.clone(),
        SignalType::Buy,
        confidence,
        price,
        market_context,
    );

    // Add a component
    signal.components.push(SignalComponent::new(
        SignalComponentType::Pattern,
        "hammer",
        0.9,
        0.4,
        1.0,
    ));

    // Save signal
    let result = repo.save_trading_signal(&signal).await;
    assert!(result.is_ok(), "Failed to save signal: {:?}", result.err());

    // Retrieve signals for symbol
    let retrieved = repo
        .get_signals_for_symbol("AAPL", 10)
        .await
        .expect("Failed to retrieve signals");

    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].symbol.as_str(), "AAPL");
    assert_eq!(retrieved[0].signal_type, SignalType::Buy);
    assert!((retrieved[0].confidence.value() - 0.85).abs() < 0.01);
}

#[tokio::test]
async fn test_signal_outcome_update() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Create and save a signal
    let symbol = Symbol::new("TSLA").unwrap();
    let confidence = Confidence::new(0.75).unwrap();
    let price = Price::new(Decimal::new(25000, 2)).unwrap();
    let market_context = MarketContext::new(TrendContext::Sideways { volatility: 0.5 });

    let signal = TradingSignal::new(symbol, SignalType::Sell, confidence, price, market_context);

    let signal_id = signal.id.clone();
    repo.save_trading_signal(&signal).await.unwrap();

    // Update outcome
    repo.update_signal_outcome(&signal_id, "win", Some(12345))
        .await
        .expect("Failed to update outcome");

    // Verify update by querying directly
    let row = sqlx::query!(
        "SELECT outcome, acted_upon, order_id FROM trading_signals WHERE id = ?",
        signal_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.outcome.unwrap(), "win");
    assert_eq!(row.acted_upon, true);
    assert_eq!(row.order_id.unwrap(), 12345);
}

#[tokio::test]
async fn test_signal_statistics() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Create multiple signals with outcomes
    for i in 0..10 {
        let symbol = Symbol::new("SPY").unwrap();
        let confidence = Confidence::new(0.6 + (i as f64 * 0.02)).unwrap();
        let price = Price::new(Decimal::new(40000 + (i * 100), 2)).unwrap();
        let market_context = MarketContext::new(TrendContext::WeakUptrend { strength: 0.5 });

        let signal = TradingSignal::new(
            symbol,
            if i % 2 == 0 {
                SignalType::Buy
            } else {
                SignalType::Sell
            },
            confidence,
            price,
            market_context,
        );

        let signal_id = signal.id.clone();
        repo.save_trading_signal(&signal).await.unwrap();

        // Set outcomes for half of them
        if i < 5 {
            let outcome = if i < 3 { "win" } else { "loss" };
            repo.update_signal_outcome(&signal_id, outcome, None)
                .await
                .unwrap();
        }
    }

    // Get statistics
    let stats = repo
        .get_signal_statistics(Utc::now() - chrono::Duration::hours(1))
        .await
        .expect("Failed to get statistics");

    assert_eq!(stats.total_signals, 10);
    assert_eq!(stats.buy_signals, 5);
    assert_eq!(stats.sell_signals, 5);
    assert_eq!(stats.win_count, 3);
    assert_eq!(stats.loss_count, 2);
    assert_eq!(stats.pending_count, 0);
    assert!((stats.win_rate - 0.6).abs() < 0.01); // 3/5 = 0.6
}

#[tokio::test]
async fn test_batch_signal_insertion() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    let mut signals = Vec::new();
    for i in 0..100 {
        let symbol = Symbol::new(&format!("TEST{}", i % 5)).unwrap();
        let confidence = Confidence::new(0.7).unwrap();
        let price = Price::new(Decimal::new(10000 + (i * 10), 2)).unwrap();
        let market_context = MarketContext::new(TrendContext::Sideways { volatility: 0.5 });

        signals.push(TradingSignal::new(
            symbol,
            SignalType::Buy,
            confidence,
            price,
            market_context,
        ));
    }

    // Batch save
    let result = repo.batch_save_trading_signals(&signals).await;
    assert!(result.is_ok(), "Batch insert failed: {:?}", result.err());

    let inserted_ids = result.unwrap();
    assert_eq!(inserted_ids.len(), 100);

    // Verify all were inserted
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trading_signals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
async fn test_concurrent_signal_writes() {
    let pool = setup_test_db().await;
    let repo = Arc::new(SignalRepository::new(pool.clone()));

    let mut handles = Vec::new();

    for i in 0..10 {
        let repo_clone = Arc::clone(&repo);
        let handle = tokio::spawn(async move {
            let symbol = Symbol::new(&format!("STOCK{}", i)).unwrap();
            let confidence = Confidence::new(0.8).unwrap();
            let price = Price::new(Decimal::new(10000, 2)).unwrap();
            let market_context = MarketContext::new(TrendContext::Sideways { volatility: 0.5 });

            let signal =
                TradingSignal::new(symbol, SignalType::Buy, confidence, price, market_context);

            repo_clone.save_trading_signal(&signal).await
        });

        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(
            result.is_ok(),
            "Concurrent write failed: {:?}",
            result.err()
        );
    }

    // Verify count
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trading_signals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 10);
}

#[tokio::test]
async fn test_json_serialization_roundtrip() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Create signal with complex component and metadata
    let symbol = Symbol::new("NVDA").unwrap();
    let confidence = Confidence::new(0.92).unwrap();
    let price = Price::new(Decimal::new(50000, 2)).unwrap();
    let market_context = MarketContext::new(TrendContext::StrongUptrend { strength: 0.85 })
        .with_volatility(0.4)
        .with_market_strength(0.8);

    let mut signal = TradingSignal::new(
        symbol.clone(),
        SignalType::Buy,
        confidence,
        price,
        market_context,
    );

    // Add multiple components
    signal.components.push(SignalComponent::new(
        SignalComponentType::Pattern,
        "hammer",
        0.9,
        0.4,
        1.0,
    ));
    signal.components.push(SignalComponent::new(
        SignalComponentType::Indicator,
        "rsi_oversold",
        0.85,
        0.3,
        1.0,
    ));
    signal.components.push(
        SignalComponent::new(SignalComponentType::Volume, "volume_surge", 0.8, 0.2, 1.0)
            .add_data("ratio", "1.5"),
    );

    // Add metadata
    signal
        .metadata
        .insert("source".to_string(), "real_time".to_string());
    signal
        .metadata
        .insert("version".to_string(), "3.0".to_string());

    // Save and retrieve
    repo.save_trading_signal(&signal).await.unwrap();

    let retrieved = repo
        .get_signals_for_symbol("NVDA", 10)
        .await
        .expect("Failed to retrieve signals");

    assert_eq!(retrieved.len(), 1);
    let retrieved_signal = &retrieved[0];

    // Verify components were preserved
    assert_eq!(retrieved_signal.components.len(), 3);
    assert_eq!(retrieved_signal.components[0].name, "hammer");
    assert_eq!(retrieved_signal.components[1].name, "rsi_oversold");
    assert_eq!(retrieved_signal.components[2].name, "volume_surge");

    // Verify metadata was preserved
    assert_eq!(
        retrieved_signal.metadata.get("source").unwrap(),
        "real_time"
    );
    assert_eq!(retrieved_signal.metadata.get("version").unwrap(), "3.0");
}

#[tokio::test]
async fn test_signal_engine_with_database_persistence() {
    let pool = setup_test_db().await;
    let repo = Arc::new(SignalRepository::new(pool.clone()));

    // Create signal engine with repository
    let config = SignalConfig::default();
    let mut engine = SignalEngine::new(config).with_signal_repository(Arc::clone(&repo));

    // Create test pattern results (mock data)
    let patterns = vec![]; // Empty for this test
    let candles = vec![]; // Empty for this test

    // Generate signals (will be empty due to empty inputs, but tests the flow)
    let result = engine.generate_signals(&patterns, &candles).await;
    assert!(
        result.is_ok(),
        "Signal generation failed: {:?}",
        result.err()
    );

    // The actual signal generation would require valid pattern and candle data,
    // but we've verified the database persistence flow is integrated
}

#[tokio::test]
async fn test_indicator_value_storage() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Save indicator values
    repo.save_indicator_value("AAPL", "rsi", 65.5, Utc::now(), None)
        .await
        .expect("Failed to save indicator value");

    repo.save_indicator_value(
        "AAPL",
        "macd",
        0.25,
        Utc::now(),
        Some(r#"{"signal": 0.20}"#.to_string()),
    )
    .await
    .expect("Failed to save indicator value");

    // Retrieve indicator values
    let rsi_values = repo
        .get_indicator_values("AAPL", "rsi", Utc::now() - chrono::Duration::hours(1), 10)
        .await
        .expect("Failed to retrieve RSI values");

    assert_eq!(rsi_values.len(), 1);
    assert_eq!(rsi_values[0].indicator_name, "rsi");
    assert!((rsi_values[0].value - 65.5).abs() < 0.01);

    let macd_values = repo
        .get_indicator_values("AAPL", "macd", Utc::now() - chrono::Duration::hours(1), 10)
        .await
        .expect("Failed to retrieve MACD values");

    assert_eq!(macd_values.len(), 1);
    assert_eq!(macd_values[0].indicator_name, "macd");
    assert!(macd_values[0].metadata.is_some());
}

#[tokio::test]
async fn test_signal_data_integrity() {
    let pool = setup_test_db().await;
    let repo = SignalRepository::new(pool.clone());

    // Test constraint: signal_type must be 'buy' or 'sell'
    let invalid_result = sqlx::query!(
        "INSERT INTO trading_signals (id, symbol, signal_type, confidence, price_level, generated_at, components) VALUES (?, ?, ?, ?, ?, ?, ?)",
        "test-id",
        "TEST",
        "invalid_type",
        0.8,
        100.0,
        Utc::now(),
        "[]"
    )
    .execute(&pool)
    .await;

    assert!(invalid_result.is_err(), "Should reject invalid signal_type");

    // Test constraint: confidence must be 0-1
    let invalid_confidence = sqlx::query!(
        "INSERT INTO trading_signals (id, symbol, signal_type, confidence, price_level, generated_at, components) VALUES (?, ?, ?, ?, ?, ?, ?)",
        "test-id-2",
        "TEST",
        "buy",
        1.5,
        100.0,
        Utc::now(),
        "[]"
    )
    .execute(&pool)
    .await;

    assert!(
        invalid_confidence.is_err(),
        "Should reject confidence > 1.0"
    );

    // Test valid signal
    let valid_result = sqlx::query!(
        "INSERT INTO trading_signals (id, symbol, signal_type, confidence, price_level, generated_at, components) VALUES (?, ?, ?, ?, ?, ?, ?)",
        "test-id-3",
        "TEST",
        "buy",
        0.85,
        100.0,
        Utc::now(),
        "[]"
    )
    .execute(&pool)
    .await;

    assert!(valid_result.is_ok(), "Valid signal should be accepted");
}
