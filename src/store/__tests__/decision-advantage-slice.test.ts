// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('decision-advantage-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty decisionWindows array', () => {
      expect(useAppStore.getState().decisionWindows).toEqual([]);
    });

    it('has compoundAdvantage null', () => {
      expect(useAppStore.getState().compoundAdvantage).toBeNull();
    });

    it('has decisionWindowsLoading false', () => {
      expect(useAppStore.getState().decisionWindowsLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadDecisionWindows
  // ---------------------------------------------------------------------------
  describe('loadDecisionWindows', () => {
    it('sets decisionWindows from invoke result', async () => {
      const mockWindows = [
        { id: 1, title: 'Migrate to Bun', urgency: 'medium', expires_at: '2024-12-01' },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockWindows);

      await useAppStore.getState().loadDecisionWindows();

      expect(invoke).toHaveBeenCalledWith('get_decision_windows', {});
      expect(useAppStore.getState().decisionWindows).toEqual(mockWindows);
      expect(useAppStore.getState().decisionWindowsLoading).toBe(false);
    });

    it('sets loading to false even on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadDecisionWindows();

      expect(useAppStore.getState().decisionWindowsLoading).toBe(false);
      expect(useAppStore.getState().decisionWindows).toEqual([]);
    });

    it('sets loading true during fetch', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise);

      const loadPromise = useAppStore.getState().loadDecisionWindows();

      // Loading should be true while waiting
      expect(useAppStore.getState().decisionWindowsLoading).toBe(true);

      resolvePromise!([]);
      await loadPromise;

      expect(useAppStore.getState().decisionWindowsLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadCompoundAdvantage
  // ---------------------------------------------------------------------------
  describe('loadCompoundAdvantage', () => {
    it('sets compoundAdvantage from invoke result', async () => {
      const mockScore = { score: 85, trend: 'rising', factors: [] };
      vi.mocked(invoke).mockResolvedValueOnce(mockScore);

      await useAppStore.getState().loadCompoundAdvantage();

      expect(invoke).toHaveBeenCalledWith('get_compound_advantage', {});
      expect(useAppStore.getState().compoundAdvantage).toEqual(mockScore);
    });

    it('silently ignores errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadCompoundAdvantage();

      expect(useAppStore.getState().compoundAdvantage).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // actOnWindow
  // ---------------------------------------------------------------------------
  describe('actOnWindow', () => {
    it('calls invoke with windowId and outcome', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await useAppStore.getState().actOnWindow(1, 'adopted');

      expect(invoke).toHaveBeenCalledWith('act_on_decision_window', { windowId: 1, outcome: 'adopted' });
    });

    it('passes null outcome when not specified', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await useAppStore.getState().actOnWindow(2);

      expect(invoke).toHaveBeenCalledWith('act_on_decision_window', { windowId: 2, outcome: null });
    });
  });

  // ---------------------------------------------------------------------------
  // closeWindow
  // ---------------------------------------------------------------------------
  describe('closeWindow', () => {
    it('calls invoke with windowId', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await useAppStore.getState().closeWindow(5);

      expect(invoke).toHaveBeenCalledWith('close_decision_window', { windowId: 5, outcome: null });
    });
  });
});
