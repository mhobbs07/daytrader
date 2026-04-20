import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Unlisten functions for cleanup
// let unlisteners: (() => void)[] = [];

interface Quote {
  symbol: string;
  bid: number;
  ask: number;
  /** Last traded price (midpoint fallback when no trade data). */
  last: number;
  timestamp: string;
}

interface Candle {
  symbol: string;
  timestamp: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

interface MarketStore {
  quotes: Map<string, Quote>;
  candles: Map<string, Candle[]>;
  subscribeToSymbol: (symbol: string) => Promise<void>;
  getLatestQuote: (symbol: string) => Promise<Quote>;
  getRecentCandles: (symbol: string, count: number) => Promise<Candle[]>;
}

export const useMarketStore = create<MarketStore>((set, _get) => ({
  quotes: new Map(),
  candles: new Map(),

  subscribeToSymbol: async (symbol: string) => {
    await invoke('subscribe_symbol', { symbol });

    // Listen for updates
    listen<Quote>('market_data:quote_update', (event) => {
      set(state => {
        const newQuotes = new Map(state.quotes);
        newQuotes.set(event.payload.symbol, event.payload);
        return { quotes: newQuotes };
      });
    });

    listen<Candle>('market_data:new_candle', (event) => {
      set(state => {
        const newCandles = new Map(state.candles);
        const symbolCandles = newCandles.get(event.payload.symbol) || [];
        newCandles.set(event.payload.symbol, [...symbolCandles, event.payload]);
        return { candles: newCandles };
      });
    });
  },

  getLatestQuote: async (symbol: string) => {
    return invoke<Quote>('get_latest_quote', { symbol });
  },

  getRecentCandles: async (symbol: string, count: number) => {
    return invoke<Candle[]>('get_recent_candles', { symbol, count });
  },
}));