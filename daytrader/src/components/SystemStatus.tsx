import React from 'react';
import { useSystemStore } from '../store/systemStore';

export const SystemStatus: React.FC = () => {
  const { status } = useSystemStore();

  if (!status) return <div>Loading system status...</div>;

  return (
    <div className="system-status">
      <h3>System Status</h3>
      <div className="status-grid">
        <div className={`status-item ${status.broker_connected ? 'connected' : 'disconnected'}`}>
          Broker: {status.broker_connected ? '✓ Connected' : '✗ Disconnected'}
        </div>
        <div className="status-item">
          Event Bus: {status.event_bus_active ? 'Active' : 'Inactive'}
        </div>
        <div className="status-item">
          Active Patterns: {status.patterns_active}
        </div>
        <div className="status-item">
          Events Published: {status.events_published.toLocaleString()}
        </div>
        <div className="status-item">
          Events Dropped: {status.events_dropped.toLocaleString()}
        </div>
        <div className="status-item">
          Avg Latency: {status.average_latency_ms.toFixed(2)}ms
        </div>
      </div>
    </div>
  );
};