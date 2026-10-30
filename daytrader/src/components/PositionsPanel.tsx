import React, { useEffect } from 'react';
import { useExecutionStore } from '../store/executionStore';

/** Open positions table with live unrealized P&L, sourced from the execution store. */
export const PositionsPanel: React.FC = () => {
  const { positions, loading, error, refreshAll } = useExecutionStore();

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (loading && positions.length === 0) {
    return <div className="positions-panel">Loading positions...</div>;
  }

  if (error && positions.length === 0) {
    return (
      <div className="positions-panel error">
        <h3>Open Positions</h3>
        <div className="error-message">Error: {error}</div>
        <button onClick={refreshAll}>Retry</button>
      </div>
    );
  }

  return (
    <div className="positions-panel">
      <h3>Open Positions</h3>

      {positions.length === 0 ? (
        <div className="orders-empty">No open positions — trades will appear here once filled.</div>
      ) : (
        <table className="orders-table">
          <thead>
            <tr>
              <th>Symbol</th>
              <th>Side</th>
              <th>Qty</th>
              <th>Avg Entry</th>
              <th>Current</th>
              <th>Unrealized P&L</th>
              <th>Pattern</th>
            </tr>
          </thead>
          <tbody>
            {positions.map((position) => (
              <tr key={position.symbol}>
                <td className="orders-symbol">{position.symbol}</td>
                <td className={position.side.toLowerCase() === 'long' ? 'side-buy' : 'side-sell'}>
                  {position.side}
                </td>
                <td>{position.quantity}</td>
                <td>${position.avg_entry_price.toFixed(2)}</td>
                <td>${position.current_price.toFixed(2)}</td>
                <td className={position.unrealized_pl >= 0 ? 'pl-positive' : 'pl-negative'}>
                  {position.unrealized_pl >= 0 ? '+' : ''}
                  ${position.unrealized_pl.toFixed(2)} (
                  {position.unrealized_pl_percent >= 0 ? '+' : ''}
                  {position.unrealized_pl_percent.toFixed(2)}%)
                </td>
                <td>{position.pattern_name ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
};
