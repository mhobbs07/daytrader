import React, { useEffect } from 'react';
import { useMarketStore } from '../store/marketStore';

export const MarketData: React.FC<{ symbol: string }> = ({ symbol }) => {
  const { quotes, getLatestQuote, subscribeToSymbol } = useMarketStore();
  const quote = quotes.get(symbol);

  useEffect(() => {
    subscribeToSymbol(symbol);
    getLatestQuote(symbol);
  }, [symbol]);

  if (!quote) return <div>Loading {symbol}...</div>;

  return (
    <div className="market-data-card">
      <h3>{symbol}</h3>
      <div className="quote-details">
        <div>Bid: ${quote.bid.toFixed(2)}</div>
        <div>Ask: ${quote.ask.toFixed(2)}</div>
        <div className="timestamp">{new Date(quote.timestamp).toLocaleTimeString()}</div>
      </div>
    </div>
  );
};