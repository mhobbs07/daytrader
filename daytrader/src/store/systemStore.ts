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
  refreshStatus: () => Promise<void>;
}

export const useSystemStore = create<SystemStore>((set) => ({
  status: null,

  refreshStatus: async () => {
    const status = await invoke<SystemStatus>('get_system_status');
    set({ status });
  },
}));

// Poll status every 5 seconds
setInterval(() => {
  useSystemStore.getState().refreshStatus();
}, 5000);