/**
 * Pattern detection types mirroring the Rust IPC response structs in
 * `daytrader/src-tauri/src/commands/patterns.rs`.
 *
 * All fields are readonly — IPC payloads are immutable snapshots.
 */

/**
 * Signal direction returned for a detected pattern.
 * Mirrors the `signal_type` string produced from the `SignalType` enum in
 * `src/events/types.rs` (serialised as "Buy" or "Sell").
 */
export type SignalType = 'Buy' | 'Sell';

/**
 * Response element from the `get_detected_patterns_for_symbol` command.
 * Mirrors `PatternSignalResponse` in patterns.rs, which explicitly converts
 * `Confidence`/`Price` newtypes to `f64` before returning — both fields are
 * plain JSON numbers on this wire shape.
 */
export interface PatternSignalResponse {
  /** Ticker symbol */
  readonly symbol: string;
  /** Human-readable pattern name, e.g. "Hammer" */
  readonly pattern_name: string;
  /** Trade direction: "Buy" or "Sell" */
  readonly signal_type: SignalType;
  /** Confidence score in [0, 1] */
  readonly confidence: number;
  /** @iso8601 When the pattern was detected */
  readonly detected_at: string;
  /** Price at detection time */
  readonly price: number;
}

/**
 * Payload pushed on the `patterns:detected` Tauri event.
 *
 * NOTE: unlike `PatternSignalResponse` above, the event bridge
 * (`daytrader/src-tauri/src/events.rs`) emits the raw domain
 * `daytrader::events::types::PatternSignal` directly
 * (`app_handle.emit("patterns:detected", &signal)`) rather than through the
 * `patterns.rs` command's hand-built response struct. That domain type keeps
 * `price` as a `Price(Decimal)` newtype, which `rust_decimal`'s default
 * serde impl serialises as a JSON **string**, not a number — `confidence`
 * (`Confidence(f64)`) and `symbol`/`pattern_name` (single-field newtypes
 * over `String`) serialise transparently as a number and strings
 * respectively, matching `PatternSignalResponse`. Only `price` differs.
 * Consumers must `Number(...)`/`parseFloat` the `price` field.
 */
export interface PatternSignalEventPayload {
  readonly symbol: string;
  readonly pattern_name: string;
  readonly signal_type: SignalType;
  readonly confidence: number;
  /** @iso8601 When the pattern was detected */
  readonly detected_at: string;
  /** Decimal serialised as a string, e.g. "150.50" — see note above. */
  readonly price: string;
}

/**
 * Response from the `get_pattern_statistics` command.
 * Mirrors `PatternStats` in patterns.rs.
 */
export interface PatternStats {
  /** Total number of patterns detected across all symbols */
  readonly total_detections: number;
  /**
   * Per-pattern detection counts.
   * Each tuple is [pattern_name, count].
   */
  readonly patterns_by_type: ReadonlyArray<readonly [string, number]>;
  /** Average confidence across all detected patterns */
  readonly average_confidence: number;
}
