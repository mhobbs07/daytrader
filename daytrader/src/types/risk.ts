/**
 * Risk management types mirroring the Rust IPC response structs in
 * `daytrader/src-tauri/src/commands/risk.rs`.
 *
 * All fields are readonly — IPC payloads are immutable snapshots.
 * Monetary values are `number` (Rust serialises f64 directly in response structs).
 * Date strings are YYYY-MM-DD (NaiveDate::to_string) unless noted otherwise.
 * Timestamps are ISO 8601 strings from DateTime::to_rfc3339().
 *
 * Field names are snake_case throughout, matching the actual JSON produced by
 * `#[derive(Serialize)]` on the Rust response structs (no rename attributes
 * are present, so serde uses the Rust field names verbatim).
 */

/**
 * Response from the `get_risk_state` command.
 * Mirrors `RiskStateResponse` in risk.rs.
 */
export interface RiskStateResponse {
  readonly account_id: string;
  /** @iso8601-date Trading date (YYYY-MM-DD) */
  readonly trading_date: string;
  /** Realised P&L for the current trading day */
  readonly daily_pl: number;
  /** Number of day trades executed in the rolling PDT window */
  readonly day_trade_count: number;
  /** Whether trading has been halted by the risk engine */
  readonly is_trading_halted: boolean;
  /** @iso8601 Timestamp of the last risk state update */
  readonly last_updated: string;
}

/**
 * Response from the `get_risk_limits` command.
 * Mirrors `RiskLimitsResponse` in risk.rs.
 */
export interface RiskLimitsResponse {
  readonly max_daily_loss_dollars: number;
  readonly max_daily_loss_percent: number;
  readonly max_position_size_dollars: number;
  readonly max_position_size_percent: number;
  readonly max_concurrent_positions: number;
  readonly enforce_pdt_rule: boolean;
  /** Minimum equity required to be exempt from PDT day-trade limits */
  readonly pdt_min_equity: number;
  /** Maximum day trades allowed per 5-business-day window when under PDT threshold */
  readonly max_day_trades_under_pdt: number;
}

/**
 * Single violation entry from the `get_recent_risk_violations` command.
 * Mirrors `RiskViolationResponse` in risk.rs.
 */
export interface RiskViolationResponse {
  readonly violation_type: string;
  readonly message: string;
  /** @iso8601 When the violation occurred */
  readonly timestamp: string;
  readonly severity: string;
  readonly suggested_action?: string;
  readonly pattern_name?: string;
}

/**
 * Payload pushed on the `risk:violation` Tauri event.
 * Mirrors `daytrader::events::types::RiskViolation`, which is a different
 * (smaller) shape than `RiskViolationResponse` above — it has no `severity`
 * field and `violation_type` is the bare `RiskViolationType` enum tag
 * (e.g. "DailyLossLimit", "PdtLimit"), not the free-form string used by the
 * violations-list command response.
 */
export interface RiskViolationEventPayload {
  readonly violation_type: string;
  readonly message: string;
  /** @iso8601 When the violation occurred */
  readonly timestamp: string;
  readonly pattern_name: string | null;
}

/**
 * Response from the `get_pdt_status` command.
 * Mirrors `PdtStatusResponse` in risk.rs.
 */
export interface PdtStatusResponse {
  readonly account_id: string;
  readonly current_equity: number;
  readonly pdt_min_equity: number;
  readonly meets_equity_requirement: boolean;
  readonly day_trade_count: number;
  /** Maximum day trades allowed. u32::MAX (4294967295) means unlimited. */
  readonly max_day_trades_allowed: number;
  /** Remaining day trades. u32::MAX means unlimited. */
  readonly remaining_day_trades: number;
  readonly is_restricted: boolean;
  /** @iso8601-date Rolling window start date (YYYY-MM-DD) */
  readonly rolling_window_start: string;
  /** @iso8601-date Rolling window end date (YYYY-MM-DD) */
  readonly rolling_window_end: string;
  readonly business_days_remaining: number;
  readonly warnings: readonly string[];
}

/**
 * Response from the `check_pdt_eligibility` command.
 * Mirrors `PdtEligibilityResponse` in risk.rs.
 */
export interface PdtEligibilityResponse {
  readonly eligible: boolean;
  readonly reason: string;
  readonly current_day_trades: number;
  /** u32::MAX (4294967295) means unlimited. */
  readonly max_allowed: number;
  readonly equity_requirement_met: boolean;
  readonly would_be_day_trade: boolean;
  readonly suggested_action?: string;
}

/**
 * Single day-trade record within `DayTradeHistoryResponse`.
 * Mirrors `DayTradeRecordResponse` in risk.rs.
 */
export interface DayTradeRecordResponse {
  readonly trade_id: string;
  readonly symbol: string;
  readonly quantity: number;
  /** @iso8601 When the position was opened */
  readonly open_time: string;
  /** @iso8601 When the position was closed */
  readonly close_time: string;
  readonly open_price: number;
  readonly close_price: number;
  readonly realized_pl: number;
  readonly commission: number;
  readonly pattern_name?: string;
}

/**
 * Response from the `get_day_trade_history` command.
 * Mirrors `DayTradeHistoryResponse` in risk.rs.
 */
export interface DayTradeHistoryResponse {
  readonly trades: readonly DayTradeRecordResponse[];
  readonly total_count: number;
  /** @iso8601-date Window start (YYYY-MM-DD) */
  readonly window_start: string;
  /** @iso8601-date Window end (YYYY-MM-DD) */
  readonly window_end: string;
}

/**
 * Single PDT violation record within `PdtViolationsResponse`.
 * Mirrors `PdtViolationRecord` in risk.rs.
 */
export interface PdtViolationRecord {
  readonly event_type: string;
  /** @iso8601-date Event date (YYYY-MM-DD) */
  readonly event_date: string;
  readonly day_trade_count?: number;
  readonly equity_value: number;
  readonly is_restricted: boolean;
  readonly violation_details?: string;
  readonly message: string;
}

/**
 * Single PDT audit event within `PdtViolationsResponse`.
 * Mirrors `PdtAuditEventResponse` in risk.rs.
 */
export interface PdtAuditEventResponse {
  readonly id: number;
  readonly event_type: string;
  /** @iso8601-date Event date (YYYY-MM-DD) */
  readonly event_date: string;
  readonly day_trade_count?: number;
  readonly equity_value: number;
  readonly is_restricted: boolean;
  readonly message: string;
  /** @iso8601 When the record was created */
  readonly created_at: string;
}

/**
 * Response from the `get_pdt_violations` command.
 * Mirrors `PdtViolationsResponse` in risk.rs.
 */
export interface PdtViolationsResponse {
  readonly violations: readonly PdtViolationRecord[];
  readonly audit_events: readonly PdtAuditEventResponse[];
}

/**
 * Single holiday entry within `BusinessDayInfoResponse`.
 * Mirrors `HolidayInfo` in risk.rs.
 */
export interface HolidayInfo {
  readonly name: string;
  /** @iso8601-date Holiday date (YYYY-MM-DD) */
  readonly date: string;
  readonly is_market_holiday: boolean;
}

/**
 * Response from the `get_business_day_info` command.
 * Mirrors `BusinessDayInfoResponse` in risk.rs.
 */
export interface BusinessDayInfoResponse {
  /** @iso8601-date Current date (YYYY-MM-DD) */
  readonly current_date: string;
  readonly is_business_day: boolean;
  /** @iso8601-date Next business day (YYYY-MM-DD) */
  readonly next_business_day: string;
  /** @iso8601-date Previous business day (YYYY-MM-DD) */
  readonly previous_business_day: string;
  readonly upcoming_holidays: readonly HolidayInfo[];
  readonly business_days_in_current_month: number;
}
