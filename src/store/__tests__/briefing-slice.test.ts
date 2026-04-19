// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('briefing-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has aiBriefing with default values', () => {
      const { aiBriefing } = useAppStore.getState();
      expect(aiBriefing.content).toBeNull();
      expect(aiBriefing.loading).toBe(false);
      expect(aiBriefing.error).toBeNull();
      expect(aiBriefing.model).toBeNull();
      expect(aiBriefing.lastGenerated).toBeNull();
    });

    it('has showBriefing false', () => {
      expect(useAppStore.getState().showBriefing).toBe(false);
    });

    it('has autoBriefingEnabled true', () => {
      expect(useAppStore.getState().autoBriefingEnabled).toBe(true);
    });

    it('has lastBackgroundResultsAt null', () => {
      expect(useAppStore.getState().lastBackgroundResultsAt).toBeNull();
    });

    it('has empty sourceHealth array', () => {
      expect(useAppStore.getState().sourceHealth).toEqual([]);
    });

    it('has freeBriefing null', () => {
      expect(useAppStore.getState().freeBriefing).toBeNull();
    });

    it('has freeBriefingLoading false', () => {
      expect(useAppStore.getState().freeBriefingLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // setShowBriefing
  // ---------------------------------------------------------------------------
  describe('setShowBriefing', () => {
    it('sets showBriefing to true', () => {
      useAppStore.getState().setShowBriefing(true);
      expect(useAppStore.getState().showBriefing).toBe(true);
    });

    it('sets showBriefing back to false', () => {
      useAppStore.getState().setShowBriefing(true);
      useAppStore.getState().setShowBriefing(false);
      expect(useAppStore.getState().showBriefing).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // setAutoBriefingEnabled
  // ---------------------------------------------------------------------------
  describe('setAutoBriefingEnabled', () => {
    it('disables auto briefing', () => {
      useAppStore.getState().setAutoBriefingEnabled(false);
      expect(useAppStore.getState().autoBriefingEnabled).toBe(false);
    });

    it('re-enables auto briefing', () => {
      useAppStore.getState().setAutoBriefingEnabled(false);
      useAppStore.getState().setAutoBriefingEnabled(true);
      expect(useAppStore.getState().autoBriefingEnabled).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // setLastBackgroundResultsAt
  // ---------------------------------------------------------------------------
  describe('setLastBackgroundResultsAt', () => {
    it('sets the last background results date', () => {
      const now = new Date();
      useAppStore.getState().setLastBackgroundResultsAt(now);
      expect(useAppStore.getState().lastBackgroundResultsAt).toBe(now);
    });
  });

  // ---------------------------------------------------------------------------
  // generateBriefing
  // ---------------------------------------------------------------------------
  describe('generateBriefing', () => {
    it('sets loading to true then updates on success', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        success: true,
        briefing: 'Your daily briefing content',
        model: 'claude-3-haiku',
        item_count: 10,
      });

      await useAppStore.getState().generateBriefing();

      const { aiBriefing } = useAppStore.getState();
      expect(aiBriefing.loading).toBe(false);
      expect(aiBriefing.content).toBe('Your daily briefing content');
      expect(aiBriefing.model).toBe('claude-3-haiku');
      expect(aiBriefing.error).toBeNull();
      expect(useAppStore.getState().showBriefing).toBe(true);
    });

    it('sets error on unsuccessful result', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        success: false,
        briefing: null,
        error: 'No API key configured',
      });

      await useAppStore.getState().generateBriefing();

      const { aiBriefing } = useAppStore.getState();
      expect(aiBriefing.loading).toBe(false);
      expect(aiBriefing.error).toBe('No API key configured');
      expect(aiBriefing.content).toBeNull();
    });

    it('sets error on invoke rejection', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Connection failed'));

      await useAppStore.getState().generateBriefing();

      const { aiBriefing } = useAppStore.getState();
      expect(aiBriefing.loading).toBe(false);
      expect(aiBriefing.error).toContain('Connection failed');
    });
  });

  // ---------------------------------------------------------------------------
  // generateFreeBriefing
  // ---------------------------------------------------------------------------
  describe('generateFreeBriefing', () => {
    it('sets freeBriefing data on success', async () => {
      const mockData = {
        success: true,
        empty: false,
        top_items: [{ title: 'Item 1', url: 'https://example.com', source: 'hackernews', score: '0.9' }],
        total_items: 5,
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockData);

      await useAppStore.getState().generateFreeBriefing();

      expect(useAppStore.getState().freeBriefing).toEqual(mockData);
      expect(useAppStore.getState().freeBriefingLoading).toBe(false);
    });

    it('resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().generateFreeBriefing();

      expect(useAppStore.getState().freeBriefingLoading).toBe(false);
      expect(useAppStore.getState().freeBriefing).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadSourceHealth
  // ---------------------------------------------------------------------------
  describe('loadSourceHealth', () => {
    it('loads source health status', async () => {
      const mockHealth = [
        { source: 'hackernews', status: 'healthy', last_fetch: '2024-01-01', item_count: 30 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockHealth);

      await useAppStore.getState().loadSourceHealth();

      expect(useAppStore.getState().sourceHealth).toEqual(mockHealth);
    });

    it('silently ignores errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadSourceHealth();

      // Should remain empty, no error thrown
      expect(useAppStore.getState().sourceHealth).toEqual([]);
    });
  });
});
