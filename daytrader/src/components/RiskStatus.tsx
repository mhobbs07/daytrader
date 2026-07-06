import React, { useEffect } from 'react';
import { useRiskStore } from '../store/riskStore';

/** u32::MAX as sent over Tauri IPC — signals "no PDT limit applies". */
const PDT_UNLIMITED = 4294967295;

/** Returns 'positive' / 'warning' / 'negative' based on PDT restriction proximity. */
function pdtSeverity(dayTradeCount: number, maxAllowed: number, isRestricted: boolean): 'positive' | 'warning' | 'negative' {
  if (isRestricted) {
    return 'negative';
  }
  if (maxAllowed !== PDT_UNLIMITED && dayTradeCount >= maxAllowed - 1) {
    return 'warning';
  }
  return 'positive';
}

export const RiskStatus: React.FC = () => {
  const {
    riskState,
    riskLimits,
    recentViolations,
    pdtStatus,
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

      {pdtStatus && (
        <div className="pdt-status">
          <h4>PDT Status</h4>
          <div className="pdt-grid">
            <div className="pdt-item">
              <span className="pdt-label">Account Equity:</span>
              <span
                className={`pdt-value ${pdtStatus.meetsEquityRequirement ? 'positive' : 'negative'}`}
              >
                ${pdtStatus.currentEquity.toFixed(2)} / ${pdtStatus.pdtMinEquity.toFixed(2)}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Day Trades:</span>
              <span
                className={`pdt-value ${pdtSeverity(
                  pdtStatus.dayTradeCount,
                  pdtStatus.maxDayTradesAllowed,
                  pdtStatus.isRestricted
                )}`}
              >
                {pdtStatus.dayTradeCount}
                {pdtStatus.maxDayTradesAllowed !== PDT_UNLIMITED &&
                  ` / ${pdtStatus.maxDayTradesAllowed}`}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Rolling Window:</span>
              <span className="pdt-value">
                {pdtStatus.rollingWindowStart} to {pdtStatus.rollingWindowEnd}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Restriction Status:</span>
              <span
                className={`pdt-value ${pdtStatus.isRestricted ? 'negative' : 'positive'}`}
              >
                {pdtStatus.isRestricted ? '🚫 RESTRICTED' : '✅ CLEAR'}
              </span>
            </div>
          </div>
          {pdtStatus.warnings.length > 0 && (
            <div className="pdt-warnings">
              {pdtStatus.warnings.map((warning, index) => (
                <div key={index} className="pdt-warning-item">
                  {warning}
                </div>
              ))}
            </div>
          )}
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