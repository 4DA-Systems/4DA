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

    it('has empty delegationScores', () => {
      expect(useAppStore.getState().delegationScores).toEqual([]);
    });

    it('has agentDataExists false', () => {
      expect(useAppStore.getState().agentDataExists).toBe(false);
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

  // ---------------------------------------------------------------------------
  // loadDelegationScores
  // ---------------------------------------------------------------------------
  describe('loadDelegationScores', () => {
    it('sets delegationScores on success', async () => {
      const mockScores = [
        {
          subject: 'refactoring',
          overall_score: 0.85,
          factors: {
            pattern_stability: 0.9,
            security_sensitivity: 0.2,
            codebase_complexity: 0.5,
            decision_density: 0.3,
            ai_track_record: 0.8,
          },
          recommendation: 'Safe to delegate',
          caveats: [],
        },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockScores);

      await useAppStore.getState().loadDelegationScores();

      expect(invoke).toHaveBeenCalledWith('get_all_delegation_scores', {});
      expect(useAppStore.getState().delegationScores).toEqual(mockScores);
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadDelegationScores();

      expect(useAppStore.getState().delegationScores).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // checkAgentDataExists
  // ---------------------------------------------------------------------------
  describe('checkAgentDataExists', () => {
    it('sets agentDataExists true when memories exist', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([{ id: 1 }]);

      await useAppStore.getState().checkAgentDataExists();

      expect(invoke).toHaveBeenCalledWith('recall_agent_memories', { subject: '', limit: 1 });
      expect(useAppStore.getState().agentDataExists).toBe(true);
    });

    it('sets agentDataExists false when no memories', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([]);

      await useAppStore.getState().checkAgentDataExists();

      expect(useAppStore.getState().agentDataExists).toBe(false);
    });

    it('sets agentDataExists false on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().checkAgentDataExists();

      expect(useAppStore.getState().agentDataExists).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // promoteMemoryToDecision
  // ---------------------------------------------------------------------------
  describe('promoteMemoryToDecision', () => {
    it('calls invoke and reloads memories', async () => {
      const reloadedMemories = [{ id: 1, promoted_to_decision_id: 10 }];
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)         // promote_memory_to_decision
        .mockResolvedValueOnce(reloadedMemories)   // recall_agent_memories (reload)
        .mockResolvedValueOnce([]);                // get_decisions (reload via loadDecisions)

      await useAppStore.getState().promoteMemoryToDecision(1);

      expect(invoke).toHaveBeenCalledWith('promote_memory_to_decision', { memoryId: 1 });
      expect(useAppStore.getState().agentMemories).toEqual(reloadedMemories);
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().promoteMemoryToDecision(999);

      // Should not throw, memories remain empty
      expect(useAppStore.getState().agentMemories).toEqual([]);
    });
  });
});
