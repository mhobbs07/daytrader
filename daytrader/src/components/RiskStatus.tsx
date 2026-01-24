import React, { useEffect } from 'react';
import { useRiskStore } from '../store/riskStore';

export const RiskStatus: React.FC = () => {
  const {
    riskState,
    riskLimits,
    recentViolations,
    loading,
    error,
    refreshAll
  } = useRiskStore();

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (loading && !riskState) {
    return <div className="risk-status">Loading risk status...</div>;
  }

  if (error) {
    return (
      <div className="risk-status error">
        <h3>Risk Status</h3>
        <div className="error-message">Error: {error}</div>
        <button onClick={refreshAll}>Retry</button>
      </div>
    );
  }

  return (
    <div className="risk-status">
      <h3>Risk Status</h3>

      {riskState && (
        <div className="risk-metrics">
          <div className="metric-group">
            <h4>Daily P&L</h4>
            <div className={`metric-value ${riskState.dailyPl >= 0 ? 'positive' : 'negative'}`}>
              ${riskState.dailyPl.toFixed(2)}
            </div>
          </div>

          <div className="metric-group">
            <h4>Trading Status</h4>
            <div className={`status-indicator ${riskState.isTradingHalted ? 'halted' : 'active'}`}>
              {riskState.isTradingHalted ? '🚫 HALTED' : '✅ ACTIVE'}
            </div>
          </div>

          <div className="metric-group">
            <h4>Day Trades</h4>
            <div className="metric-value">
              {riskState.dayTradeCount}
              {riskLimits && ` / ${riskLimits.maxDayTradesUnderPdt}`}
            </div>
          </div>
        </div>
      )}

      {riskLimits && (
        <div className="risk-limits">
          <h4>Risk Limits</h4>
          <div className="limits-grid">
            <div className="limit-item">
              <span className="limit-label">Daily Loss Limit:</span>
              <span className="limit-value">${riskLimits.maxDailyLossDollars.toFixed(2)}</span>
            </div>
            <div className="limit-item">
              <span className="limit-label">Max Position Size:</span>
              <span className="limit-value">${riskLimits.maxPositionSizeDollars.toFixed(2)}</span>
            </div>
            <div className="limit-item">
              <span className="limit-label">PDT Rule:</span>
              <span className="limit-value">
                {riskLimits.enforcePdtRule ? 'Enabled' : 'Disabled'}
              </span>
            </div>
            <div className="limit-item">
              <span className="limit-label">Min Equity:</span>
              <span className="limit-value">${riskLimits.pdtMinEquity.toFixed(2)}</span>
            </div>
          </div>
        </div>
      )}

      {recentViolations.length > 0 && (
        <div className="risk-violations">
          <h4>Recent Violations</h4>
          <div className="violations-list">
            {recentViolations.slice(0, 3).map((violation, index) => (
              <div key={index} className={`violation-item ${violation.severity.toLowerCase()}`}>
                <div className="violation-type">{violation.violationType}</div>
                <div className="violation-message">{violation.message}</div>
                <div className="violation-time">
                  {new Date(violation.timestamp).toLocaleTimeString()}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="risk-actions">
        <button onClick={refreshAll} disabled={loading}>
          {loading ? 'Refreshing...' : 'Refresh'}
        </button>
      </div>
    </div>
  );
};