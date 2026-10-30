/**
 * Trading execution types mirroring the Rust IPC response structs in
 * `daytrader/src-tauri/src/commands/execution.rs`.
 *
 * All fields are readonly — IPC payloads are immutable snapshots.
 * Monetary values are `number` (Rust serialises f64 directly in response structs).
 * Timestamps are ISO 8601 strings from DateTime::to_rfc3339().
 *
 * Field names are snake_case throughout, matching the actual JSON produced by
 * `#[derive(Serialize)]` on the Rust response structs (no rename attributes
 * are present, so serde uses the Rust field names verbatim).
 */

/**
 * Response from the `get_execution_status` command.
 * Mirrors `ExecutionStatusResponse` in execution.rs.
 */
export interface ExecutionStatusResponse {
  /** False when the app started without a database and the trading loop is inert */
  readonly engines_available: boolean;
  readonly automation_enabled: boolean;
  readonly total_signals: number;
  readonly auto_executed: number;
  readonly manual_review: number;
  readonly rejected: number;
  readonly avg_exec_time_ms: number;
}

/**
 * A single order, active or terminal, from `get_active_orders` / `get_recent_orders`.
 * Mirrors `OrderResponse` in execution.rs.
 */
export interface OrderResponse {
  readonly id: string;
  readonly symbol: string;
  readonly side: string;
  readonly order_type: string;
  readonly quantity: number;
  readonly limit_price: number | null;
  readonly status: string;
  readonly filled_quantity: number;
  readonly filled_avg_price: number | null;
  readonly pattern_name: string | null;
  /** @iso8601 */
  readonly created_at: string;
}

/**
 * A single open position from `get_open_positions`.
 * Mirrors `PositionResponse` in execution.rs.
 */
export interface PositionResponse {
  readonly symbol: string;
  readonly side: string;
  readonly quantity: number;
  readonly avg_entry_price: number;
  readonly current_price: number;
  readonly unrealized_pl: number;
  readonly unrealized_pl_percent: number;
  readonly realized_pl: number;
  readonly pattern_name: string | null;
  /** @iso8601 */
  readonly opened_at: string;
}
