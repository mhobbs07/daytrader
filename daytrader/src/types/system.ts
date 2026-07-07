/**
 * System status types mirroring the Rust IPC response structs in
 * `daytrader/src-tauri/src/commands/system.rs`.
 *
 * All fields are readonly — IPC payloads are immutable snapshots.
 */

/**
 * Response from the `get_system_status` command.
 * Mirrors `SystemStatus` in system.rs.
 */
export interface SystemStatus {
  /** Whether the broker connection is currently active */
  readonly broker_connected: boolean;
  /** Whether the internal event bus has active subscribers */
  readonly event_bus_active: boolean;
  /** Number of pattern detectors currently registered */
  readonly patterns_active: number;
  /** Total events published on the event bus since startup */
  readonly events_published: number;
  /** Total events dropped (slow consumers) since startup */
  readonly events_dropped: number;
  /** Rolling average event processing latency in milliseconds */
  readonly average_latency_ms: number;
}
