//! Database integration tests for risk management persistence
//!
//! These tests validate database operations for risk state management:
//! - Risk state persistence and loading
//! - Concurrent database access under load
//! - Data integrity and referential integrity
//! - Database schema migration testing

use daytrader::risk::types::{AccountId, RiskRuleId, ViolationSeverity};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;

/// Test risk state persistence in database
#[tokio::test]
async fn test_risk_state_persistence() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    // Create database connection
    let conn = Connection::open(db_path).unwrap();

    // Create risk state table
    conn.execute(
        "CREATE TABLE risk_states (
            account_id TEXT PRIMARY KEY,
            daily_pl REAL NOT NULL,
            day_trade_count INTEGER NOT NULL,
            is_trading_halted INTEGER NOT NULL,
            last_updated TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    let account_id = AccountId::new("test-account");

    // Insert initial risk state
    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl, day_trade_count, is_trading_halted, last_updated)
         VALUES (?, ?, ?, ?, ?)",
        (&account_id.to_string(), 0.0, 0, 0, "2023-12-15T09:30:00Z"),
    ).unwrap();

    // Verify insertion
    let mut stmt = conn
        .prepare("SELECT * FROM risk_states WHERE account_id = ?")
        .unwrap();
    let mut rows = stmt
        .query_map([&account_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .unwrap();

    let row = rows.next().unwrap().unwrap();
    assert_eq!(row.0, account_id.to_string());
    assert_eq!(row.1, 0.0);
    assert_eq!(row.2, 0);
    assert_eq!(row.3, 0);

    // Update risk state
    conn.execute(
        "UPDATE risk_states SET daily_pl = ?, day_trade_count = ?, is_trading_halted = ?, last_updated = ?
         WHERE account_id = ?",
        (500.0, 2, 1, "2023-12-15T16:00:00Z", &account_id.to_string()),
    ).unwrap();

    // Verify update
    let mut stmt = conn.prepare("SELECT daily_pl, day_trade_count, is_trading_halted FROM risk_states WHERE account_id = ?").unwrap();
    let mut rows = stmt
        .query_map([&account_id.to_string()], |row| {
            Ok((
                row.get::<_, f64>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })
        .unwrap();

    let row = rows.next().unwrap().unwrap();
    assert_eq!(row.0, 500.0);
    assert_eq!(row.1, 2);
    assert_eq!(row.2, 1);
}

/// Test concurrent database access under load
#[tokio::test]
async fn test_concurrent_database_access() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    // Create database with WAL mode for better concurrency
    let conn = Connection::open(db_path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();

    // Create risk violations table
    conn.execute(
        "CREATE TABLE risk_violations (
            id INTEGER PRIMARY KEY,
            rule_id TEXT NOT NULL,
            severity TEXT NOT NULL,
            message TEXT NOT NULL,
            timestamp TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    let conn_arc = Arc::new(Mutex::new(conn));

    // Spawn multiple tasks to simulate concurrent database writes
    let mut handles = vec![];

    for i in 0..10 {
        let conn_clone = Arc::clone(&conn_arc);
        let handle = tokio::spawn(async move {
            let conn = conn_clone.lock().unwrap();
            for j in 0..5 {
                conn.execute(
                    "INSERT INTO risk_violations (rule_id, severity, message, timestamp)
                     VALUES (?, ?, ?, ?)",
                    (
                        format!("rule-{}-{}", i, j),
                        "critical",
                        format!("Violation message {}-{}", i, j),
                        "2023-12-15T10:00:00Z",
                    ),
                )
                .unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all records were inserted
    let conn = conn_arc.lock().unwrap();
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM risk_violations", [], |row| row.get(0))
        .unwrap();

    assert_eq!(count, 50, "All concurrent inserts should succeed");
}

/// Test data integrity and referential integrity
#[tokio::test]
async fn test_data_integrity() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();

    // Create tables with foreign key relationships
    conn.execute(
        "CREATE TABLE accounts (
            id TEXT PRIMARY KEY,
            balance REAL NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE risk_states (
            account_id TEXT PRIMARY KEY REFERENCES accounts(id),
            daily_pl REAL NOT NULL
        )",
        [],
    )
    .unwrap();

    // Insert account first
    conn.execute(
        "INSERT INTO accounts (id, balance) VALUES (?, ?)",
        ("account-1", 10000.0),
    )
    .unwrap();

    // Insert risk state - should succeed
    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-1", 0.0),
    )
    .unwrap();

    // Try to insert risk state for non-existent account - should fail
    let result = conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("non-existent-account", 0.0),
    );

    assert!(result.is_err(), "Should reject foreign key violation");

    // Verify valid data exists
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM risk_states WHERE account_id = ?",
            ["account-1"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 1);
}

/// Test database schema migration handling
#[tokio::test]
async fn test_database_schema_migration() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();

    // Create initial schema (version 1)
    conn.execute(
        "CREATE TABLE risk_states (
            account_id TEXT PRIMARY KEY,
            daily_pl REAL NOT NULL
        )",
        [],
    )
    .unwrap();

    // Insert some data
    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-1", 100.0),
    )
    .unwrap();

    // Simulate migration: add new column
    conn.execute(
        "ALTER TABLE risk_states ADD COLUMN day_trade_count INTEGER DEFAULT 0",
        [],
    )
    .unwrap();

    // Verify migration worked and old data is preserved
    let mut stmt = conn
        .prepare("SELECT account_id, daily_pl, day_trade_count FROM risk_states")
        .unwrap();
    let mut rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })
        .unwrap();

    let row = rows.next().unwrap().unwrap();
    assert_eq!(row.0, "account-1");
    assert_eq!(row.1, 100.0);
    assert_eq!(row.2, 0); // Default value

    // Test inserting with new schema
    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl, day_trade_count) VALUES (?, ?, ?)",
        ("account-2", 200.0, 5),
    )
    .unwrap();

    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM risk_states WHERE day_trade_count = ?",
            [5],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 1);
}

/// Test database transaction handling
#[tokio::test]
async fn test_database_transaction_handling() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();

    // Create table
    conn.execute(
        "CREATE TABLE risk_states (
            account_id TEXT PRIMARY KEY,
            daily_pl REAL NOT NULL
        )",
        [],
    )
    .unwrap();

    // Test successful transaction
    let tx = conn.unchecked_transaction().unwrap();
    tx.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-1", 100.0),
    )
    .unwrap();
    tx.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-2", 200.0),
    )
    .unwrap();
    tx.commit().unwrap();

    // Verify both inserts succeeded
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM risk_states", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);

    // Test failed transaction (simulate rollback)
    let tx = conn.unchecked_transaction().unwrap();
    tx.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-3", 300.0),
    )
    .unwrap();
    // Don't commit - transaction should rollback

    // Verify transaction was rolled back
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM risk_states", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2, "Transaction should have rolled back");
}

/// Test database connection pooling behavior
#[tokio::test]
async fn test_database_connection_pooling() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    // Simulate connection pool by creating multiple connections
    let mut connections = vec![];

    for _ in 0..5 {
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS test_table (
                id INTEGER PRIMARY KEY,
                value TEXT
            )",
            [],
        )
        .unwrap();
        connections.push(conn);
    }

    // All connections should be able to operate independently
    for (i, conn) in connections.iter().enumerate() {
        conn.execute(
            "INSERT INTO test_table (value) VALUES (?)",
            [format!("value-{}", i)],
        )
        .unwrap();
    }

    // Verify all inserts worked
    let conn = &connections[0];
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM test_table", [], |row| row.get(0))
        .unwrap();

    assert_eq!(count, 5);
}

/// Test database backup and recovery scenarios
#[tokio::test]
async fn test_database_backup_recovery() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();

    // Create and populate database
    conn.execute(
        "CREATE TABLE risk_states (
            account_id TEXT PRIMARY KEY,
            daily_pl REAL NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-1", 1000.0),
    )
    .unwrap();

    // Simulate backup by copying file
    let backup_path = format!("{}.backup", db_path);
    std::fs::copy(db_path, &backup_path).unwrap();

    // Add more data
    conn.execute(
        "INSERT INTO risk_states (account_id, daily_pl) VALUES (?, ?)",
        ("account-2", 2000.0),
    )
    .unwrap();

    // Simulate recovery by restoring from backup
    std::fs::copy(&backup_path, db_path).unwrap();

    // Verify only backup data exists
    let conn_restored = Connection::open(db_path).unwrap();
    let count: i32 = conn_restored
        .query_row("SELECT COUNT(*) FROM risk_states", [], |row| row.get(0))
        .unwrap();

    assert_eq!(count, 1, "Should only have backup data after recovery");

    // Clean up
    std::fs::remove_file(backup_path).unwrap();
}
