import React, { useState } from 'react';

const SYMBOL_PATTERN = /^[A-Z]{1,5}$/;
const STORAGE_KEY = 'daytrader:watchlist';
const DEFAULT_SYMBOLS = ['AAPL'];

/** Reads the persisted watchlist from localStorage, falling back to the default on any failure. */
export function loadWatchlist(): string[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return DEFAULT_SYMBOLS;
    }
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return DEFAULT_SYMBOLS;
    }
    const symbols = parsed.filter((s): s is string => typeof s === 'string' && SYMBOL_PATTERN.test(s));
    return symbols.length > 0 ? symbols : DEFAULT_SYMBOLS;
  } catch {
    return DEFAULT_SYMBOLS;
  }
}

/** Persists the watchlist to localStorage. Failures (e.g. storage disabled) are non-fatal. */
export function saveWatchlist(symbols: string[]): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(symbols));
  } catch {
    // Ignore — persistence is a nice-to-have, not required for the session to function.
  }
}

interface WatchlistProps {
  symbols: string[];
  onAdd: (symbol: string) => void;
  onRemove: (symbol: string) => void;
}

/**
 * Watchlist management UI: an add-symbol input (validated to 1-5 uppercase
 * letters) plus a removable chip per tracked symbol.
 */
export const Watchlist: React.FC<WatchlistProps> = ({ symbols, onAdd, onRemove }) => {
  const [input, setInput] = useState('');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const candidate = input.trim().toUpperCase();

    if (!SYMBOL_PATTERN.test(candidate)) {
      setError('Enter 1-5 uppercase letters, e.g. AAPL');
      return;
    }
    if (symbols.includes(candidate)) {
      setError(`${candidate} is already in your watchlist`);
      return;
    }

    onAdd(candidate);
    setInput('');
    setError(null);
  };

  return (
    <div className="watchlist-bar">
      <form className="watchlist-form" onSubmit={handleSubmit}>
        <input
          type="text"
          value={input}
          onChange={(e) => {
            setInput(e.target.value.toUpperCase());
            if (error) {
              setError(null);
            }
          }}
          placeholder="Add symbol (e.g. TSLA)"
          maxLength={5}
          aria-label="Add symbol to watchlist"
          className="watchlist-input"
        />
        <button type="submit">Add</button>
      </form>

      {error && <div className="watchlist-error">{error}</div>}

      <ul className="watchlist-chips">
        {symbols.map((symbol) => (
          <li key={symbol} className="watchlist-chip">
            <span>{symbol}</span>
            <button
              type="button"
              className="watchlist-chip-remove"
              onClick={() => onRemove(symbol)}
              aria-label={`Remove ${symbol} from watchlist`}
              disabled={symbols.length === 1}
              title={symbols.length === 1 ? 'At least one symbol is required' : `Remove ${symbol}`}
            >
              ×
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
};
