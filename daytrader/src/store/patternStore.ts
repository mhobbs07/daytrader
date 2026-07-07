import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { PatternSignalResponse, PatternStats } from '../types';

const unlisteners: UnlistenFn[] = [];

interface PatternStore {
  patterns: PatternSignalResponse[];
  statistics: PatternStats | null;
  loadStatistics: () => Promise<void>;
  dispose: () => void;
}

export const usePatternStore = create<PatternStore>((set) => {
  // Initialize listener at store creation
  (async () => {
    if (unlisteners.length === 0) {
      const unlisten = await listen<PatternSignalResponse>('patterns:detected', (event) => {
        usePatternStore.setState(state => ({
          patterns: [event.payload, ...state.patterns].slice(0, 100)
        }));
      });
      unlisteners.push(unlisten);
    }
  })();

  return {
    patterns: [],
    statistics: null,

    loadStatistics: async () => {
      const stats = await invoke<PatternStats>('get_pattern_statistics');
      set({ statistics: stats });
    },

    dispose: () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
      unlisteners.length = 0;
    }
  };
});