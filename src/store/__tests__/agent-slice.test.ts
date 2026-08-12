// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

describe('agent-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty agentMemories', () => {
      expect(useAppStore.getState().agentMemories).toEqual([]);
    });

    it('has agentMemoryLoading false', () => {
      expect(useAppStore.getState().agentMemoryLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadAgentMemories
  // ---------------------------------------------------------------------------
  describe('loadAgentMemories', () => {
    it('sets agentMemories on success', async () => {
      const mockMemories = [
        {
          id: 1,
          session_id: 'sess-1',
          agent_type: 'explorer',
          memory_type: 'learning',
          subject: 'test subject',
          content: 'test content',
          context_tags: ['rust'],
          created_at: '2024-01-01',
          expires_at: null,
          promoted_to_decision_id: null,
        },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockMemories);

      await useAppStore.getState().loadAgentMemories();

      expect(invoke).toHaveBeenCalledWith('recall_agent_memories', { subject: '', limit: 50 });
      expect(useAppStore.getState().agentMemories).toEqual(mockMemories);
      expect(useAppStore.getState().agentMemoryLoading).toBe(false);
    });

    it('sets loading true during fetch', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise);

      const loadPromise = useAppStore.getState().loadAgentMemories();

      expect(useAppStore.getState().agentMemoryLoading).toBe(true);

      resolvePromise!([]);
      await loadPromise;

      expect(useAppStore.getState().agentMemoryLoading).toBe(false);
    });

    it('resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadAgentMemories();

      expect(useAppStore.getState().agentMemoryLoading).toBe(false);
      expect(useAppStore.getState().agentMemories).toEqual([]);
    });
  });

});
