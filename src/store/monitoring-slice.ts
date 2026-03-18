import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, MonitoringSlice } from './types';

export const createMonitoringSlice: StateCreator<AppStore, [], [], MonitoringSlice> = (set, get) => ({
  monitoring: null,
  monitoringInterval: 30,
  notificationThreshold: 'high_and_above',

  setMonitoringInterval: (interval) => set({ monitoringInterval: interval }),

  setNotificationThreshold: async (threshold) => {
    set({ notificationThreshold: threshold });
    try {
      await cmd('set_notification_threshold', { threshold });
    } catch (error) {
      console.error('Failed to set notification threshold:', error);
    }
  },

  loadMonitoringStatus: async () => {
    try {
      const status = await cmd('get_monitoring_status');
      set({ monitoring: status, monitoringInterval: status.interval_minutes });
      const raw = status as unknown as Record<string, unknown>;
      if (raw.notification_threshold) {
        set({ notificationThreshold: raw.notification_threshold as string });
      }
    } catch {
      /* monitoring status not available */
    }
  },

  toggleMonitoring: async () => {
    const { monitoring, loadMonitoringStatus } = get();
    if (!monitoring) return 'Monitoring not available';
    const newEnabled = !monitoring.enabled;
    await cmd('set_monitoring_enabled', { enabled: newEnabled });
    await loadMonitoringStatus();
    return newEnabled ? 'Monitoring enabled' : 'Monitoring disabled';
  },

  updateMonitoringInterval: async () => {
    const { monitoringInterval, loadMonitoringStatus } = get();
    await cmd('set_monitoring_interval', { minutes: monitoringInterval });
    await loadMonitoringStatus();
    return `Interval set to ${monitoringInterval} minutes`;
  },

  testNotification: async () => {
    await cmd('trigger_notification_test');
    return 'Test notification sent!';
  },
});
