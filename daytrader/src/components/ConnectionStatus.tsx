import React, { useEffect, useState } from 'react';
import { useTauriEvents } from '../hooks';
import { useSystemStore } from '../store/systemStore';

type ConnectionState = 'connected' | 'disconnected' | 'unknown';

export const ConnectionStatus: React.FC = () => {
  const { status } = useSystemStore();
  const [connectionState, setConnectionState] = useState<ConnectionState>('unknown');

  // Listen for push events from the backend event bridge
  useTauriEvents<string>('system:connection', () => {
    setConnectionState('connected');
  });

  useTauriEvents<string>('system:disconnection', () => {
    setConnectionState('disconnected');
  });

  // Sync with polled system status as fallback
  useEffect(() => {
    if (status !== null) {
      setConnectionState(status.broker_connected ? 'connected' : 'disconnected');
    }
  }, [status]);

  const label =
    connectionState === 'connected' ? 'Connected' :
    connectionState === 'disconnected' ? 'Disconnected' :
    'Connecting…';

  return (
    <div className="connection-status">
      <div className={`connection-dot ${connectionState}`} />
      <span className="connection-label">{label}</span>
    </div>
  );
};
