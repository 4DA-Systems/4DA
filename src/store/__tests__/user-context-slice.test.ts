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

describe('user-context-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has userContext null', () => {
      expect(useAppStore.getState().userContext).toBeNull();
    });

    it('has empty suggestedInterests', () => {
      expect(useAppStore.getState().suggestedInterests).toEqual([]);
    });

    it('has empty input fields', () => {
      expect(useAppStore.getState().newInterest).toBe('');
      expect(useAppStore.getState().newExclusion).toBe('');
      expect(useAppStore.getState().newTechStack).toBe('');
      expect(useAppStore.getState().newRole).toBe('');
    });
  });

  // ---------------------------------------------------------------------------
  // setters
  // ---------------------------------------------------------------------------
  describe('field setters', () => {
    it('setNewInterest updates newInterest', () => {
      useAppStore.getState().setNewInterest('rust');
      expect(useAppStore.getState().newInterest).toBe('rust');
    });

    it('setNewExclusion updates newExclusion', () => {
      useAppStore.getState().setNewExclusion('crypto');
      expect(useAppStore.getState().newExclusion).toBe('crypto');
    });

    it('setNewTechStack updates newTechStack', () => {
      useAppStore.getState().setNewTechStack('React');
      expect(useAppStore.getState().newTechStack).toBe('React');
    });

    it('setNewRole updates newRole', () => {
      useAppStore.getState().setNewRole('backend engineer');
      expect(useAppStore.getState().newRole).toBe('backend engineer');
    });
  });

  // ---------------------------------------------------------------------------
  // loadUserContext
  // ---------------------------------------------------------------------------
  describe('loadUserContext', () => {
    it('sets userContext on success', async () => {
      const mockCtx = {
        interests: ['rust', 'wasm'],
        exclusions: ['crypto'],
        tech_stack: ['React'],
        role: 'senior engineer',
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockCtx);

      await useAppStore.getState().loadUserContext();

      expect(invoke).toHaveBeenCalledWith('get_user_context');
      expect(useAppStore.getState().userContext).toEqual(mockCtx);
      expect(useAppStore.getState().newRole).toBe('senior engineer');
    });

    it('does not update newRole when role is absent', async () => {
      const mockCtx = {
        interests: [],
        exclusions: [],
        tech_stack: [],
        role: null,
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockCtx);

      useAppStore.getState().setNewRole('existing role');
      await useAppStore.getState().loadUserContext();

      expect(useAppStore.getState().userContext).toEqual(mockCtx);
      // newRole should remain unchanged since ctx.role is falsy
      expect(useAppStore.getState().newRole).toBe('existing role');
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadUserContext();

      expect(useAppStore.getState().userContext).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadSuggestedInterests
  // ---------------------------------------------------------------------------
  describe('loadSuggestedInterests', () => {
    it('sets suggestedInterests on success', async () => {
      const mockSuggestions = [
        { topic: 'WebAssembly', reason: 'Related to Rust', confidence: 0.85 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockSuggestions);

      await useAppStore.getState().loadSuggestedInterests();

      expect(invoke).toHaveBeenCalledWith('ace_get_suggested_interests');
      expect(useAppStore.getState().suggestedInterests).toEqual(mockSuggestions);
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadSuggestedInterests();

      expect(useAppStore.getState().suggestedInterests).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // addInterest
  // ---------------------------------------------------------------------------
  describe('addInterest', () => {
    it('calls invoke and reloads context on success', async () => {
      const mockCtx = { interests: ['rust'], exclusions: [], tech_stack: [], role: null };
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // add_interest
        .mockResolvedValueOnce(mockCtx);   // get_user_context (reload)

      useAppStore.getState().setNewInterest('rust');
      await useAppStore.getState().addInterest();

      expect(invoke).toHaveBeenCalledWith('add_interest', { topic: 'rust' });
      expect(useAppStore.getState().newInterest).toBe('');
    });

    it('does nothing when newInterest is empty', async () => {
      useAppStore.getState().setNewInterest('   ');
      await useAppStore.getState().addInterest();

      expect(invoke).not.toHaveBeenCalled();
    });
  });

  // ---------------------------------------------------------------------------
  // removeInterest
  // ---------------------------------------------------------------------------
  describe('removeInterest', () => {
    it('calls invoke and reloads context', async () => {
      const mockCtx = { interests: [], exclusions: [], tech_stack: [], role: null };
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)  // remove_interest
        .mockResolvedValueOnce(mockCtx);    // get_user_context (reload)

      await useAppStore.getState().removeInterest('rust');

      expect(invoke).toHaveBeenCalledWith('remove_interest', { topic: 'rust' });
    });
  });
});
