use crate::state::AppState;
use daytrader::events::types::PatternSignal;
use tauri::State;

#[tauri::command]
pub async fn get_detected_patterns_for_symbol(
    symbol: String,
    state: State<'_, AppState>,
) -> Result<Vec<PatternSignalResponse>, String> {
    // Get recent patterns from event history or pattern cache
    let patterns = state
        .pattern_cache
        .lock()
        .unwrap()
        .get_patterns_for_symbol(&symbol)
        .map_err(|e| format!("Failed to get patterns: {}", e))?;

    Ok(patterns
        .into_iter()
        .map(|signal| PatternSignalResponse {
            symbol: signal.symbol.to_string(),
            pattern_name: signal.pattern_name.to_string(),
            signal_type: match signal.signal_type {
                daytrader::events::types::SignalType::Buy => "Buy".to_string(),
                daytrader::events::types::SignalType::Sell => "Sell".to_string(),
            },
            confidence: signal.confidence.value(),
            detected_at: signal.detected_at.to_rfc3339(),
            price: signal.price.as_f64(),
        })
        .collect())
}

#[tauri::command]
pub async fn get_pattern_statistics(state: State<'_, AppState>) -> Result<PatternStats, String> {
    // Return aggregated pattern performance stats
    let all_stats = state.pattern_registry.get_all_stats();

    let total_detections: u64 = all_stats.values().map(|s| s.total_detections).sum();
    let average_confidence: f64 = if !all_stats.is_empty() {
        all_stats
            .values()
            .map(|s| s.average_confidence)
            .sum::<f64>()
            / all_stats.len() as f64
    } else {
        0.0
    };

    let patterns_by_type: Vec<(String, u64)> = all_stats
        .into_iter()
        .map(|(name, stats)| (name, stats.total_detections))
        .collect();

    Ok(PatternStats {
        total_detections,
        patterns_by_type,
        average_confidence,
    })
}

#[derive(serde::Serialize)]
pub struct PatternSignalResponse {
    pub symbol: String,
    pub pattern_name: String,
    pub signal_type: String,
    pub confidence: f64,
    pub detected_at: String,
    pub price: f64,
}

#[derive(serde::Serialize)]
pub struct PatternStats {
    pub total_detections: u64,
    pub patterns_by_type: Vec<(String, u64)>,
    pub average_confidence: f64,
}
