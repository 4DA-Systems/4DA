import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const initialState = useAppStore.getState();

describe('autophagy-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has autophagyStatus null', () => {
      expect(useAppStore.getState().autophagyStatus).toBeNull();
    });

    it('has empty autophagyHistory', () => {
      expect(useAppStore.getState().autophagyHistory).toEqual([]);
    });

    it('has autophagyLoading false', () => {
      expect(useAppStore.getState().autophagyLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadAutophagyStatus
  // ---------------------------------------------------------------------------
  describe('loadAutophagyStatus', () => {
    it('sets autophagyStatus on success', async () => {
      const mockStatus = {
        last_cycle: '2024-01-01T00:00:00Z',
        items_processed: 150,
        items_removed: 12,
        health_score: 0.85,
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockStatus);

      await useAppStore.getState().loadAutophagyStatus();

      expect(invoke).toHaveBeenCalledWith('get_autophagy_status');
      expect(useAppStore.getState().autophagyStatus).toEqual(mockStatus);
      expect(useAppStore.getState().autophagyLoading).toBe(false);
    });

    it('sets loading true during fetch then resets', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise as ReturnType<typeof invoke>);

      const loadPromise = useAppStore.getState().loadAutophagyStatus();

      expect(useAppStore.getState().autophagyLoading).toBe(true);

      resolvePromise!({ last_cycle: null, items_processed: 0, items_removed: 0, health_score: 0 });
      await loadPromise;

      expect(useAppStore.getState().autophagyLoading).toBe(false);
    });

    it('resets loading on failure (finally block)', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadAutophagyStatus();

      expect(useAppStore.getState().autophagyLoading).toBe(false);
      expect(useAppStore.getState().autophagyStatus).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadAutophagyHistory
  // ---------------------------------------------------------------------------
  describe('loadAutophagyHistory', () => {
    it('sets autophagyHistory on success', async () => {
      const mockHistory = [
        { cycle_id: 1, timestamp: '2024-01-01', items_processed: 50, items_removed: 3 },
        { cycle_id: 2, timestamp: '2024-01-02', items_processed: 45, items_removed: 1 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockHistory);

      await useAppStore.getState().loadAutophagyHistory();

      expect(invoke).toHaveBeenCalledWith('get_autophagy_history', { limit: 10 });
      expect(useAppStore.getState().autophagyHistory).toEqual(mockHistory);
    });

    it('passes custom limit parameter', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([]);

      await useAppStore.getState().loadAutophagyHistory(25);

      expect(invoke).toHaveBeenCalledWith('get_autophagy_history', { limit: 25 });
    });

    it('handles errors silently', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadAutophagyHistory();

      expect(useAppStore.getState().autophagyHistory).toEqual([]);
    });
  });
});
