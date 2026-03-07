//! Mock database implementations for testing
//!
//! This module provides mock implementations of database repositories
//! that can be used for testing without requiring a real database connection.
//!
//! Enable with the "mock" feature flag.

use async_trait::async_trait;
use chrono::{DateTime, Utc, NaiveDate};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::db::traits::*;
use crate::db::models::*;

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
    async fn record_day_trade(&self, day_trade: &DayTradeRecord) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut trades = self.day_trades.write().unwrap();
        let account_trades = trades.entry(day_trade.account_id).or_insert_with(Vec::new);
        let id = (account_trades.len() + 1) as i64;
        let mut trade_with_id = day_trade.clone();
        // Note: This assumes DayTradeRecord has an id field that can be modified
        // In a real implementation, you'd use a proper ID assignment
        account_trades.push(trade_with_id);
        Ok(id)
    }

    async fn record_audit_event(&self, event: &PdtAuditEvent) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut events = self.audit_events.write().unwrap();
        let id = (events.len() + 1) as i64;
        let mut event_with_id = event.clone();
        // Note: This assumes PdtAuditEvent has an id field
        events.push(event_with_id);
        Ok(id)
    }

    async fn upsert_pdt_tracking(&self, pdt_tracking: &PdtTracking) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut tracking = self.tracking.write().unwrap();
        tracking.insert(pdt_tracking.account_id, pdt_tracking.clone());
        Ok(())
    }

    async fn get_day_trades_in_range(
        &self,
        account_id: i64,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<DayTradeRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let trades = self.day_trades.read().unwrap();
        if let Some(account_trades) = trades.get(&account_id) {
            let filtered: Vec<DayTradeRecord> = account_trades
                .iter()
                .filter(|trade| {
                    trade.close_time.date_naive() >= start_date &&
                    trade.close_time.date_naive() <= end_date
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
        let trades = self.day_trades.read().unwrap();
        if let Some(account_trades) = trades.get(&account_id) {
            let count = account_trades
                .iter()
                .filter(|trade| {
                    trade.close_time.date_naive() >= window_start &&
                    trade.close_time.date_naive() <= window_end
                })
                .count() as i32;
            Ok(count)
        } else {
            Ok(0)
        }
    }

    async fn get_current_pdt_tracking(&self, account_id: i64) -> Result<Option<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let tracking = self.tracking.read().unwrap();
        Ok(tracking.get(&account_id).cloned())
    }

    async fn get_pdt_tracking_history(
        &self,
        account_id: i64,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<PdtTracking>, Box<dyn std::error::Error + Send + Sync>> {
        let tracking = self.tracking.read().unwrap();
        if let Some(account_tracking) = tracking.get(&account_id) {
            if account_tracking.trading_date >= start_date && account_tracking.trading_date <= end_date {
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
        let events = self.audit_events.read().unwrap();
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
        let events = self.audit_events.read().unwrap();
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
        let trades = self.day_trades.read().unwrap();
        let account_trades = trades.get(&account_id).unwrap_or(&vec![]);

        let total_trades = account_trades.len() as i32;
        let winning_trades = account_trades.iter().filter(|t| t.realized_pl > 0.0).count() as i32;
        let losing_trades = total_trades - winning_trades;
        let total_pl: f64 = account_trades.iter().map(|t| t.realized_pl).sum();
        let avg_pl = if total_trades > 0 { total_pl / total_trades as f64 } else { 0.0 };
        let max_profit = account_trades.iter().filter(|t| t.realized_pl > 0.0).map(|t| t.realized_pl).fold(0.0, f64::max);
        let max_loss = account_trades.iter().filter(|t| t.realized_pl < 0.0).map(|t| t.realized_pl).fold(0.0, f64::min);

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
            unique_symbols: account_trades.iter().collect::<std::collections::HashSet<_>>().len() as i32,
            trading_days: account_trades.iter().map(|t| t.close_time.date_naive()).collect::<std::collections::HashSet<_>>().len() as i32,
            win_rate: if total_trades > 0 { winning_trades as f64 / total_trades as f64 } else { 0.0 },
        })
    }

    async fn cleanup_old_records(&self, _retention_years: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Mock implementation - just return success
        Ok(())
    }
}

/// Factory function to create a mock PDT repository
#[cfg(feature = "mock")]
pub fn create_mock_pdt_repository() -> Box<dyn PdtRepositoryTrait> {
    Box::new(MockPdtRepository::new())
}