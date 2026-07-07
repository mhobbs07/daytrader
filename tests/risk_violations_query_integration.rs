//! Integration tests for B-062: querying persisted risk violations
//!
//! Verifies `PerformanceRepository::get_recent_risk_violations` filters by
//! account, orders newest-first, respects the limit, and excludes non-violation
//! risk events (e.g. `RiskWarning`).

use chrono::Utc;
use daytrader::db::repositories::PerformanceRepository;
use sqlx::SqlitePool;

/// Sets up an in-memory test database with the full schema loaded and two
/// seeded accounts (ids 1 and 2) to satisfy the `risk_events.account_id`
/// foreign key.
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    let schema = include_str!("../src/db/schema.sql");
    sqlx::raw_sql(schema).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO accounts (id, broker, account_number, account_type) VALUES \
         (1, 'mock', 'acct-1', 'paper'), (2, 'mock', 'acct-2', 'paper')",
    )
    .execute(&pool)
    .await
    .expect("seed accounts");

    pool
}

/// Inserts a raw risk_events row for test setup.
async fn insert_risk_event(
    pool: &SqlitePool,
    event_type: &str,
    account_id: i64,
    violation_type: Option<&str>,
    severity: &str,
    message: &str,
    timestamp: chrono::DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO risk_events (event_type, timestamp, account_id, pattern_name, violation_type, severity, message, details, created_at) \
         VALUES (?, ?, ?, NULL, ?, ?, ?, NULL, ?)",
    )
    .bind(event_type)
    .bind(timestamp)
    .bind(account_id)
    .bind(violation_type)
    .bind(severity)
    .bind(message)
    .bind(timestamp)
    .execute(pool)
    .await
    .expect("insert risk event");
}

#[tokio::test]
async fn test_get_recent_risk_violations_returns_newest_first_for_account() {
    let pool = setup_test_db().await;
    let repo = PerformanceRepository::new(pool.clone());

    let base = Utc::now();
    insert_risk_event(
        &pool,
        "RiskViolation",
        1,
        Some("daily_loss_limit"),
        "high",
        "oldest violation",
        base - chrono::Duration::minutes(10),
    )
    .await;
    insert_risk_event(
        &pool,
        "RiskViolation",
        1,
        Some("pdt_limit"),
        "critical",
        "newest violation",
        base,
    )
    .await;

    let violations = repo
        .get_recent_risk_violations(1, 10)
        .await
        .expect("query should succeed");

    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].message, "newest violation");
    assert_eq!(violations[1].message, "oldest violation");
}

#[tokio::test]
async fn test_get_recent_risk_violations_excludes_other_accounts() {
    let pool = setup_test_db().await;
    let repo = PerformanceRepository::new(pool.clone());

    let now = Utc::now();
    insert_risk_event(
        &pool,
        "RiskViolation",
        1,
        Some("daily_loss_limit"),
        "high",
        "account 1 violation",
        now,
    )
    .await;
    insert_risk_event(
        &pool,
        "RiskViolation",
        2,
        Some("daily_loss_limit"),
        "high",
        "account 2 violation",
        now,
    )
    .await;

    let violations = repo
        .get_recent_risk_violations(1, 10)
        .await
        .expect("query should succeed");

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].message, "account 1 violation");
}

#[tokio::test]
async fn test_get_recent_risk_violations_excludes_non_violation_event_types() {
    let pool = setup_test_db().await;
    let repo = PerformanceRepository::new(pool.clone());

    let now = Utc::now();
    insert_risk_event(&pool, "RiskWarning", 1, None, "low", "just a warning", now).await;
    insert_risk_event(
        &pool,
        "RiskViolation",
        1,
        Some("pdt_limit"),
        "critical",
        "actual violation",
        now,
    )
    .await;

    let violations = repo
        .get_recent_risk_violations(1, 10)
        .await
        .expect("query should succeed");

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].message, "actual violation");
}

#[tokio::test]
async fn test_get_recent_risk_violations_respects_limit() {
    let pool = setup_test_db().await;
    let repo = PerformanceRepository::new(pool.clone());

    let base = Utc::now();
    for i in 0..5 {
        insert_risk_event(
            &pool,
            "RiskViolation",
            1,
            Some("daily_loss_limit"),
            "moderate",
            &format!("violation {i}"),
            base - chrono::Duration::minutes(i as i64),
        )
        .await;
    }

    let violations = repo
        .get_recent_risk_violations(1, 2)
        .await
        .expect("query should succeed");

    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].message, "violation 0");
    assert_eq!(violations[1].message, "violation 1");
}

#[tokio::test]
async fn test_get_recent_risk_violations_empty_when_no_matches() {
    let pool = setup_test_db().await;
    let repo = PerformanceRepository::new(pool.clone());

    let violations = repo
        .get_recent_risk_violations(999, 10)
        .await
        .expect("query should succeed");

    assert!(violations.is_empty());
}
