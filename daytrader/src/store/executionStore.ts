import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  ExecutionStatusResponse,
  OrderResponse,
  PositionResponse,
} from '../types';

let executionIntervalId: number | null = null;

interface ExecutionStore {
  status: ExecutionStatusResponse | null;
  activeOrders: OrderResponse[];
  recentOrders: OrderResponse[];
  positions: PositionResponse[];
  loading: boolean;
  error: string | null;

  // Actions
  fetchStatus: () => Promise<void>;
  fetchActiveOrders: () => Promise<void>;
  fetchRecentOrders: () => Promise<void>;
  fetchPositions: () => Promise<void>;
  refreshAll: () => Promise<void>;
  setAutomation: (enabled: boolean) => Promise<void>;
  dispose: () => void;
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  status: null,
  activeOrders: [],
  recentOrders: [],
  positions: [],
  loading: false,
  error: null,

  fetchStatus: async () => {
    try {
      const status = await invoke<ExecutionStatusResponse>('get_execution_status');
      set({ status, error: null });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to fetch execution status' });
    }
  },

  fetchActiveOrders: async () => {
    try {
      const activeOrders = await invoke<OrderResponse[]>('get_active_orders');
      set({ activeOrders, error: null });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to fetch active orders' });
    }
  },

  fetchRecentOrders: async () => {
    try {
      const recentOrders = await invoke<OrderResponse[]>('get_recent_orders');
      set({ recentOrders, error: null });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to fetch recent orders' });
    }
  },

  fetchPositions: async () => {
    try {
      const positions = await invoke<PositionResponse[]>('get_open_positions');
      set({ positions, error: null });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to fetch open positions' });
    }
  },

  refreshAll: async () => {
    const { fetchStatus, fetchActiveOrders, fetchRecentOrders, fetchPositions } = get();
    set({ loading: true });
    await Promise.all([
      fetchStatus(),
      fetchActiveOrders(),
      fetchRecentOrders(),
      fetchPositions(),
    ]);
    set({ loading: false });
  },

  setAutomation: async (enabled: boolean) => {
    try {
      await invoke<boolean>('set_automation_enabled', { enabled });
      await get().fetchStatus();
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to toggle automation' });
    }
  },

  dispose: () => {
    if (executionIntervalId !== null) {
      clearInterval(executionIntervalId);
      executionIntervalId = null;
    }
  },
}));

// Start 5-second polling (v1 — no push events for execution data yet)
useExecutionStore.getState().refreshAll();
executionIntervalId = window.setInterval(() => {
  useExecutionStore.getState().refreshAll();
}, 5000);
