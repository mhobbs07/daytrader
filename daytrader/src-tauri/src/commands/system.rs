use tauri::State;
use std::sync::atomic::Ordering;
use crate::state::AppState;

#[tauri::command]
pub async fn get_system_status(
    state: State<'_, AppState>
) -> Result<SystemStatus, String> {
    Ok(SystemStatus {
        broker_connected: state.broker_connected.load(Ordering::Relaxed),
        event_bus_active: state.event_bus.subscriber_count() > 0,
        patterns_active: state.pattern_registry.pattern_count(),
        events_published: state.event_bus.events_published(),
        events_dropped: state.event_bus.events_dropped(),
        average_latency_ms: state.event_bus.get_average_latency_ns() / 1_000_000.0,
    })
}

#[derive(serde::Serialize)]
pub struct SystemStatus {
    pub broker_connected: bool,
    pub event_bus_active: bool,
    pub patterns_active: usize,
    pub events_published: u64,
    pub events_dropped: u64,
    pub average_latency_ms: f64,
}