import { useEffect, useState } from 'react';
import { useMarketStore } from '../store/marketStore';
import type { QuoteResponse } from '../types';

interface UseMarketDataResult {
  /** Latest known quote for `symbol`, or `undefined` before the first update. */
  quote: QuoteResponse | undefined;
  /** True while the initial subscribe + quote fetch is in flight. */
  initializing: boolean;
  /** Set if the initial subscribe/fetch failed. Live updates may still arrive. */
  error: string | null;
}

/**
 * Subscribes a component to live market data for `symbol` via
 * `marketStore.subscribeToSymbol` on mount, and unsubscribes on unmount or
 * when `symbol` changes. Returns the latest quote for the symbol, tracked
 * reactively from the store.
 *
 * @example
 * ```tsx
 * const { quote, initializing, error } = useMarketData('AAPL');
 * ```
 */
export function useMarketData(symbol: string): UseMarketDataResult {
  const quote = useMarketStore((state) => state.quotes.get(symbol));
  const [initializing, setInitializing] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setInitializing(true);
    setError(null);

    const init = async () => {
      try {
        const { subscribeToSymbol, getLatestQuote } = useMarketStore.getState();
        await subscribeToSymbol(symbol);
        await getLatestQuote(symbol);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load market data');
        }
      } finally {
        if (!cancelled) {
          setInitializing(false);
        }
      }
    };

    init();

    return () => {
      cancelled = true;
      useMarketStore.getState().unsubscribeFromSymbol(symbol);
    };
  }, [symbol]);

  return { quote, initializing, error };
}
