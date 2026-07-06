import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface RiskState {
  accountId: string;
  tradingDate: string;
  dailyPl: number;
  dayTradeCount: number;
  isTradingHalted: boolean;
  lastUpdated: string;
}

interface RiskLimits {
  maxDailyLossDollars: number;
  maxDailyLossPercent: number;
  maxPositionSizeDollars: number;
  maxPositionSizePercent: number;
  maxConcurrentPositions: number;
  enforcePdtRule: boolean;
  pdtMinEquity: number;
  maxDayTradesUnderPdt: number;
}

interface RiskViolation {
  violationType: string;
  message: string;
  timestamp: string;
  severity: string;
  suggestedAction?: string;
}

interface PdtStatus {
  accountId: string;
  currentEquity: number;
  pdtMinEquity: number;
  meetsEquityRequirement: boolean;
  dayTradeCount: number;
  maxDayTradesAllowed: number;
  remainingDayTrades: number;
  isRestricted: boolean;
  rollingWindowStart: string;
  rollingWindowEnd: string;
  businessDaysRemaining: number;
  warnings: string[];
}

/** Payload emitted by the Rust EventBridge on `risk:violation`. */
interface RiskViolationPayload {
  violation_type: string;
  message: string;
  timestamp: string;
  pattern_name: string | null;
}

let riskIntervalId: number | null = null;
let riskViolationUnlisten: UnlistenFn | null = null;

interface RiskStore {
  riskState: RiskState | null;
  riskLimits: RiskLimits | null;
  recentViolations: RiskViolation[];
  pdtStatus: PdtStatus | null;
  loading: boolean;
  error: string | null;

  // Actions
  fetchRiskState: () => Promise<void>;
  fetchRiskLimits: () => Promise<void>;
  fetchRecentViolations: () => Promise<void>;
  fetchPdtStatus: () => Promise<void>;
  refreshAll: () => Promise<void>;
  listenForViolations: () => Promise<void>;
  dispose: () => void;
}

export const useRiskStore = create<RiskStore>((set, get) => ({
  riskState: null,
  riskLimits: null,
  recentViolations: [],
  pdtStatus: null,
  loading: false,
  error: null,

  fetchRiskState: async () => {
    try {
      set({ loading: true, error: null });
      const riskState = await invoke<RiskState>('get_risk_state');
      set({ riskState, loading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch risk state',
        loading: false
      });
    }
  },

  fetchRiskLimits: async () => {
    try {
      set({ loading: true, error: null });
      const riskLimits = await invoke<RiskLimits>('get_risk_limits');
      set({ riskLimits, loading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch risk limits',
        loading: false
      });
    }
  },

  fetchRecentViolations: async () => {
    try {
      set({ loading: true, error: null });
      const recentViolations = await invoke<RiskViolation[]>('get_recent_risk_violations');
      set({ recentViolations, loading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch risk violations',
        loading: false
      });
    }
  },

  fetchPdtStatus: async () => {
    try {
      set({ loading: true, error: null });
      const pdtStatus = await invoke<PdtStatus>('get_pdt_status');
      set({ pdtStatus, loading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch PDT status',
        loading: false
      });
    }
  },

  refreshAll: async () => {
    const { fetchRiskState, fetchRiskLimits, fetchRecentViolations, fetchPdtStatus } = get();
    await Promise.all([
      fetchRiskState(),
      fetchRiskLimits(),
      fetchRecentViolations(),
      fetchPdtStatus()
    ]);
  },

  listenForViolations: async () => {
    // Remove any existing listener before registering a new one
    if (riskViolationUnlisten !== null) {
      riskViolationUnlisten();
      riskViolationUnlisten = null;
    }

    riskViolationUnlisten = await listen<RiskViolationPayload>('risk:violation', () => {
      // A violation just arrived — immediately refresh all risk data
      useRiskStore.getState().refreshAll();
    });
  },

  dispose: () => {
    if (riskIntervalId !== null) {
      clearInterval(riskIntervalId);
      riskIntervalId = null;
    }
    if (riskViolationUnlisten !== null) {
      riskViolationUnlisten();
      riskViolationUnlisten = null;
    }
  },
}));

// Start 30-second polling fallback
riskIntervalId = window.setInterval(() => {
  useRiskStore.getState().refreshAll();
}, 30000);

// Wire up the push-based risk:violation listener
useRiskStore.getState().listenForViolations();