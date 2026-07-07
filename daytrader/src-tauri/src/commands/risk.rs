use crate::state::AppState;
use chrono::{DateTime, Datelike, Utc};
use daytrader::risk::{RiskEngine, engine::RiskState};
use daytrader::utils::holidays::MarketHolidayCalendar;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tracing::{debug, error, warn};

// ============================================================================
// Response Structures for Frontend
// ============================================================================

#[derive(Serialize)]
pub struct RiskStateResponse {
    pub account_id: String,
    pub trading_date: String,
    pub daily_pl: f64,
    pub day_trade_count: u32,
    pub is_trading_halted: bool,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct RiskLimitsResponse {
    pub max_daily_loss_dollars: f64,
    pub max_daily_loss_percent: f64,
    pub max_position_size_dollars: f64,
    pub max_position_size_percent: f64,
    pub max_concurrent_positions: u32,
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: f64,
    pub max_day_trades_under_pdt: u32,
}

#[derive(Serialize)]
pub struct RiskViolationResponse {
    pub violation_type: String,
    pub message: String,
    pub timestamp: String,
    pub severity: String,
    pub suggested_action: Option<String>,
    pub pattern_name: Option<String>,
}

#[derive(Serialize)]
pub struct PdtStatusResponse {
    pub account_id: String,
    pub current_equity: f64,
    pub pdt_min_equity: f64,
    pub meets_equity_requirement: bool,
    pub day_trade_count: u32,
    pub max_day_trades_allowed: u32,
    pub remaining_day_trades: u32,
    pub is_restricted: bool,
    pub rolling_window_start: String,
    pub rolling_window_end: String,
    pub business_days_remaining: u32,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct PdtEligibilityResponse {
    pub eligible: bool,
    pub reason: String,
    pub current_day_trades: u32,
    pub max_allowed: u32,
    pub equity_requirement_met: bool,
    pub would_be_day_trade: bool,
    pub suggested_action: Option<String>,
}

#[derive(Serialize)]
pub struct DayTradeHistoryResponse {
    pub trades: Vec<DayTradeRecordResponse>,
    pub total_count: i32,
    pub window_start: String,
    pub window_end: String,
}

#[derive(Serialize)]
pub struct DayTradeRecordResponse {
    pub trade_id: String,
    pub symbol: String,
    pub quantity: i32,
    pub open_time: String,
    pub close_time: String,
    pub open_price: f64,
    pub close_price: f64,
    pub realized_pl: f64,
    pub commission: f64,
    pub pattern_name: Option<String>,
}

#[derive(Serialize)]
pub struct PdtViolationsResponse {
    pub violations: Vec<PdtViolationRecord>,
    pub audit_events: Vec<PdtAuditEventResponse>,
}

#[derive(Serialize)]
pub struct PdtViolationRecord {
    pub event_type: String,
    pub event_date: String,
    pub day_trade_count: Option<i32>,
    pub equity_value: f64,
    pub is_restricted: bool,
    pub violation_details: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct PdtAuditEventResponse {
    pub id: i64,
    pub event_type: String,
    pub event_date: String,
    pub day_trade_count: Option<i32>,
    pub equity_value: f64,
    pub is_restricted: bool,
    pub message: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct BusinessDayInfoResponse {
    pub current_date: String,
    pub is_business_day: bool,
    pub next_business_day: String,
    pub previous_business_day: String,
    pub upcoming_holidays: Vec<HolidayInfo>,
    pub business_days_in_current_month: u32,
}

#[derive(Serialize)]
pub struct HolidayInfo {
    pub name: String,
    pub date: String,
    pub is_market_holiday: bool,
}

// ============================================================================
// Existing Commands (Enhanced)
// ============================================================================

#[tauri::command]
pub async fn get_risk_state(state: State<'_, AppState>) -> Result<RiskStateResponse, String> {
    debug!("Getting risk state");

    let risk_state = state.risk_engine.get_risk_state().await;

    Ok(RiskStateResponse {
        account_id: risk_state.account_id.to_string(),
        trading_date: risk_state.trading_date.to_string(),
        daily_pl: risk_state.daily_pl.to_f64().unwrap_or(0.0),
        day_trade_count: risk_state.day_trade_count,
        is_trading_halted: risk_state.is_trading_halted,
        last_updated: risk_state.last_updated.to_rfc3339(),
    })
}

#[tauri::command]
pub async fn get_risk_limits(state: State<'_, AppState>) -> Result<RiskLimitsResponse, String> {
    debug!("Getting risk limits from configuration");

    let config = state.risk_engine.get_config();
    Ok(RiskLimitsResponse {
        max_daily_loss_dollars: config.max_daily_loss_dollars,
        max_daily_loss_percent: config.max_daily_loss_percent,
        max_position_size_dollars: config.max_position_size_dollars,
        max_position_size_percent: config.max_position_size_percent,
        max_concurrent_positions: config.max_concurrent_positions as u32,
        enforce_pdt_rule: config.enforce_pdt_rule,
        pdt_min_equity: config.pdt_min_equity,
        max_day_trades_under_pdt: config.max_day_trades_under_pdt as u32,
    })
}

#[tauri::command]
pub async fn get_recent_risk_violations(
    state: State<'_, AppState>,
) -> Result<Vec<RiskViolationResponse>, String> {
    debug!("Getting recent risk violations");

    const RECENT_VIOLATIONS_LIMIT: i32 = 50;

    let Some(performance_repository) = state.performance_repository() else {
        // Mock mode: no database is configured at all, so "no violations" is
        // the correct answer rather than an error condition.
        debug!("No database configured, returning empty risk violations list");
        return Ok(vec![]);
    };

    match performance_repository
        .get_recent_risk_violations(state.account_id, RECENT_VIOLATIONS_LIMIT)
        .await
    {
        Ok(events) => Ok(events.into_iter().map(risk_event_to_response).collect()),
        Err(e) => {
            // A real database is configured but the query failed: surface the
            // error to the frontend instead of masking an outage as "no
            // violations" (an empty list here would be misleading on a risk
            // dashboard).
            error!("Failed to retrieve risk violations: {}", e);
            Err(format!("Failed to retrieve risk violations: {}", e))
        }
    }
}

/// Maps a persisted risk event row to the frontend violation response shape.
fn risk_event_to_response(event: daytrader::db::models::RiskEvent) -> RiskViolationResponse {
    RiskViolationResponse {
        violation_type: event
            .violation_type
            .unwrap_or_else(|| event.event_type.clone()),
        message: event.message,
        timestamp: event.timestamp.to_rfc3339(),
        severity: event.severity.unwrap_or_else(|| "moderate".to_string()),
        suggested_action: None,
        pattern_name: event.pattern_name,
    }
}

// ============================================================================
// New PDT-Specific Commands
// ============================================================================

#[tauri::command]
pub async fn get_pdt_status(state: State<'_, AppState>) -> Result<PdtStatusResponse, String> {
    debug!("Getting PDT status");

    // Check if PDT enforcer is available
    if let Some(pdt_enforcer) = state.pdt_enforcer() {
        let pdt_state = pdt_enforcer.get_pdt_state().await;
        let current_date = Utc::now().date_naive();
        let day_trade_count = pdt_enforcer.count_day_trades().await;
        let meets_requirement = pdt_enforcer.meets_equity_requirement().await;

        // Calculate rolling window dates using holiday calendar
        let holiday_calendar = state.holiday_calendar();
        let window_start = holiday_calendar.subtract_business_days(current_date, 5);

        // Determine max allowed day trades based on equity
        let max_allowed = if meets_requirement { u32::MAX } else { 3 };
        let remaining = if meets_requirement {
            u32::MAX
        } else {
            max_allowed.saturating_sub(day_trade_count)
        };

        // Generate warnings
        let mut warnings = Vec::new();
        if !meets_requirement && day_trade_count >= 2 {
            warnings.push("Approaching PDT day trade limit".to_string());
        }
        if !meets_requirement && day_trade_count >= 3 {
            warnings.push("PDT day trade limit reached. Cannot day trade until equity exceeds $25,000 or rolling window resets".to_string());
        }

        Ok(PdtStatusResponse {
            account_id: state.account_id.to_string(),
            current_equity: pdt_state.current_equity.to_f64().unwrap_or(0.0),
            pdt_min_equity: 25000.0, // Standard PDT minimum
            meets_equity_requirement: meets_requirement,
            day_trade_count,
            max_day_trades_allowed: max_allowed,
            remaining_day_trades: remaining,
            is_restricted: !meets_requirement && day_trade_count >= 3,
            rolling_window_start: window_start.to_string(),
            rolling_window_end: current_date.to_string(),
            business_days_remaining: 5, // Standard PDT window
            warnings,
        })
    } else {
        // Fallback when no PDT enforcer is available
        warn!("PDT enforcer not available, returning basic status");
        let current_date = Utc::now().date_naive();
        let window_start = current_date - chrono::Duration::days(5);

        Ok(PdtStatusResponse {
            account_id: state.account_id.to_string(),
            current_equity: 50000.0, // Default for demo
            pdt_min_equity: 25000.0,
            meets_equity_requirement: true,
            day_trade_count: 0,
            max_day_trades_allowed: u32::MAX,
            remaining_day_trades: u32::MAX,
            is_restricted: false,
            rolling_window_start: window_start.to_string(),
            rolling_window_end: current_date.to_string(),
            business_days_remaining: 5,
            warnings: vec!["PDT enforcer not configured - using demo values".to_string()],
        })
    }
}

#[tauri::command]
pub async fn get_day_trade_history(
    days: Option<i32>,
    state: State<'_, AppState>,
) -> Result<DayTradeHistoryResponse, String> {
    let days = days.unwrap_or(30);
    debug!("Getting day trade history for {} days", days);

    let end_date = Utc::now().date_naive();
    let start_date = end_date - chrono::Duration::days(days as i64);

    if let Some(pdt_repo) = state.pdt_repository() {
        match pdt_repo
            .get_day_trades_in_range(state.account_id, start_date, end_date)
            .await
        {
            Ok(trades) => {
                let trade_responses: Vec<DayTradeRecordResponse> = trades
                    .into_iter()
                    .map(|trade| DayTradeRecordResponse {
                        trade_id: trade.trade_id,
                        symbol: trade.symbol,
                        quantity: trade.quantity,
                        open_time: trade.open_time.to_rfc3339(),
                        close_time: trade.close_time.to_rfc3339(),
                        open_price: trade.open_price,
                        close_price: trade.close_price,
                        realized_pl: trade.realized_pl,
                        commission: trade.commission,
                        pattern_name: trade.pattern_name,
                    })
                    .collect();

                Ok(DayTradeHistoryResponse {
                    total_count: trade_responses.len() as i32,
                    trades: trade_responses,
                    window_start: start_date.to_string(),
                    window_end: end_date.to_string(),
                })
            }
            Err(e) => {
                error!("Failed to get day trade history: {}", e);
                Err(format!("Failed to retrieve day trade history: {}", e))
            }
        }
    } else {
        warn!("PDT repository not available");
        Ok(DayTradeHistoryResponse {
            trades: vec![],
            total_count: 0,
            window_start: start_date.to_string(),
            window_end: end_date.to_string(),
        })
    }
}

#[tauri::command]
pub async fn check_pdt_eligibility(
    symbol: String,
    quantity: i32,
    side: String,
    state: State<'_, AppState>,
) -> Result<PdtEligibilityResponse, String> {
    debug!(
        "Checking PDT eligibility for {} {} shares of {}",
        side, quantity, symbol
    );

    // Input validation
    if symbol.is_empty() {
        return Err("Symbol cannot be empty".to_string());
    }

    if quantity <= 0 {
        return Err("Quantity must be positive".to_string());
    }

    if !matches!(side.to_lowercase().as_str(), "buy" | "sell") {
        return Err("Side must be 'buy' or 'sell'".to_string());
    }

    if let Some(pdt_enforcer) = state.pdt_enforcer() {
        let day_trade_count = pdt_enforcer.count_day_trades().await;
        let equity_requirement_met = pdt_enforcer.meets_equity_requirement().await;

        // Check if this would be a day trade using position tracker
        let position_tracker = state.position_tracker();
        let symbol_obj = daytrader::brokers::types::Symbol::new(&symbol)
            .map_err(|e| format!("Invalid symbol: {}", e))?;

        let would_be_day_trade =
            if let Some(position) = position_tracker.get_position(&symbol_obj).await {
                let current_date = Utc::now().date_naive();
                position.has_components_opened_on_date(current_date)
            } else {
                false
            };

        // Determine eligibility
        let max_allowed = if equity_requirement_met { u32::MAX } else { 3 };
        let eligible = if equity_requirement_met {
            true
        } else if would_be_day_trade {
            day_trade_count < max_allowed
        } else {
            true // Not a day trade, so eligible
        };

        let reason = if !eligible {
            format!(
                "PDT day trade limit reached ({}/{})",
                day_trade_count, max_allowed
            )
        } else if would_be_day_trade && !equity_requirement_met {
            format!(
                "Would be day trade - {} day trades used of {} allowed",
                day_trade_count, max_allowed
            )
        } else {
            "Eligible for trade".to_string()
        };

        let suggested_action = if !eligible {
            Some("Increase account equity to $25,000 or wait for rolling window reset".to_string())
        } else if would_be_day_trade && day_trade_count >= 2 && !equity_requirement_met {
            Some("Consider overnight position to avoid PDT limit".to_string())
        } else {
            None
        };

        Ok(PdtEligibilityResponse {
            eligible,
            reason,
            current_day_trades: day_trade_count,
            max_allowed,
            equity_requirement_met,
            would_be_day_trade,
            suggested_action,
        })
    } else {
        // Fallback when no PDT enforcer is available
        warn!("PDT enforcer not available, returning permissive eligibility");
        Ok(PdtEligibilityResponse {
            eligible: true,
            reason: "PDT enforcer not configured - trade allowed".to_string(),
            current_day_trades: 0,
            max_allowed: u32::MAX,
            equity_requirement_met: true,
            would_be_day_trade: false,
            suggested_action: Some("PDT enforcement not active".to_string()),
        })
    }
}

#[tauri::command]
pub async fn get_pdt_violations(
    limit: Option<i32>,
    state: State<'_, AppState>,
) -> Result<PdtViolationsResponse, String> {
    let limit = limit.unwrap_or(50);
    debug!("Getting PDT violations with limit {}", limit);

    if let Some(pdt_repo) = state.pdt_repository() {
        match pdt_repo
            .get_audit_events(state.account_id, Some(limit))
            .await
        {
            Ok(audit_events) => {
                let audit_responses: Vec<PdtAuditEventResponse> = audit_events
                    .into_iter()
                    .map(|event| PdtAuditEventResponse {
                        id: event.id,
                        event_type: event.event_type,
                        event_date: event.event_date.to_string(),
                        day_trade_count: event.day_trade_count,
                        equity_value: event.equity_value,
                        is_restricted: event.is_restricted,
                        message: event.message,
                        created_at: event.created_at.to_rfc3339(),
                    })
                    .collect();

                // For violations, filter audit events for violation types
                let violation_events: Vec<PdtViolationRecord> = audit_responses
                    .iter()
                    .filter(|event| event.event_type.contains("violation") || event.is_restricted)
                    .map(|event| PdtViolationRecord {
                        event_type: event.event_type.clone(),
                        event_date: event.event_date.clone(),
                        day_trade_count: event.day_trade_count,
                        equity_value: event.equity_value,
                        is_restricted: event.is_restricted,
                        violation_details: None,
                        message: event.message.clone(),
                    })
                    .collect();

                Ok(PdtViolationsResponse {
                    violations: violation_events,
                    audit_events: audit_responses,
                })
            }
            Err(e) => {
                error!("Failed to get PDT violations: {}", e);
                Err(format!("Failed to retrieve PDT violations: {}", e))
            }
        }
    } else {
        warn!("PDT repository not available");
        Ok(PdtViolationsResponse {
            violations: vec![],
            audit_events: vec![],
        })
    }
}

#[tauri::command]
pub async fn get_business_day_info(
    state: State<'_, AppState>,
) -> Result<BusinessDayInfoResponse, String> {
    debug!("Getting business day information");

    let current_date = Utc::now().date_naive();
    let holiday_calendar = state.holiday_calendar();

    // Use the actual holiday calendar for accurate business day calculations
    let is_business_day = holiday_calendar.is_business_day(current_date);
    let next_business_day = holiday_calendar.add_business_days(current_date, 1);
    let previous_business_day = holiday_calendar.subtract_business_days(current_date, 1);

    // Get upcoming holidays (next 90 days)
    let mut upcoming_holidays = Vec::new();
    let mut check_date = current_date;
    let end_date = current_date + chrono::Duration::days(90);

    while check_date <= end_date {
        if holiday_calendar.is_holiday(check_date) {
            if let Some(holiday_name) = holiday_calendar.get_holiday_name(check_date) {
                upcoming_holidays.push(HolidayInfo {
                    name: holiday_name.to_string(),
                    date: check_date.to_string(),
                    is_market_holiday: true,
                });
            }
        }
        check_date = check_date.succ_opt().unwrap_or(check_date);

        // Prevent infinite loops
        if upcoming_holidays.len() > 20 {
            break;
        }
    }

    // Count business days in current month
    let business_days_in_month =
        count_business_days_in_month_with_calendar(current_date, &holiday_calendar);

    Ok(BusinessDayInfoResponse {
        current_date: current_date.to_string(),
        is_business_day,
        next_business_day: next_business_day.to_string(),
        previous_business_day: previous_business_day.to_string(),
        upcoming_holidays,
        business_days_in_current_month: business_days_in_month,
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

fn is_weekday(date: chrono::NaiveDate) -> bool {
    let weekday = date.weekday();
    !matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun)
}

fn find_next_business_day(mut date: chrono::NaiveDate) -> chrono::NaiveDate {
    loop {
        date = date.succ_opt().unwrap_or(date);
        if is_weekday(date) {
            return date;
        }
    }
}

fn find_previous_business_day(mut date: chrono::NaiveDate) -> chrono::NaiveDate {
    loop {
        date = date.pred_opt().unwrap_or(date);
        if is_weekday(date) {
            return date;
        }
    }
}

fn count_business_days_in_month(date: chrono::NaiveDate) -> u32 {
    let start_of_month =
        chrono::NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    let end_of_month = if date.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
            .unwrap_or(date)
            .pred_opt()
            .unwrap_or(date)
    } else {
        chrono::NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
            .unwrap_or(date)
            .pred_opt()
            .unwrap_or(date)
    };

    let mut count = 0;
    let mut current = start_of_month;
    while current <= end_of_month {
        if is_weekday(current) {
            count += 1;
        }
        current = current.succ_opt().unwrap_or_else(|| current);
        if current == start_of_month {
            break; // Prevent infinite loop
        }
    }
    count
}

fn count_business_days_in_month_with_calendar(
    date: chrono::NaiveDate,
    calendar: &MarketHolidayCalendar,
) -> u32 {
    let start_of_month =
        chrono::NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    let end_of_month = if date.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
            .unwrap_or(date)
            .pred_opt()
            .unwrap_or(date)
    } else {
        chrono::NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
            .unwrap_or(date)
            .pred_opt()
            .unwrap_or(date)
    };

    let mut count = 0;
    let mut current = start_of_month;
    while current <= end_of_month {
        if calendar.is_business_day(current) {
            count += 1;
        }
        current = current.succ_opt().unwrap_or_else(|| current);
        if current == start_of_month {
            break; // Prevent infinite loop
        }
    }
    count
}
