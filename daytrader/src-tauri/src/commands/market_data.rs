use tauri::State;
use daytrader::brokers::{Symbol, Candle, Quote};
use crate::state::AppState;

#[tauri::command]
pub async fn get_latest_quote(
    symbol: String,
    state: State<'_, AppState>
) -> Result<QuoteResponse, String> {
    // 1. Validate input
    let symbol = Symbol::new(&symbol)
        .map_err(|e| format!("Invalid symbol: {}", e))?;

    // 2. Query from cache (functional, read-only)
    let quote = state.market_cache
        .lock()
        .unwrap()
        .get_latest_quote(&symbol)
        .ok_or_else(|| format!("No quote available for {}", symbol))?;

    // 3. Convert to response type
    Ok(QuoteResponse {
        symbol: symbol.to_string(),
        bid: quote.bid.as_f64(),
        ask: quote.ask.as_f64(),
        timestamp: quote.timestamp.to_rfc3339(),
    })
}

#[tauri::command]
pub async fn get_recent_candles(
    symbol: String,
    count: usize,
    state: State<'_, AppState>
) -> Result<Vec<CandleResponse>, String> {
    let symbol = Symbol::new(&symbol)
        .map_err(|e| format!("Invalid symbol: {}", e))?;

    let candles = state.market_cache
        .lock()
        .unwrap()
        .get_recent_candles(&symbol, count)
        .map_err(|e| format!("Failed to get candles: {}", e))?;

    Ok(candles.into_iter().map(|candle| CandleResponse {
        symbol: candle.symbol.to_string(),
        timestamp: candle.timestamp.to_rfc3339(),
        open: candle.open.as_f64(),
        high: candle.high.as_f64(),
        low: candle.low.as_f64(),
        close: candle.close.as_f64(),
        volume: candle.volume,
    }).collect())
}

#[tauri::command]
pub async fn subscribe_symbol(
    symbol: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    let symbol = Symbol::new(&symbol)
        .map_err(|e| format!("Invalid symbol: {}", e))?;

    state.broker
        .subscribe_quotes(vec![symbol])
        .await
        .map_err(|e| format!("Subscription failed: {}", e))
}

#[derive(serde::Serialize)]
pub struct QuoteResponse {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    pub timestamp: String,
}

#[derive(serde::Serialize)]
pub struct CandleResponse {
    pub symbol: String,
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}