import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  RiskStateResponse,
  RiskLimitsResponse,
  RiskViolationResponse,
  RiskViolationEventPayload,
  PdtStatusResponse,
} from '../types';

let riskIntervalId: number | null = null;
let riskViolationUnlisten: UnlistenFn | null = null;

interface RiskStore {
  riskState: RiskStateResponse | null;
  riskLimits: RiskLimitsResponse | null;
  recentViolations: RiskViolationResponse[];
  pdtStatus: PdtStatusResponse | null;
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
      const riskState = await invoke<RiskStateResponse>('get_risk_state');
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
      const riskLimits = await invoke<RiskLimitsResponse>('get_risk_limits');
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
      const recentViolations = await invoke<RiskViolationResponse[]>('get_recent_risk_violations');
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
      const pdtStatus = await invoke<PdtStatusResponse>('get_pdt_status');
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

    riskViolationUnlisten = await listen<RiskViolationEventPayload>('risk:violation', () => {
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
