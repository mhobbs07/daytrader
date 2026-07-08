use crate::state::AppState;
use daytrader::brokers::{Candle, Quote, Symbol};
use tauri::State;

#[tauri::command]
pub async fn get_latest_quote(
    symbol: String,
    state: State<'_, AppState>,
) -> Result<QuoteResponse, String> {
    // 1. Validate input
    let symbol = Symbol::new(&symbol).map_err(|e| format!("Invalid symbol: {}", e))?;

    // 2. Query from cache (functional, read-only)
    let quote = state
        .market_cache
        .lock()
        .unwrap()
        .get_latest_quote(&symbol)
        .ok_or_else(|| format!("No quote available for {}", symbol))?;

    // 3. Convert to response type
    Ok(QuoteResponse::from(&quote))
}

#[tauri::command]
pub async fn get_recent_candles(
    symbol: String,
    count: usize,
    state: State<'_, AppState>,
) -> Result<Vec<CandleResponse>, String> {
    let symbol = Symbol::new(&symbol).map_err(|e| format!("Invalid symbol: {}", e))?;

    let candles = state
        .market_cache
        .lock()
        .unwrap()
        .get_recent_candles(&symbol, count)
        .map_err(|e| format!("Failed to get candles: {}", e))?;

    Ok(candles
        .into_iter()
        .map(|candle| CandleResponse {
            symbol: candle.symbol.to_string(),
            timestamp: candle.timestamp.to_rfc3339(),
            open: candle.open.as_f64(),
            high: candle.high.as_f64(),
            low: candle.low.as_f64(),
            close: candle.close.as_f64(),
            volume: candle.volume,
        })
        .collect())
}

#[tauri::command]
pub async fn subscribe_symbol(symbol: String, state: State<'_, AppState>) -> Result<(), String> {
    let symbol = Symbol::new(&symbol).map_err(|e| format!("Invalid symbol: {}", e))?;

    // If a mock data driver is wired up, delegate subscription to it so that
    // synthetic events are emitted.  Otherwise fall back to the broker trait
    // (real broker implementations handle their own streaming).
    if let Some(driver) = &state.mock_driver {
        driver
            .subscribe(symbol)
            .map_err(|e| format!("Subscription failed: {}", e))
    } else {
        state
            .broker
            .subscribe_quotes(vec![symbol])
            .await
            .map_err(|e| format!("Subscription failed: {}", e))
    }
}

#[tauri::command]
pub async fn unsubscribe_symbol(symbol: String, state: State<'_, AppState>) -> Result<(), String> {
    let symbol = Symbol::new(&symbol).map_err(|e| format!("Invalid symbol: {}", e))?;

    if let Some(driver) = &state.mock_driver {
        driver.unsubscribe(&symbol);
        Ok(())
    } else {
        state
            .broker
            .unsubscribe_quotes(vec![symbol])
            .await
            .map_err(|e| format!("Unsubscription failed: {}", e))
    }
}

#[derive(Clone, serde::Serialize)]
pub struct QuoteResponse {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    /// Last traded price. Falls back to the bid/ask midpoint when no last trade is available.
    pub last: f64,
    pub timestamp: String,
}

impl From<&Quote> for QuoteResponse {
    fn from(quote: &Quote) -> Self {
        let bid = quote.bid.as_f64();
        let ask = quote.ask.as_f64();
        // Use last trade price if available; fall back to midpoint
        let last = if quote.last.as_f64() > 0.0 {
            quote.last.as_f64()
        } else {
            (bid + ask) / 2.0
        };
        Self {
            symbol: quote.symbol.to_string(),
            bid,
            ask,
            last,
            timestamp: quote.timestamp.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use daytrader::brokers::types::Price;
    use rust_decimal::Decimal;

    fn sample_quote(last_cents: i64) -> Quote {
        Quote {
            symbol: Symbol::new("AAPL").unwrap(),
            bid: Price::new(Decimal::new(15000, 2)).unwrap(),
            ask: Price::new(Decimal::new(15010, 2)).unwrap(),
            last: Price::new(Decimal::new(last_cents, 2)).unwrap(),
            bid_size: 100,
            ask_size: 200,
            timestamp: Utc.with_ymd_and_hms(2026, 7, 6, 14, 30, 0).unwrap(),
        }
    }

    #[test]
    fn test_quote_response_from_quote_includes_symbol() {
        let response = QuoteResponse::from(&sample_quote(15005));

        assert_eq!(response.symbol, "AAPL");
        assert_eq!(response.bid, 150.0);
        assert_eq!(response.ask, 150.10);
        assert_eq!(response.last, 150.05);
        assert_eq!(response.timestamp, "2026-07-06T14:30:00+00:00");
    }

    /// The `market_data:quote_update` Tauri event emits this exact payload;
    /// the frontend keys quotes by `payload.symbol`, so the serialized JSON
    /// must contain a `symbol` field (regression test for the missing-symbol bug).
    #[test]
    fn test_quote_update_event_json_contains_symbol() {
        let json = serde_json::to_value(QuoteResponse::from(&sample_quote(15005))).unwrap();

        assert_eq!(json["symbol"], "AAPL");
        assert_eq!(json["bid"], 150.0);
        assert_eq!(json["ask"], 150.10);
        assert_eq!(json["last"], 150.05);
        assert_eq!(json["timestamp"], "2026-07-06T14:30:00+00:00");
    }
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
