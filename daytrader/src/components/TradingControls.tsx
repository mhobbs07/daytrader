import React, { useEffect } from 'react';
import { useExecutionStore } from '../store/executionStore';

/**
 * Automation kill-switch and execution metrics. Automation ON is a warning
 * state (real orders may be sent to the broker), so it is called out loudly.
 */
export const TradingControls: React.FC = () => {
  const { status, loading, error, refreshAll, setAutomation } = useExecutionStore();

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (loading && !status) {
    return <div className="trading-controls">Loading trading controls...</div>;
  }

  if (error && !status) {
    return (
      <div className="trading-controls error">
        <h3>Trading Controls</h3>
        <div className="error-message">Error: {error}</div>
        <button onClick={refreshAll}>Retry</button>
      </div>
    );
  }

  const enginesAvailable = status?.engines_available ?? false;
  const automationEnabled = status?.automation_enabled ?? false;

  return (
    <div className="trading-controls">
      <h3>Trading Controls</h3>

      <div className="automation-toggle-row">
        <button
          type="button"
          className={`automation-toggle ${automationEnabled ? 'on' : 'off'}`}
          onClick={() => setAutomation(!automationEnabled)}
          disabled={!enginesAvailable}
          aria-pressed={automationEnabled}
        >
          {automationEnabled ? '⚠️ AUTOMATION ON' : 'AUTOMATION OFF'}
        </button>
      </div>

      {!enginesAvailable && (
        <div className="automation-note">
          Trading engines unavailable — database not connected. Automation cannot be
          enabled until the app restarts with a database (see startup logs).
        </div>
      )}

      {error && (
        <div className="error-message">Error: {error}</div>
      )}

      {status && (
        <div className="execution-metrics">
          <div className="metric-group">
            <h4>Signals</h4>
            <div className="metric-value">{status.total_signals}</div>
          </div>
          <div className="metric-group">
            <h4>Auto-Executed</h4>
            <div className="metric-value positive">{status.auto_executed}</div>
          </div>
          <div className="metric-group">
            <h4>Manual Review</h4>
            <div className="metric-value">{status.manual_review}</div>
          </div>
          <div className="metric-group">
            <h4>Rejected</h4>
            <div className="metric-value negative">{status.rejected}</div>
          </div>
          <div className="metric-group">
            <h4>Avg Exec Time</h4>
            <div className="metric-value">{status.avg_exec_time_ms}ms</div>
          </div>
        </div>
      )}
    </div>
  );
};
