// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * Monitoring hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the init-load effect.
 */
export function useMonitoring() {
  const monitoring = useAppStore(s => s.monitoring);
  const monitoringInterval = useAppStore(s => s.monitoringInterval);
  const setMonitoringInterval = useAppStore(s => s.setMonitoringInterval);
  const loadMonitoringStatus = useAppStore(s => s.loadMonitoringStatus);
  const toggleMonitoring = useAppStore(s => s.toggleMonitoring);
  const updateMonitoringInterval = useAppStore(s => s.updateMonitoringInterval);
  const testNotification = useAppStore(s => s.testNotification);

  useEffect(() => {
    loadMonitoringStatus();
  }, [loadMonitoringStatus]);

  return {
    monitoring,
    monitoringInterval,
    setMonitoringInterval,
    loadMonitoringStatus,
    toggleMonitoring,
    updateMonitoringInterval,
    testNotification,
  };
}
