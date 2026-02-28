import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('suns-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty sunStatuses', () => {
      expect(useAppStore.getState().sunStatuses).toEqual([]);
    });

    it('has empty sunAlerts', () => {
      expect(useAppStore.getState().sunAlerts).toEqual([]);
    });

    it('has sunsLoading false', () => {
      expect(useAppStore.getState().sunsLoading).toBe(false);
    });

    it('has sunsError null', () => {
      expect(useAppStore.getState().sunsError).toBeNull();
    });

    it('has streetHealth null', () => {
      expect(useAppStore.getState().streetHealth).toBeNull();
    });

    it('has streetHealthLoading false', () => {
      expect(useAppStore.getState().streetHealthLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadSunStatuses
  // ---------------------------------------------------------------------------
  describe('loadSunStatuses', () => {
    it('loads sun statuses from backend', async () => {
      const mockStatuses = [
        { id: 'sun-1', name: 'Content Refresh', module_id: 'm1', enabled: true, interval_secs: 3600, last_run: null, next_run_in_secs: 1800, last_result: null, run_count: 0 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockStatuses);

      await useAppStore.getState().loadSunStatuses();

      expect(invoke).toHaveBeenCalledWith('get_sun_statuses');
      expect(useAppStore.getState().sunStatuses).toEqual(mockStatuses);
    });

    it('sets sunsError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('network error'));

      await useAppStore.getState().loadSunStatuses();

      expect(useAppStore.getState().sunsError).toContain('network error');
    });
  });

  // ---------------------------------------------------------------------------
  // loadSunAlerts
  // ---------------------------------------------------------------------------
  describe('loadSunAlerts', () => {
    it('loads alerts from backend', async () => {
      const mockAlerts = [
        { id: 1, sun_id: 'sun-1', alert_type: 'warning', message: 'Rate limited', acknowledged: false, created_at: '2024-01-01' },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockAlerts);

      await useAppStore.getState().loadSunAlerts();

      expect(invoke).toHaveBeenCalledWith('get_sun_alerts');
      expect(useAppStore.getState().sunAlerts).toEqual(mockAlerts);
    });

    it('sets sunsError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('db error'));

      await useAppStore.getState().loadSunAlerts();

      expect(useAppStore.getState().sunsError).toContain('db error');
    });
  });

  // ---------------------------------------------------------------------------
  // toggleSun
  // ---------------------------------------------------------------------------
  describe('toggleSun', () => {
    it('invokes toggle and reloads statuses', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // toggle_sun
        .mockResolvedValueOnce([]);       // get_sun_statuses (reload)

      await useAppStore.getState().toggleSun('sun-1', false);

      expect(invoke).toHaveBeenCalledWith('toggle_sun', { sunId: 'sun-1', enabled: false });
    });

    it('sets sunsError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('toggle fail'));

      await useAppStore.getState().toggleSun('sun-1', true);

      expect(useAppStore.getState().sunsError).toContain('toggle fail');
    });
  });

  // ---------------------------------------------------------------------------
  // triggerSun
  // ---------------------------------------------------------------------------
  describe('triggerSun', () => {
    it('triggers sun manually and returns result', async () => {
      const mockResult = { success: true, message: 'Fetched 10 items', data: null };
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockResult) // trigger_sun_manually
        .mockResolvedValueOnce([])         // get_sun_statuses (reload)
        .mockResolvedValueOnce([]);        // get_sun_alerts (reload)

      const result = await useAppStore.getState().triggerSun('sun-1');

      expect(invoke).toHaveBeenCalledWith('trigger_sun_manually', { sunId: 'sun-1' });
      expect(result).toEqual(mockResult);
      expect(useAppStore.getState().sunsLoading).toBe(false);
    });

    it('returns null and sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('trigger fail'));

      const result = await useAppStore.getState().triggerSun('sun-1');

      expect(result).toBeNull();
      expect(useAppStore.getState().sunsError).toContain('trigger fail');
      expect(useAppStore.getState().sunsLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadStreetHealth
  // ---------------------------------------------------------------------------
  describe('loadStreetHealth', () => {
    it('loads street health score', async () => {
      const mockHealth = { overall: 85, module_scores: [], trend: 'improving', top_action: 'Complete Module 3' };
      vi.mocked(invoke).mockResolvedValueOnce(mockHealth);

      await useAppStore.getState().loadStreetHealth();

      expect(invoke).toHaveBeenCalledWith('get_street_health');
      expect(useAppStore.getState().streetHealth).toEqual(mockHealth);
      expect(useAppStore.getState().streetHealthLoading).toBe(false);
    });

    it('sets error and resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('health fail'));

      await useAppStore.getState().loadStreetHealth();

      expect(useAppStore.getState().sunsError).toContain('health fail');
      expect(useAppStore.getState().streetHealthLoading).toBe(false);
    });
  });
});
