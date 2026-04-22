import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

let patternUnlistener: UnlistenFn | null = null;

interface PatternSignal {
  symbol: string;
  pattern_name: string;
  signal_type: string;
  confidence: number;
  detected_at: string;
  price: number;
}

interface PatternStats {
  total_detections: number;
  patterns_by_type: [string, number][];
  average_confidence: number;
}

interface PatternStore {
  patterns: PatternSignal[];
  statistics: PatternStats | null;
  loadStatistics: () => Promise<void>;
  dispose: () => void;
}

export const usePatternStore = create<PatternStore>((set) => {
  if (!patternUnlistener) {
    (async () => {
      patternUnlistener = await listen<PatternSignal>('patterns:detected', (event) => {
        usePatternStore.setState(state => ({
          patterns: [event.payload, ...state.patterns].slice(0, 100)
        }));
      });
    })();
  }

  return {
    patterns: [],
    statistics: null,

    loadStatistics: async () => {
      const stats = await invoke<PatternStats>('get_pattern_statistics');
      set({ statistics: stats });
    },

    dispose: () => {
      if (patternUnlistener) {
        patternUnlistener();
        patternUnlistener = null;
      }
    }
  };
});