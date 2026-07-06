import React, { useEffect } from 'react';
import { useSystemStore } from '../store/systemStore';
import { useMarketStore } from '../store/marketStore';
import { usePatternStore } from '../store/patternStore';
import { useRiskStore } from '../store/riskStore';

interface StoreCleanupProps {
  children: React.ReactNode;
}

/**
 * Wrapper component that manages cleanup of Tauri event listeners
 * and intervals from stores on component unmount.
 */
export const StoreCleanup: React.FC<StoreCleanupProps> = ({ children }) => {
  useEffect(() => {
    return () => {
      // Cleanup all stores on unmount
      useSystemStore.getState().dispose();
      useMarketStore.getState().dispose();
      usePatternStore.getState().dispose();
      useRiskStore.getState().dispose();
    };
  }, []);

  return <>{children}</>;
};
