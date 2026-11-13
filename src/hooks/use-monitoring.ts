// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * How often the header's freshness is re-read. The headless engine process
 * and failed advisory checks emit no event to this window, so without a
 * poll the line could stay green after a check failed.
 */
export const MONITORING_REFRESH_MS = 3 * 60 * 1000;

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
    void loadMonitoringStatus();
    const id = setInterval(() => { void loadMonitoringStatus(); }, MONITORING_REFRESH_MS);
    const onFocus = () => { void loadMonitoringStatus(); };
    window.addEventListener('focus', onFocus);
    return () => {
      clearInterval(id);
      window.removeEventListener('focus', onFocus);
    };
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
