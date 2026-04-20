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

    it('has morningBriefData null', () => {
      expect(useAppStore.getState().morningBriefData).toBeNull();
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
  // setMorningBriefData
  // ---------------------------------------------------------------------------
  describe('setMorningBriefData', () => {
    it('stores morning brief items', () => {
      useAppStore.getState().setMorningBriefData({
        title: '4DA Intelligence Briefing',
        totalRelevant: 3,
        items: [{ title: 'Test', sourceType: 'hn', score: 0.8, signalType: null }],
      });
      expect(useAppStore.getState().morningBriefData?.totalRelevant).toBe(3);
    });

    it('clears morning brief data', () => {
      useAppStore.getState().setMorningBriefData({
        title: 'Brief', totalRelevant: 1, items: [{ title: 'X', sourceType: 'hn', score: 0.5, signalType: null }],
      });
      useAppStore.getState().setMorningBriefData(null);
      expect(useAppStore.getState().morningBriefData).toBeNull();
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
