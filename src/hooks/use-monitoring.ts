import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { MonitoringStatus } from '../types';

export function useMonitoring() {
  const [monitoring, setMonitoring] = useState<MonitoringStatus | null>(null);
  const [monitoringInterval, setMonitoringInterval] = useState(30);

  const loadMonitoringStatus = useCallback(async () => {
    try {
      const status = await invoke<MonitoringStatus>('get_monitoring_status');
      setMonitoring(status);
      setMonitoringInterval(status.interval_minutes);
    } catch (error) {
      console.debug('Monitoring status not available:', error);
    }
  }, []);

  const toggleMonitoring = useCallback(async (): Promise<string> => {
    if (!monitoring) return 'Monitoring not available';
    const newEnabled = !monitoring.enabled;
    await invoke('set_monitoring_enabled', { enabled: newEnabled });
    await loadMonitoringStatus();
    return newEnabled ? 'Monitoring enabled' : 'Monitoring disabled';
  }, [monitoring, loadMonitoringStatus]);

  const updateMonitoringInterval = useCallback(async (): Promise<string> => {
    await invoke('set_monitoring_interval', { minutes: monitoringInterval });
    await loadMonitoringStatus();
    return `Interval set to ${monitoringInterval} minutes`;
  }, [monitoringInterval, loadMonitoringStatus]);

  const testNotification = useCallback(async (): Promise<string> => {
    await invoke('trigger_notification_test');
    return 'Test notification sent!';
  }, []);

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
