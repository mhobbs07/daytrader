import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  QuoteResponse,
  CandleResponse,
  QuoteUpdateEventPayload,
  CandleUpdateEventPayload,
} from '../types';

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

    // The `market_data:quote_update` payload does not include `symbol` on
    // the wire (see QuoteUpdateEventPayload in ../types/market.ts), so we
    // key the update by the symbol this listener was registered for rather
    // than reading it off the payload. A concurrent fix to events.rs may add
    // `symbol` to the payload later, at which point this can switch back to
    // reading it from the event.
    handlers.push(await listen<QuoteUpdateEventPayload>('market_data:quote_update', (event) => {
      const quote: QuoteResponse = {
        symbol,
        bid: Number(event.payload.bid),
        ask: Number(event.payload.ask),
        last: Number(event.payload.bid),
        timestamp: event.payload.timestamp,
      };
      set(state => {
        const newQuotes = new Map(state.quotes);
        newQuotes.set(symbol, quote);
        return { quotes: newQuotes };
      });
    }));

    // `open`/`high`/`low`/`close` arrive as Decimal-serialised strings (see
    // CandleUpdateEventPayload) and must be converted to numbers to match
    // CandleResponse, the shape the rest of the app (CandleChart) expects.
    handlers.push(await listen<CandleUpdateEventPayload>('market_data:new_candle', (event) => {
      const candle: CandleResponse = {
        symbol: event.payload.symbol,
        timestamp: event.payload.timestamp,
        open: Number(event.payload.open),
        high: Number(event.payload.high),
        low: Number(event.payload.low),
        close: Number(event.payload.close),
        volume: event.payload.volume,
      };
      set(state => {
        const newCandles = new Map(state.candles);
        const symbolCandles = newCandles.get(candle.symbol) || [];
        newCandles.set(candle.symbol, [...symbolCandles, candle]);
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