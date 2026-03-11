/**
 * Edge case tests for the Zustand store.
 *
 * Tests cross-slice interactions, concurrent updates,
 * malformed data resilience, and reset behaviors.
 */
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

describe('store edge cases', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Settings form edge cases
  // ---------------------------------------------------------------------------
  describe('settings form edge cases', () => {
    it('handles empty string for all form fields', () => {
      useAppStore.getState().setSettingsForm({
        provider: '',
        apiKey: '',
        model: '',
        baseUrl: '',
      });
      const form = useAppStore.getState().settingsForm;
      expect(form.provider).toBe('');
      expect(form.apiKey).toBe('');
    });

    it('handles special characters in API key', () => {
      useAppStore.getState().setSettingsForm({
        apiKey: 'sk-test_key!@#$%^&*()=+[]{}|;:,.<>?/',
      });
      expect(useAppStore.getState().settingsForm.apiKey).toBe(
        'sk-test_key!@#$%^&*()=+[]{}|;:,.<>?/',
      );
    });

    it('handles very long model name', () => {
      const longModel = 'a'.repeat(1000);
      useAppStore.getState().setSettingsForm({ model: longModel });
      expect(useAppStore.getState().settingsForm.model).toBe(longModel);
    });

    it('handles negative values for numeric fields', () => {
      useAppStore.getState().setSettingsForm({
        maxItems: -1,
        minScore: -0.5,
        dailyTokenLimit: -100,
      });
      const form = useAppStore.getState().settingsForm;
      expect(form.maxItems).toBe(-1);
      expect(form.minScore).toBe(-0.5);
    });

    it('handles zero values for numeric fields', () => {
      useAppStore.getState().setSettingsForm({
        maxItems: 0,
        minScore: 0,
        dailyTokenLimit: 0,
        dailyCostLimit: 0,
      });
      const form = useAppStore.getState().settingsForm;
      expect(form.maxItems).toBe(0);
      expect(form.dailyTokenLimit).toBe(0);
    });
  });

  // ---------------------------------------------------------------------------
  // Toast edge cases
  // ---------------------------------------------------------------------------
  describe('toast edge cases', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('toast with empty message still gets added', () => {
      useAppStore.getState().addToast('info', '');
      expect(useAppStore.getState().toasts).toHaveLength(1);
      expect(useAppStore.getState().toasts[0].message).toBe('');
    });

    it('toast IDs are unique across multiple additions', () => {
      useAppStore.getState().addToast('success', 'A');
      useAppStore.getState().addToast('error', 'B');
      useAppStore.getState().addToast('warning', 'C');

      const ids = useAppStore.getState().toasts.map(t => t.id);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(3);
    });

    it('removing an already-removed toast is safe', () => {
      useAppStore.getState().addToast('info', 'Test');
      const id = useAppStore.getState().toasts[0].id;
      useAppStore.getState().removeToast(id);
      // Removing again should not throw
      useAppStore.getState().removeToast(id);
      expect(useAppStore.getState().toasts).toHaveLength(0);
    });
  });

  // ---------------------------------------------------------------------------
  // Analysis state edge cases
  // ---------------------------------------------------------------------------
  describe('analysis state edge cases', () => {
    it('setAppState preserves existing results when updating status', () => {
      const results = [{ id: 1, title: 'Test', top_score: 0.8 }];
      useAppStore.getState().setAppState({ relevanceResults: results as never[] });
      useAppStore.getState().setAppState({ status: 'Updated status' });

      expect(useAppStore.getState().appState.relevanceResults).toHaveLength(1);
      expect(useAppStore.getState().appState.status).toBe('Updated status');
    });

    it('handles rapid state updates without data loss', () => {
      for (let i = 0; i < 100; i++) {
        useAppStore.getState().setAppState({ progress: i });
      }
      expect(useAppStore.getState().appState.progress).toBe(99);
    });

    it('setExpandedItem handles numeric edge cases', () => {
      useAppStore.getState().setExpandedItem(0);
      expect(useAppStore.getState().expandedItem).toBe(0);

      useAppStore.getState().setExpandedItem(Number.MAX_SAFE_INTEGER);
      expect(useAppStore.getState().expandedItem).toBe(Number.MAX_SAFE_INTEGER);
    });
  });

  // ---------------------------------------------------------------------------
  // UI state edge cases
  // ---------------------------------------------------------------------------
  describe('UI state edge cases', () => {
    it('multiple rapid view changes settle on the last one', () => {
      useAppStore.getState().setActiveView('results');
      useAppStore.getState().setActiveView('briefing');
      useAppStore.getState().setActiveView('saved');
      useAppStore.getState().setActiveView('toolkit');
      useAppStore.getState().setActiveView('briefing');

      expect(useAppStore.getState().activeView).toBe('briefing');
    });

    it('showSettings and showSplash can both be true', () => {
      useAppStore.getState().setShowSettings(true);
      // showSplash defaults to true
      expect(useAppStore.getState().showSettings).toBe(true);
      expect(useAppStore.getState().showSplash).toBe(true);
    });

    it('embedding mode handles unknown string values', () => {
      useAppStore.getState().setEmbeddingMode('unknown-mode' as never);
      expect(useAppStore.getState().embeddingMode).toBe('unknown-mode');
    });
  });

  // ---------------------------------------------------------------------------
  // Filter edge cases
  // ---------------------------------------------------------------------------
  describe('filter edge cases', () => {
    it('search query handles special regex characters', () => {
      useAppStore.getState().setSearchQuery('test (regex) [chars] .* +');
      expect(useAppStore.getState().searchQuery).toBe('test (regex) [chars] .* +');
    });

    it('search query handles unicode characters', () => {
      useAppStore.getState().setSearchQuery('日本語テスト');
      expect(useAppStore.getState().searchQuery).toBe('日本語テスト');
    });

    it('search query handles very long strings', () => {
      const longQuery = 'a'.repeat(10000);
      useAppStore.getState().setSearchQuery(longQuery);
      expect(useAppStore.getState().searchQuery).toBe(longQuery);
    });

    it('sort order can be set to date then back to score', () => {
      useAppStore.getState().setSortBy('date');
      expect(useAppStore.getState().sortBy).toBe('date');
      useAppStore.getState().setSortBy('score');
      expect(useAppStore.getState().sortBy).toBe('score');
    });
  });

  // ---------------------------------------------------------------------------
  // Feedback edge cases
  // ---------------------------------------------------------------------------
  describe('feedback edge cases', () => {
    it('handles large number of feedback entries', () => {
      const largeFeedback: Record<number, string> = {};
      for (let i = 0; i < 1000; i++) {
        largeFeedback[i] = i % 2 === 0 ? 'save' : 'dismiss';
      }
      useAppStore.getState().setFeedbackGivenFull(largeFeedback);
      expect(Object.keys(useAppStore.getState().feedbackGiven)).toHaveLength(1000);
    });

    it('feedback map handles numeric string keys', () => {
      useAppStore.getState().setFeedbackGivenFull({ 42: 'save' });
      expect(useAppStore.getState().feedbackGiven[42]).toBe('save');
    });
  });

  // ---------------------------------------------------------------------------
  // License edge cases
  // ---------------------------------------------------------------------------
  describe('license edge cases', () => {
    it('isPro returns false after state reset', () => {
      useAppStore.setState({ tier: 'signal', expired: false });
      expect(useAppStore.getState().isPro()).toBe(true);

      useAppStore.setState(initialState, true);
      expect(useAppStore.getState().isPro()).toBe(false);
    });

    it('activateLicense handles empty key', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: false, reason: 'Empty key' });
      const result = await useAppStore.getState().activateLicense('');
      expect(result.ok).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // Briefing edge cases
  // ---------------------------------------------------------------------------
  describe('briefing edge cases', () => {
    it('generateBriefing handles null briefing in response', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        success: true,
        briefing: null,
        model: null,
        item_count: 0,
      });

      await useAppStore.getState().generateBriefing();

      expect(useAppStore.getState().aiBriefing.content).toBeNull();
    });

    it('autoBriefingEnabled survives other state changes', () => {
      useAppStore.getState().setAutoBriefingEnabled(false);
      useAppStore.getState().setActiveView('results');
      useAppStore.getState().setShowSettings(true);

      expect(useAppStore.getState().autoBriefingEnabled).toBe(false);
    });
  });
});
