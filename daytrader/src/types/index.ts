/**
 * Central re-export for all TypeScript IPC types.
 *
 * Import from this module to get any type used in Tauri command calls or
 * event listeners:
 *
 * ```ts
 * import type { SystemStatus, QuoteResponse, PatternSignalResponse } from '../types';
 * ```
 */

export type {
  QuoteResponse,
  CandleResponse,
  QuoteUpdateEventPayload,
  CandleUpdateEventPayload,
} from './market';

export type {
  SignalType,
  PatternSignalResponse,
  PatternSignalEventPayload,
  PatternStats,
} from './patterns';

export type { SystemStatus } from './system';

export type {
  RiskStateResponse,
  RiskLimitsResponse,
  RiskViolationResponse,
  RiskViolationEventPayload,
  PdtStatusResponse,
  PdtEligibilityResponse,
  DayTradeRecordResponse,
  DayTradeHistoryResponse,
  PdtViolationRecord,
  PdtAuditEventResponse,
  PdtViolationsResponse,
  HolidayInfo,
  BusinessDayInfoResponse,
} from './risk';

export type {
  ExecutionStatusResponse,
  OrderResponse,
  PositionResponse,
} from './execution';
