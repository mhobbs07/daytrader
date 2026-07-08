import { create } from 'zustand';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { PatternSignalResponse, PatternSignalEventPayload } from '../types';

const unlisteners: UnlistenFn[] = [];

interface PatternStore {
  patterns: PatternSignalResponse[];
  dispose: () => void;
}

export const usePatternStore = create<PatternStore>(() => {
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

    dispose: () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
      unlisteners.length = 0;
    }
  };
});
