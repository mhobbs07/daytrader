import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { QuoteResponse, CandleResponse, CandleUpdateEventPayload } from '../types';

const unlisteners = new Map<string, UnlistenFn[]>();

interface MarketStore {
  quotes: Map<string, QuoteResponse>;
  candles: Map<string, CandleResponse[]>;
  subscribeToSymbol: (symbol: string) => Promise<void>;
  unsubscribeFromSymbol: (symbol: string) => void;
  dispose: () => void;
  getLatestQuote: (symbol: string) => Promise<QuoteResponse>;
  getRecentCandles: (symbol: string, count: number) => Promise<CandleResponse[]>;
}

export const useMarketStore = create<MarketStore>((set, _get) => ({
  quotes: new Map(),
  candles: new Map(),

  subscribeToSymbol: async (symbol: string) => {
    if (unlisteners.has(symbol)) {
      for (const unlisten of unlisteners.get(symbol)!) {
        unlisten();
      }
      unlisteners.delete(symbol);
    }

    await invoke('subscribe_symbol', { symbol });

    const handlers: UnlistenFn[] = [];

    // NOTE: the `market_data:quote_update` event payload does not actually
    // include `symbol` on the wire (see QuoteUpdateEventPayload in
    // ../types/market.ts) — this cast reflects the pre-existing assumption
    // in this handler, not a verified wire shape.
    handlers.push(await listen<QuoteResponse>('market_data:quote_update', (event) => {
      set(state => {
        const newQuotes = new Map(state.quotes);
        newQuotes.set(event.payload.symbol, event.payload);
        return { quotes: newQuotes };
      });
    }));

    handlers.push(await listen<CandleUpdateEventPayload>('market_data:new_candle', (event) => {
      set(state => {
        const newCandles = new Map(state.candles);
        const symbolCandles = newCandles.get(event.payload.symbol) || [];
        newCandles.set(event.payload.symbol, [...symbolCandles, event.payload]);
        return { candles: newCandles };
      });
    }));

    unlisteners.set(symbol, handlers);
  },

  unsubscribeFromSymbol: (symbol: string) => {
    if (unlisteners.has(symbol)) {
      for (const unlisten of unlisteners.get(symbol)!) {
        unlisten();
      }
      unlisteners.delete(symbol);
    }
  },

  dispose: () => {
    for (const [, handlers] of unlisteners) {
      for (const unlisten of handlers) {
        unlisten();
      }
    }
    unlisteners.clear();
  },

  getLatestQuote: async (symbol: string) => {
    return invoke<QuoteResponse>('get_latest_quote', { symbol });
  },

  getRecentCandles: async (symbol: string, count: number) => {
    return invoke<CandleResponse[]>('get_recent_candles', { symbol, count });
  },
}));