// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, MonitoringSlice } from './types';

const positive = (v: unknown): number | null =>
  typeof v === 'number' && Number.isFinite(v) && v > 0 ? Math.round(v) : null;

/**
 * The configured monitoring interval in minutes. `get_monitoring_status` has
 * returned `interval_mins` / `interval_secs`, never `interval_minutes`, so the
 * Settings field rendered blank (audit 2026-10-07). Accept every spelling.
 */
export function intervalMinutesFrom(raw: Record<string, unknown>): number | null {
  const secs = positive(raw.interval_secs);
  return (
    positive(raw.interval_minutes) ?? positive(raw.interval_mins) ?? (secs ? Math.max(1, Math.round(secs / 60)) : null)
  );
}

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
      const raw = status as unknown as Record<string, unknown>;
      const interval = intervalMinutesFrom(raw);
      set({
        monitoring: { ...status, interval_minutes: interval ?? get().monitoringInterval },
        ...(interval != null ? { monitoringInterval: interval } : {}),
      });
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
