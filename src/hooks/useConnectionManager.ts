import { useCallback, useState } from 'react';

interface ConnectionState {
  [tabId: string]: 'connecting' | 'connected' | 'disconnected' | 'error';
}

export function useConnectionManager() {
  const [connectionStates, setConnectionStates] = useState<ConnectionState>({});

  // Set connection state
  const setConnectionState = useCallback((tabId: string, state: 'connecting' | 'connected' | 'disconnected' | 'error') => {
    setConnectionStates(prev => ({ ...prev, [tabId]: state }));
  }, []);

  // Get connection state
  const getConnectionState = useCallback((tabId: string) => {
    return connectionStates[tabId] || 'disconnected';
  }, [connectionStates]);

  // Clean up connection state
  const cleanupConnection = useCallback((tabId: string) => {
    setConnectionStates(prev => {
      const newState = { ...prev };
      delete newState[tabId];
      return newState;
    });
  }, []);

  return {
    connectionStates,
    setConnectionState,
    getConnectionState,
    cleanupConnection,
  };
}