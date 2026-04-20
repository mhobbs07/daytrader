import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

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

let riskIntervalId: number | null = null;

interface RiskStore {
  riskState: RiskState | null;
  riskLimits: RiskLimits | null;
  recentViolations: RiskViolation[];
  loading: boolean;
  error: string | null;

  // Actions
  fetchRiskState: () => Promise<void>;
  fetchRiskLimits: () => Promise<void>;
  fetchRecentViolations: () => Promise<void>;
  refreshAll: () => Promise<void>;
  dispose: () => void;
}

export const useRiskStore = create<RiskStore>((set, get) => ({
  riskState: null,
  riskLimits: null,
  recentViolations: [],
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

  refreshAll: async () => {
    const { fetchRiskState, fetchRiskLimits, fetchRecentViolations } = get();
    await Promise.all([
      fetchRiskState(),
      fetchRiskLimits(),
      fetchRecentViolations()
    ]);
  },

  dispose: () => {
    if (riskIntervalId !== null) {
      clearInterval(riskIntervalId);
      riskIntervalId = null;
    }
  },
}));

riskIntervalId = window.setInterval(() => {
  useRiskStore.getState().refreshAll();
}, 30000);