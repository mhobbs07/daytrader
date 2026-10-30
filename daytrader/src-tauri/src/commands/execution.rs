use crate::state::AppState;
use tauri::State;

const ENGINES_UNAVAILABLE: &str =
    "trading engines unavailable — database not connected (see startup logs)";

#[derive(serde::Serialize)]
pub struct ExecutionStatusResponse {
    /// False when the app started without a database and the trading loop is inert
    pub engines_available: bool,
    pub automation_enabled: bool,
    pub total_signals: u64,
    pub auto_executed: u64,
    pub manual_review: u64,
    pub rejected: u64,
    pub avg_exec_time_ms: u64,
}

#[derive(serde::Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: u32,
    pub limit_price: Option<f64>,
    pub status: String,
    pub filled_quantity: u32,
    pub filled_avg_price: Option<f64>,
    pub pattern_name: Option<String>,
    pub created_at: String,
}

#[derive(serde::Serialize)]
pub struct PositionResponse {
    pub symbol: String,
    pub side: String,
    pub quantity: u32,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pl: f64,
    pub unrealized_pl_percent: f64,
    pub realized_pl: f64,
    pub pattern_name: Option<String>,
    pub opened_at: String,
}

fn order_to_response(order: &daytrader::orders::types::ManagedOrder) -> OrderResponse {
    OrderResponse {
        id: order.id.as_str().to_string(),
        symbol: order.symbol.as_str().to_string(),
        side: format!("{:?}", order.side),
        order_type: format!("{:?}", order.order_type),
        quantity: order.quantity.value(),
        limit_price: order.limit_price.map(|p| p.as_f64()),
        status: format!("{:?}", order.status),
        filled_quantity: order.filled_quantity,
        filled_avg_price: order.filled_avg_price.map(|p| p.as_f64()),
        pattern_name: order.pattern_name.as_ref().map(|p| p.as_str().to_string()),
        created_at: order.created_at.to_rfc3339(),
    }
}

/// Reports whether the trading loop is live and its execution metrics
#[tauri::command]
pub async fn get_execution_status(
    state: State<'_, AppState>,
) -> Result<ExecutionStatusResponse, String> {
    match state.execution_engine.as_ref() {
        Some(engine) => {
            let metrics = engine.get_metrics();
            Ok(ExecutionStatusResponse {
                engines_available: true,
                automation_enabled: engine.is_automation_enabled(),
                total_signals: metrics.total_signals,
                auto_executed: metrics.auto_executed,
                manual_review: metrics.manual_review,
                rejected: metrics.rejected,
                avg_exec_time_ms: metrics.avg_exec_time_ms,
            })
        }
        None => Ok(ExecutionStatusResponse {
            engines_available: false,
            automation_enabled: false,
            total_signals: 0,
            auto_executed: 0,
            manual_review: 0,
            rejected: 0,
            avg_exec_time_ms: 0,
        }),
    }
}

/// Toggles global automated execution; returns the new state
#[tauri::command]
pub async fn set_automation_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let engine = state.execution_engine.as_ref().ok_or(ENGINES_UNAVAILABLE)?;

    if enabled {
        engine.enable_automation();
    } else {
        engine.disable_automation();
    }
    Ok(engine.is_automation_enabled())
}

/// Lists orders still in flight (not yet filled/cancelled/rejected)
#[tauri::command]
pub async fn get_active_orders(state: State<'_, AppState>) -> Result<Vec<OrderResponse>, String> {
    let engine = state.order_engine.as_ref().ok_or(ENGINES_UNAVAILABLE)?;
    Ok(engine
        .get_active_orders()
        .iter()
        .map(order_to_response)
        .collect())
}

/// Lists completed (terminal) orders, newest first
#[tauri::command]
pub async fn get_recent_orders(state: State<'_, AppState>) -> Result<Vec<OrderResponse>, String> {
    let engine = state.order_engine.as_ref().ok_or(ENGINES_UNAVAILABLE)?;
    let mut orders: Vec<OrderResponse> = engine
        .get_archived_orders()
        .await
        .iter()
        .map(order_to_response)
        .collect();
    orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    orders.truncate(50);
    Ok(orders)
}

/// Lists open positions with live P&L from the positions tracker
#[tauri::command]
pub async fn get_open_positions(
    state: State<'_, AppState>,
) -> Result<Vec<PositionResponse>, String> {
    use rust_decimal::prelude::ToPrimitive;

    let tracker = state.pnl_tracker.as_ref().ok_or(ENGINES_UNAVAILABLE)?;
    let positions = tracker
        .get_all_positions()
        .await
        .map_err(|e| format!("Failed to read positions: {e}"))?;

    Ok(positions
        .iter()
        .map(|p| PositionResponse {
            symbol: p.symbol.as_str().to_string(),
            side: format!("{:?}", p.side),
            quantity: p.quantity.value(),
            avg_entry_price: p.avg_entry_price.as_f64(),
            current_price: p.current_price.as_f64(),
            unrealized_pl: p.unrealized_pl.to_f64().unwrap_or(0.0),
            unrealized_pl_percent: p.unrealized_pl_percent.to_f64().unwrap_or(0.0),
            realized_pl: p.realized_pl.to_f64().unwrap_or(0.0),
            pattern_name: p.pattern_name.as_ref().map(|n| n.as_str().to_string()),
            opened_at: p.opened_at.to_rfc3339(),
        })
        .collect())
}
