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
 * Response element from the `get_detected_patterns_for_symbol` command, and
 * also the payload pushed on the `patterns:detected` Tauri event (the event
 * bridge emits the same `PatternSignal` domain type this response mirrors).
 * Mirrors `PatternSignalResponse` in patterns.rs.
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
