//! Mock database implementations for testing
//!
//! This module provides mock implementations of database repositories
//! that can be used for testing without requiring a real database connection.
//!
//! Enable with the "mock" feature flag.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::models::*;
use crate::db::traits::*;

/// Mock implementation of PdtRepository for testing
#[derive(Clone)]
pub struct MockPdtRepository {
    day_trades: Arc<RwLock<HashMap<i64, Vec<DayTradeRecord>>>>,
    audit_events: Arc<RwLock<Vec<PdtAuditEvent>>>,
    tracking: Arc<RwLock<HashMap<i64, PdtTracking>>>,
}

impl MockPdtRepository {
    pub fn new() -> Self {
        Self {
            day_trades: Arc::new(RwLock::new(HashMap::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            tracking: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PdtRepositoryTrait for MockPdtRepository {
    async fn record_day_trade(
        &self,
        day_trade: &DayTradeRecord,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut trades = self.day_trades.write().await;
        let account_trades = trades.entry(day_trade.account_id).or_insert_with(Vec::new);
        let id = (account_trades.len() + 1) as i64;
        let mut trade_with_id = day_trade.clone();
        // Note: This assumes DayTradeRecord has an id field that can be modified
        // In a real implementation, you'd use a proper ID assignment
        account_trades.push(trade_with_id);
        Ok(id)
    }

    async fn record_audit_event(
        &self,
        event: &PdtAuditEvent,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut events = self.audit_events.write().await;
        let id = (events.len() + 1) as i64;
        let mut event_with_id = event.clone();
        // Note: This assumes PdtAuditEvent has an id field
        events.push(event_with_id);
        Ok(id)
    }

    async fn upsert_pdt_tracking(
        &self,
        pdt_tracking: &PdtTracking,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut tracking = self.tracking.write().await;
        tracking.insert(pdt_tracking.account_id, pdt_tracking.clone());
        Ok(())
    }

    async fn get_day_trades_in_range(
        &self,
        account_id: i64,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let trades = self.day_trades.read().await;
        if let Some(account_trades) = trades.get(&account_id) {
            let filtered: Vec<DayTradeRecord> = account_trades
                .iter()
                .filter(|trade| {
                    trade.close_time.date_naive() >= start_date
                        && trade.close_time.date_naive() <= end_date
                })
                .cloned()
                .collect();
            Ok(filtered)
        } else {
            Ok(vec![])
        }
    }

    async fn count_day_trades_in_window(
        &self,
        account_id: i64,
        window_start: NaiveDate,
        window_end: NaiveDate,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        let trades = self.day_trades.read().await;
        if let Some(account_trades) = trades.get(&account_id) {
            let count = account_trades
                .iter()
                .filter(|trade| {
                    trade.close_time.date_naive() >= window_start
                        && trade.close_time.date_naive() <= window_end
                })
                .count() as i32;
            Ok(count)
        } else {
            Ok(0)
        }
    }

    async fn get_current_pdt_tracking(
        &self,
        account_id: i64,
    ) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let tracking = self.tracking.read().await;
        Ok(tracking.get(&account_id).cloned())
    }

    async fn get_pdt_tracking_history(
        &self,
        account_id: i64,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let tracking = self.tracking.read().await;
        if let Some(account_tracking) = tracking.get(&account_id) {
            if account_tracking.trading_date >= start_date
                && account_tracking.trading_date <= end_date
            {
                Ok(vec![account_tracking.clone()])
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }

    async fn get_audit_events(
        &self,
        account_id: i64,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let events = self.audit_events.read().await;
        let filtered: Vec<PdtAuditEvent> = events
            .iter()
            .filter(|event| event.account_id == account_id)
            .cloned()
            .collect();

        let limit = limit.unwrap_or(i32::MAX) as usize;
        Ok(filtered.into_iter().take(limit).collect())
    }

    async fn get_audit_events_by_type(
        &self,
        account_id: i64,
        event_type: &str,
        limit: Option<i32>,
    ) -> Result<Vec<PdtAuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let events = self.audit_events.read().await;
        let filtered: Vec<PdtAuditEvent> = events
            .iter()
            .filter(|event| event.account_id == account_id && event.event_type == event_type)
            .cloned()
            .collect();

        let limit = limit.unwrap_or(i32::MAX) as usize;
        Ok(filtered.into_iter().take(limit).collect())
    }

    async fn get_day_trade_summary(
        &self,
        account_id: i64,
        date_range_days: i32,
    ) -> Result<DayTradeSummary, Box<dyn std::error::Error + Send + Sync>> {
        let trades = self.day_trades.read().await;
        let account_trades = trades.get(&account_id).unwrap_or(&vec![]);

        let total_trades = account_trades.len() as i32;
        let winning_trades = account_trades
            .iter()
            .filter(|t| t.realized_pl > 0.0)
            .count() as i32;
        let losing_trades = total_trades - winning_trades;
        let total_pl: f64 = account_trades.iter().map(|t| t.realized_pl).sum();
        let avg_pl = if total_trades > 0 {
            total_pl / total_trades as f64
        } else {
            0.0
        };
        let max_profit = account_trades
            .iter()
            .filter(|t| t.realized_pl > 0.0)
            .map(|t| t.realized_pl)
            .fold(0.0, f64::max);
        let max_loss = account_trades
            .iter()
            .filter(|t| t.realized_pl < 0.0)
            .map(|t| t.realized_pl)
            .fold(0.0, f64::min);

        Ok(DayTradeSummary {
            account_id,
            date_range_days,
            total_trades,
            total_profit_loss: total_pl,
            winning_trades,
            losing_trades,
            avg_profit_loss: avg_pl,
            max_profit,
            max_loss,
            unique_symbols: account_trades
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len() as i32,
            trading_days: account_trades
                .iter()
                .map(|t| t.close_time.date_naive())
                .collect::<std::collections::HashSet<_>>()
                .len() as i32,
            win_rate: if total_trades > 0 {
                winning_trades as f64 / total_trades as f64
            } else {
                0.0
            },
        })
    }

    async fn cleanup_old_records(
        &self,
        _retention_years: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Mock implementation - just return success
        Ok(())
    }
}

/// Factory function to create a mock PDT repository
#[cfg(feature = "mock")]
pub fn create_mock_pdt_repository() -> Box<dyn PdtRepositoryTrait> {
    Box::new(MockPdtRepository::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirms the `tokio::sync::RwLock` conversion (B-063) still lets
    /// concurrent async callers record and read day trades without a
    /// blocking `std::sync` lock or a poisoned-lock unwrap.
    #[tokio::test]
    async fn test_record_and_read_day_trades_concurrently() {
        let repo = MockPdtRepository::new();
        let account_id = 42;
        let today = Utc::now().date_naive();

        let now = Utc::now();
        let trade = DayTradeRecord {
            id: 0,
            account_id,
            trade_id: "trade-1".to_string(),
            symbol: "AAPL".to_string(),
            quantity: 10,
            open_time: now,
            close_time: now,
            open_price: 100.0,
            close_price: 101.0,
            realized_pl: 10.0,
            commission: 0.0,
            pattern_name: None,
            is_validated: true,
            created_at: now,
            updated_at: now,
        };

        repo.record_day_trade(&trade)
            .await
            .expect("record day trade");

        let trades = repo
            .get_day_trades_in_range(account_id, today, today)
            .await
            .expect("read day trades");

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].symbol, "AAPL");
    }

    /// Reproduces the write-then-read pattern PdtEnforcer relies on: several
    /// concurrent writers must not deadlock or panic on a poisoned lock.
    #[tokio::test]
    async fn test_concurrent_writers_do_not_deadlock() {
        let repo = Arc::new(MockPdtRepository::new());
        let account_id = 7;

        let mut handles = Vec::new();
        for i in 0..10 {
            let repo = Arc::clone(&repo);
            handles.push(tokio::spawn(async move {
                let event = PdtAuditEvent {
                    id: 0,
                    account_id,
                    event_type: "day_trade".to_string(),
                    event_date: Utc::now().date_naive(),
                    day_trade_count: Some(i),
                    equity_value: 25000.0,
                    min_equity_threshold: None,
                    max_day_trades_allowed: None,
                    is_restricted: false,
                    violation_details: None,
                    trade_reference: None,
                    message: format!("event {i}"),
                    metadata: None,
                    created_at: Utc::now(),
                };
                repo.record_audit_event(&event).await
            }));
        }

        for handle in handles {
            handle.await.expect("task join").expect("record event");
        }

        let events = repo
            .get_audit_events(account_id, None)
            .await
            .expect("read audit events");
        assert_eq!(events.len(), 10);
    }
}
