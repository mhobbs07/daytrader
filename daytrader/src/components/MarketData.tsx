import React, { useEffect, useState } from 'react';
import { useMarketStore } from '../store/marketStore';

export const MarketData: React.FC<{ symbol: string }> = ({ symbol }) => {
  const { quotes, getLatestQuote, subscribeToSymbol } = useMarketStore();
  const [error, setError] = useState<string | null>(null);
  const [initializing, setInitializing] = useState(true);
  const quote = quotes.get(symbol);

  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      try {
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
    return () => { cancelled = true; };
  }, [symbol]);

  if (initializing && !quote) {
    return (
      <div className="market-data-card">
        <h3>{symbol}</h3>
        <div className="market-data-loading loading-pulse">Loading market data…</div>
      </div>
    );
  }

  if (error && !quote) {
    return (
      <div className="market-data-card">
        <h3>{symbol}</h3>
        <div className="market-data-error">
          {error}
          <br />
          <small>Waiting for live data…</small>
        </div>
      </div>
    );
  }

  if (!quote) {
    return (
      <div className="market-data-card">
        <h3>{symbol}</h3>
        <div className="market-data-loading">Waiting for quote data…</div>
      </div>
    );
  }

  return (
    <div className="market-data-card">
      <h3>{symbol}</h3>
      <div className="quote-details">
        <div className="quote-item">
          <div className="quote-label">Bid</div>
          <div className="quote-value">${quote.bid.toFixed(2)}</div>
        </div>
        <div className="quote-item">
          <div className="quote-label">Last</div>
          <div className="quote-value">${quote.last.toFixed(2)}</div>
        </div>
        <div className="quote-item">
          <div className="quote-label">Ask</div>
          <div className="quote-value">${quote.ask.toFixed(2)}</div>
        </div>
        <div className="timestamp">
          Updated: {new Date(quote.timestamp).toLocaleTimeString()}
        </div>
      </div>
    </div>
  );
};
