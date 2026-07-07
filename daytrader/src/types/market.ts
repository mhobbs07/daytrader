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
 * NOT include `symbol` on the wire — marketStore must key updates by the
 * symbol it subscribed for, not by a field on the payload. A concurrent
 * session is fixing this gap in events.rs; once `symbol` is added to the
 * wire payload, this type should gain a `readonly symbol: string` field.
 *
 * `bid`/`ask` are `Price(Decimal)` newtypes. `rust_decimal`'s default serde
 * impl (no `serde-float` feature) serialises `Decimal` as a JSON **string**
 * (e.g. `"150.50"`), not a number — unlike `QuoteResponse` above, which the
 * Tauri command explicitly converts to `f64` before returning. Consumers
 * must `Number(...)`/`parseFloat` these fields.
 */
export interface QuoteUpdateEventPayload {
  readonly timestamp: string;
  /** Decimal serialised as a string, e.g. "150.50" — see note above. */
  readonly bid: string;
  /** Decimal serialised as a string, e.g. "150.55" — see note above. */
  readonly ask: string;
  readonly bid_size: number;
  readonly ask_size: number;
}

/**
 * Payload pushed on the `market_data:new_candle` Tauri event.
 * Mirrors `daytrader::brokers::traits::Candle` (re-exported as
 * `events::types::Candle`), which the event bridge emits *directly*
 * (`app_handle.emit("market_data:new_candle", &*candle)`) rather than
 * through a hand-built response struct.
 *
 * Unlike `CandleResponse` (produced by the `get_recent_candles` command,
 * which explicitly converts each `Price` to `f64`), the raw `Candle` struct
 * has `open`/`high`/`low`/`close` typed as `Price(Decimal)`. `rust_decimal`'s
 * default serde impl serialises `Decimal` as a JSON **string**, not a
 * number. `volume: u64` serialises as a plain number (no wrapper). Consumers
 * must `Number(...)`/`parseFloat` the OHLC fields before using them.
 */
export interface CandleUpdateEventPayload {
  /** Ticker symbol */
  readonly symbol: string;
  /** @iso8601 Candle open time */
  readonly timestamp: string;
  /** Decimal serialised as a string — see note above. */
  readonly open: string;
  /** Decimal serialised as a string — see note above. */
  readonly high: string;
  /** Decimal serialised as a string — see note above. */
  readonly low: string;
  /** Decimal serialised as a string — see note above. */
  readonly close: string;
  readonly volume: number;
}
