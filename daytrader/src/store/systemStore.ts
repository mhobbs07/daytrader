import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

interface SystemStatus {
  broker_connected: boolean;
  event_bus_active: boolean;
  patterns_active: number;
  events_published: number;
  events_dropped: number;
  average_latency_ms: number;
}

interface SystemStore {
  status: SystemStatus | null;
  error: string | null;
  refreshStatus: () => Promise<void>;
}

export const useSystemStore = create<SystemStore>((set) => ({
  status: null,
  error: null,

  refreshStatus: async () => {
    try {
      const status = await invoke<SystemStatus>('get_system_status');
      set({ status, error: null });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to fetch system status' });
    }
  },
}));

// Fetch immediately on module load, then poll every 5 seconds
useSystemStore.getState().refreshStatus();
setInterval(() => {
  useSystemStore.getState().refreshStatus();
}, 5000);