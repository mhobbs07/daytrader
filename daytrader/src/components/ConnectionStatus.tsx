import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSystemStore } from '../store/systemStore';

type ConnectionState = 'connected' | 'disconnected' | 'unknown';

export const ConnectionStatus: React.FC = () => {
  const { status } = useSystemStore();
  const [connectionState, setConnectionState] = useState<ConnectionState>('unknown');

  useEffect(() => {
    // Listen for push events from the backend event bridge
    const unlistenConnected = listen<string>('system:connection', () => {
      setConnectionState('connected');
    });

    const unlistenDisconnected = listen<string>('system:disconnection', () => {
      setConnectionState('disconnected');
    });

    return () => {
      unlistenConnected.then(fn => fn());
      unlistenDisconnected.then(fn => fn());
    };
  }, []);

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
