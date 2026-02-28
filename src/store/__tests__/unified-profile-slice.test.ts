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

describe('unified-profile-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has unifiedProfile null', () => {
      expect(useAppStore.getState().unifiedProfile).toBeNull();
    });

    it('has unifiedProfileLoading false', () => {
      expect(useAppStore.getState().unifiedProfileLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadUnifiedProfile
  // ---------------------------------------------------------------------------
  describe('loadUnifiedProfile', () => {
    it('sets unifiedProfile on success', async () => {
      const mockProfile = {
        identity: { role: 'Senior Engineer', experience_level: 'senior' },
        tech_stack: { languages: ['Rust', 'TypeScript'] },
        completeness: { percentage: 75 },
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockProfile);

      await useAppStore.getState().loadUnifiedProfile();

      expect(invoke).toHaveBeenCalledWith('get_sovereign_developer_profile');
      expect(useAppStore.getState().unifiedProfile).toEqual(mockProfile);
      expect(useAppStore.getState().unifiedProfileLoading).toBe(false);
    });

    it('sets loading true during fetch', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise as ReturnType<typeof invoke>);

      const loadPromise = useAppStore.getState().loadUnifiedProfile();

      expect(useAppStore.getState().unifiedProfileLoading).toBe(true);

      resolvePromise!({});
      await loadPromise;

      expect(useAppStore.getState().unifiedProfileLoading).toBe(false);
    });

    it('resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadUnifiedProfile();

      expect(useAppStore.getState().unifiedProfileLoading).toBe(false);
      expect(useAppStore.getState().unifiedProfile).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // exportProfileMarkdown
  // ---------------------------------------------------------------------------
  describe('exportProfileMarkdown', () => {
    it('returns markdown string from invoke', async () => {
      vi.mocked(invoke).mockResolvedValueOnce('# Developer Profile\n\nRust engineer...');

      const result = await useAppStore.getState().exportProfileMarkdown();

      expect(invoke).toHaveBeenCalledWith('export_sovereign_profile_markdown');
      expect(result).toBe('# Developer Profile\n\nRust engineer...');
    });
  });

  // ---------------------------------------------------------------------------
  // exportProfileJson
  // ---------------------------------------------------------------------------
  describe('exportProfileJson', () => {
    it('returns JSON string from invoke', async () => {
      const jsonStr = '{"identity":{"role":"engineer"}}';
      vi.mocked(invoke).mockResolvedValueOnce(jsonStr);

      const result = await useAppStore.getState().exportProfileJson();

      expect(invoke).toHaveBeenCalledWith('export_sovereign_profile_json');
      expect(result).toBe(jsonStr);
    });
  });
});
