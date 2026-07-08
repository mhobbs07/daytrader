use crate::state::AppState;
use tauri::State;

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
pub struct PatternStats {
    pub total_detections: u64,
    pub patterns_by_type: Vec<(String, u64)>,
    pub average_confidence: f64,
}
