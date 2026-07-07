import { useMemo } from 'react';
import { usePatternStore } from '../store/patternStore';
import type { PatternSignalResponse, SignalType } from '../types';

export interface PatternFilters {
  /** Case-insensitive substring match against `symbol`. */
  symbol?: string;
  /** Restrict to a single signal direction. */
  signalType?: SignalType;
  /** Minimum confidence in [0, 1], inclusive. */
  minConfidence?: number;
}

/**
 * Reads live-detected patterns from `patternStore` (populated by the
 * `patterns:detected` Tauri event) and applies optional client-side filters.
 *
 * Filtering is memoized on `patterns` and the individual filter values, so
 * passing a fresh `filters` object literal on every render does not defeat
 * memoization.
 *
 * @example
 * ```tsx
 * const patterns = usePatterns({ signalType: 'Buy', minConfidence: 0.7 });
 * ```
 */
export function usePatterns(filters?: PatternFilters): PatternSignalResponse[] {
  const patterns = usePatternStore((state) => state.patterns);
  const symbolFilter = filters?.symbol?.toLowerCase() ?? '';
  const signalTypeFilter = filters?.signalType;
  const minConfidence = filters?.minConfidence ?? 0;

  return useMemo(() => {
    return patterns.filter((pattern) => {
      if (symbolFilter && !pattern.symbol.toLowerCase().includes(symbolFilter)) {
        return false;
      }
      if (signalTypeFilter && pattern.signal_type !== signalTypeFilter) {
        return false;
      }
      if (pattern.confidence < minConfidence) {
        return false;
      }
      return true;
    });
  }, [patterns, symbolFilter, signalTypeFilter, minConfidence]);
}
