/**
 * Market data types mirroring the Rust IPC response structs in
 * `daytrader/src-tauri/src/commands/market_data.rs`.
 *
 * All fields are readonly — IPC payloads are immutable snapshots.
 * Numeric price/volume fields are `number` because Rust serialises f64/u64
 * directly to JSON numbers here (no Decimal wrapper in the response structs).
 * Timestamps are ISO 8601 strings produced by `DateTime::to_rfc3339()`.
 */

/**
 * Response from the `get_latest_quote` command.
 * Mirrors `QuoteResponse` in market_data.rs.
 */
export interface QuoteResponse {
  /** Ticker symbol, e.g. "AAPL" */
  readonly symbol: string;
  /** Current bid price */
  readonly bid: number;
  /** Current ask price */
  readonly ask: number;
  /**
   * Last traded price.
   * Falls back to midpoint when no last-trade data is available.
   */
  readonly last: number;
  /** @iso8601 Timestamp of the quote */
  readonly timestamp: string;
}

/**
 * Response element from the `get_recent_candles` command.
 * Mirrors `CandleResponse` in market_data.rs.
 */
export interface CandleResponse {
  /** Ticker symbol */
  readonly symbol: string;
  /** @iso8601 Candle open time */
  readonly timestamp: string;
  readonly open: number;
  readonly high: number;
  readonly low: number;
  readonly close: number;
  readonly volume: number;
}

/**
 * Payload pushed on the `market_data:quote_update` Tauri event.
 *
 * NOTE: this mirrors `daytrader::events::types::Quote` (src/events/types.rs),
 * which is a *different* struct from the one backing `QuoteResponse` above —
 * the event bridge (`daytrader/src-tauri/src/events.rs`) emits the inner
 * `Quote` value from `MarketEvent::QuoteUpdate { symbol, quote }` without the
 * `symbol` field attached. Until that backend gap is fixed, this payload does
 * NOT include `symbol` on the wire even though marketStore keys the update by
 * `event.payload.symbol`. Kept separate from `QuoteResponse` so this
 * discrepancy is visible at the type level rather than silently assumed.
 */
export interface QuoteUpdateEventPayload {
  readonly timestamp: string;
  readonly bid: number;
  readonly ask: number;
  readonly bid_size: number;
  readonly ask_size: number;
}

/**
 * Payload pushed on the `market_data:new_candle` Tauri event.
 * Mirrors `daytrader::brokers::traits::Candle`, re-exported as
 * `events::types::Candle`. Field-for-field identical to `CandleResponse`.
 */
export type CandleUpdateEventPayload = CandleResponse;
