import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { PatternSignalResponse, PatternSignalEventPayload, PatternStats } from '../types';

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
      // `price` arrives as a Decimal-serialised string (see
      // PatternSignalEventPayload) and must be converted to a number to
      // match PatternSignalResponse, the shape PatternList expects.
      const unlisten = await listen<PatternSignalEventPayload>('patterns:detected', (event) => {
        const signal: PatternSignalResponse = {
          ...event.payload,
          price: Number(event.payload.price),
        };
        usePatternStore.setState(state => ({
          patterns: [signal, ...state.patterns].slice(0, 100)
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