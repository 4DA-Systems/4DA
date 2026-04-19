// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('monitoring-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has monitoring null', () => {
      expect(useAppStore.getState().monitoring).toBeNull();
    });

    it('has monitoringInterval defaulting to 30', () => {
      expect(useAppStore.getState().monitoringInterval).toBe(30);
    });

    it('has notificationThreshold defaulting to high_and_above', () => {
      expect(useAppStore.getState().notificationThreshold).toBe('high_and_above');
    });
  });

  // ---------------------------------------------------------------------------
  // setMonitoringInterval
  // ---------------------------------------------------------------------------
  describe('setMonitoringInterval', () => {
    it('sets the monitoring interval', () => {
      useAppStore.getState().setMonitoringInterval(60);
      expect(useAppStore.getState().monitoringInterval).toBe(60);
    });

    it('can set interval to a different value', () => {
      useAppStore.getState().setMonitoringInterval(15);
      expect(useAppStore.getState().monitoringInterval).toBe(15);
    });

    it('overwrites previous interval value', () => {
      useAppStore.getState().setMonitoringInterval(10);
      useAppStore.getState().setMonitoringInterval(45);
      expect(useAppStore.getState().monitoringInterval).toBe(45);
    });
  });

  // ---------------------------------------------------------------------------
  // setNotificationThreshold
  // ---------------------------------------------------------------------------
  describe('setNotificationThreshold', () => {
    it('updates the threshold in state', async () => {
      await useAppStore.getState().setNotificationThreshold('all');
      expect(useAppStore.getState().notificationThreshold).toBe('all');
    });

    it('calls invoke with the threshold', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      await useAppStore.getState().setNotificationThreshold('critical_only');
      expect(invoke).toHaveBeenCalledWith('set_notification_threshold', { threshold: 'critical_only' });
    });
  });

  // ---------------------------------------------------------------------------
  // toggleMonitoring
  // ---------------------------------------------------------------------------
  describe('toggleMonitoring', () => {
    it('returns message when monitoring is not available', async () => {
      const result = await useAppStore.getState().toggleMonitoring();
      expect(result).toBe('Monitoring not available');
    });
  });
});
