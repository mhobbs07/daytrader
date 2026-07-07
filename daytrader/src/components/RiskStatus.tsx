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
            <div className={`metric-value ${riskState.daily_pl >= 0 ? 'positive' : 'negative'}`}>
              ${riskState.daily_pl.toFixed(2)}
            </div>
          </div>

          <div className="metric-group">
            <h4>Trading Status</h4>
            <div className={`status-indicator ${riskState.is_trading_halted ? 'halted' : 'active'}`}>
              {riskState.is_trading_halted ? '🚫 HALTED' : '✅ ACTIVE'}
            </div>
          </div>

          <div className="metric-group">
            <h4>Day Trades</h4>
            <div className="metric-value">
              {riskState.day_trade_count}
              {riskLimits && ` / ${riskLimits.max_day_trades_under_pdt}`}
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
              <span className="limit-value">${riskLimits.max_daily_loss_dollars.toFixed(2)}</span>
            </div>
            <div className="limit-item">
              <span className="limit-label">Max Position Size:</span>
              <span className="limit-value">${riskLimits.max_position_size_dollars.toFixed(2)}</span>
            </div>
            <div className="limit-item">
              <span className="limit-label">PDT Rule:</span>
              <span className="limit-value">
                {riskLimits.enforce_pdt_rule ? 'Enabled' : 'Disabled'}
              </span>
            </div>
            <div className="limit-item">
              <span className="limit-label">Min Equity:</span>
              <span className="limit-value">${riskLimits.pdt_min_equity.toFixed(2)}</span>
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
                className={`pdt-value ${pdtStatus.meets_equity_requirement ? 'positive' : 'negative'}`}
              >
                ${pdtStatus.current_equity.toFixed(2)} / ${pdtStatus.pdt_min_equity.toFixed(2)}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Day Trades:</span>
              <span
                className={`pdt-value ${pdtSeverity(
                  pdtStatus.day_trade_count,
                  pdtStatus.max_day_trades_allowed,
                  pdtStatus.is_restricted
                )}`}
              >
                {pdtStatus.day_trade_count}
                {pdtStatus.max_day_trades_allowed !== PDT_UNLIMITED &&
                  ` / ${pdtStatus.max_day_trades_allowed}`}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Rolling Window:</span>
              <span className="pdt-value">
                {pdtStatus.rolling_window_start} to {pdtStatus.rolling_window_end}
              </span>
            </div>
            <div className="pdt-item">
              <span className="pdt-label">Restriction Status:</span>
              <span
                className={`pdt-value ${pdtStatus.is_restricted ? 'negative' : 'positive'}`}
              >
                {pdtStatus.is_restricted ? '🚫 RESTRICTED' : '✅ CLEAR'}
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
                <div className="violation-type">{violation.violation_type}</div>
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