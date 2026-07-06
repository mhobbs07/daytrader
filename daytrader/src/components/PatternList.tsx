import React, { useMemo, useState } from 'react';
import { usePatternStore } from '../store/patternStore';

export const PatternList: React.FC = () => {
  const { patterns } = usePatternStore();

  // Filter state (local to component)
  const [symbolFilter, setSymbolFilter] = useState('');
  const [signalTypeFilter, setSignalTypeFilter] = useState('All');
  const [confidenceThreshold, setConfidenceThreshold] = useState(0.0);

  // Apply filters using useMemo
  const filteredPatterns = useMemo(() => {
    return patterns.filter(pattern => {
      // Symbol filter (case-insensitive substring match)
      if (symbolFilter && !pattern.symbol.toLowerCase().includes(symbolFilter.toLowerCase())) {
        return false;
      }

      // Signal type filter
      if (signalTypeFilter !== 'All' && pattern.signal_type !== signalTypeFilter) {
        return false;
      }

      // Confidence threshold filter
      if (pattern.confidence < confidenceThreshold) {
        return false;
      }

      return true;
    });
  }, [patterns, symbolFilter, signalTypeFilter, confidenceThreshold]);

  return (
    <div className="pattern-list">
      <h3>Detected Patterns</h3>

      {/* Filter UI */}
      <div className="pattern-filters">
        <div className="filter-group">
          <label htmlFor="symbol-filter" className="filter-label">Symbol</label>
          <input
            id="symbol-filter"
            type="text"
            placeholder="Filter by symbol..."
            value={symbolFilter}
            onChange={(e) => setSymbolFilter(e.target.value)}
            aria-label="Filter patterns by symbol"
            className="filter-input"
          />
        </div>

        <div className="filter-group">
          <label htmlFor="signal-type-filter" className="filter-label">Signal Type</label>
          <select
            id="signal-type-filter"
            value={signalTypeFilter}
            onChange={(e) => setSignalTypeFilter(e.target.value)}
            aria-label="Filter patterns by signal type"
            className="filter-select"
          >
            <option value="All">All</option>
            <option value="Buy">Buy</option>
            <option value="Sell">Sell</option>
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="confidence-filter" className="filter-label">
            Min Confidence: {(confidenceThreshold * 100).toFixed(0)}%
          </label>
          <input
            id="confidence-filter"
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={confidenceThreshold}
            onChange={(e) => setConfidenceThreshold(parseFloat(e.target.value))}
            aria-label="Filter patterns by confidence threshold"
            className="filter-slider"
          />
        </div>
      </div>

      {filteredPatterns.length === 0 ? (
        <div className="pattern-empty">
          {patterns.length === 0 ? 'No patterns detected yet' : 'No patterns match the current filters'}
        </div>
      ) : (
        <ul>
          {filteredPatterns.map((pattern, index) => (
            <li
              key={index}
              className={`pattern-${pattern.signal_type.toLowerCase()}`}
            >
              <strong>{pattern.pattern_name.replace(/_/g, ' ')}</strong>
              {' — '}
              {pattern.symbol}
              {' ('}
              {pattern.signal_type}
              {') '}
              <span title="Confidence">
                {(pattern.confidence * 100).toFixed(1)}%
              </span>
              <div className="pattern-time">
                {new Date(pattern.detected_at).toLocaleString()}
                {' · '}
                ${pattern.price.toFixed(2)}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
