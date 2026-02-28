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

describe('sovereign-profile-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has sovereignProfile null', () => {
      expect(useAppStore.getState().sovereignProfile).toBeNull();
    });

    it('has profileCompleteness null', () => {
      expect(useAppStore.getState().profileCompleteness).toBeNull();
    });

    it('has profileLoading false', () => {
      expect(useAppStore.getState().profileLoading).toBe(false);
    });

    it('has generatedDocument null', () => {
      expect(useAppStore.getState().generatedDocument).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadSovereignProfile
  // ---------------------------------------------------------------------------
  describe('loadSovereignProfile', () => {
    it('sets sovereignProfile on success', async () => {
      const mockProfile = {
        facts: [
          {
            category: 'languages',
            key: 'primary',
            value: 'Rust',
            source_lesson: null,
            confidence: 0.95,
            updated_at: '2024-01-01',
          },
        ],
        categories: [
          { category: 'languages', fact_count: 1, last_updated: '2024-01-01' },
        ],
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockProfile);

      await useAppStore.getState().loadSovereignProfile();

      expect(invoke).toHaveBeenCalledWith('get_sovereign_profile');
      expect(useAppStore.getState().sovereignProfile).toEqual(mockProfile);
      expect(useAppStore.getState().profileLoading).toBe(false);
    });

    it('sets loading true during fetch', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise as ReturnType<typeof invoke>);

      const loadPromise = useAppStore.getState().loadSovereignProfile();

      expect(useAppStore.getState().profileLoading).toBe(true);

      resolvePromise!({ facts: [], categories: [] });
      await loadPromise;

      expect(useAppStore.getState().profileLoading).toBe(false);
    });

    it('resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadSovereignProfile();

      expect(useAppStore.getState().profileLoading).toBe(false);
      expect(useAppStore.getState().sovereignProfile).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadProfileCompleteness
  // ---------------------------------------------------------------------------
  describe('loadProfileCompleteness', () => {
    it('sets profileCompleteness on success', async () => {
      const mockCompleteness = {
        total_categories: 10,
        filled_categories: 6,
        percentage: 60,
        missing: ['devops', 'testing', 'databases', 'cloud'],
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockCompleteness);

      await useAppStore.getState().loadProfileCompleteness();

      expect(invoke).toHaveBeenCalledWith('get_sovereign_profile_completeness');
      expect(useAppStore.getState().profileCompleteness).toEqual(mockCompleteness);
    });

    it('handles errors silently', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadProfileCompleteness();

      expect(useAppStore.getState().profileCompleteness).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // saveFact
  // ---------------------------------------------------------------------------
  describe('saveFact', () => {
    it('calls invoke and reloads profile', async () => {
      const mockProfile = { facts: [], categories: [] };
      const mockCompleteness = { total_categories: 10, filled_categories: 1, percentage: 10, missing: [] };
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)       // save_sovereign_fact
        .mockResolvedValueOnce(mockProfile)      // get_sovereign_profile (reload)
        .mockResolvedValueOnce(mockCompleteness); // get_sovereign_profile_completeness (reload)

      await useAppStore.getState().saveFact('languages', 'primary', 'Rust');

      expect(invoke).toHaveBeenCalledWith('save_sovereign_fact', {
        category: 'languages',
        key: 'primary',
        value: 'Rust',
      });
    });
  });

  // ---------------------------------------------------------------------------
  // generateDocument
  // ---------------------------------------------------------------------------
  describe('generateDocument', () => {
    it('sets generatedDocument on success', async () => {
      vi.mocked(invoke).mockResolvedValueOnce('# My Sovereign Stack\n\nRust, React...');

      await useAppStore.getState().generateDocument();

      expect(invoke).toHaveBeenCalledWith('generate_sovereign_stack_document');
      expect(useAppStore.getState().generatedDocument).toBe('# My Sovereign Stack\n\nRust, React...');
    });

    it('handles errors silently', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().generateDocument();

      expect(useAppStore.getState().generatedDocument).toBeNull();
    });
  });
});
