use tauri::State;
use daytrader::risk::{RiskEngine, engine::RiskState};
use crate::state::AppState;
use chrono::Utc;

#[derive(serde::Serialize)]
pub struct RiskStateResponse {
    pub account_id: String,
    pub trading_date: String,
    pub daily_pl: f64,
    pub day_trade_count: u32,
    pub is_trading_halted: bool,
    pub last_updated: String,
}

#[derive(serde::Serialize)]
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

#[derive(serde::Serialize)]
pub struct RiskViolationResponse {
    pub violation_type: String,
    pub message: String,
    pub timestamp: String,
    pub severity: String,
    pub suggested_action: Option<String>,
}

#[tauri::command]
pub async fn get_risk_state(
    state: State<'_, AppState>
) -> Result<RiskStateResponse, String> {
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
pub async fn get_risk_limits(
    state: State<'_, AppState>
) -> Result<RiskLimitsResponse, String> {
    // For now, return the configured limits
    // TODO: Add method to RiskEngine to expose limits
    Ok(RiskLimitsResponse {
        max_daily_loss_dollars: 500.0,
        max_daily_loss_percent: 2.0,
        max_position_size_dollars: 5000.0,
        max_position_size_percent: 10.0,
        max_concurrent_positions: 5,
        enforce_pdt_rule: true,
        pdt_min_equity: 25000.0,
        max_day_trades_under_pdt: 3,
    })
}

#[tauri::command]
pub async fn get_recent_risk_violations(
    state: State<'_, AppState>
) -> Result<Vec<RiskViolationResponse>, String> {
    // Return empty list for now - no violations
    // TODO: Get from actual risk violation history
    Ok(vec![])
}