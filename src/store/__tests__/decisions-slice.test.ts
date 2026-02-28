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

describe('decisions-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty decisions array', () => {
      expect(useAppStore.getState().decisions).toEqual([]);
    });

    it('has decisionsLoading false', () => {
      expect(useAppStore.getState().decisionsLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadDecisions
  // ---------------------------------------------------------------------------
  describe('loadDecisions', () => {
    it('sets decisions on success', async () => {
      const mockDecisions = [
        {
          id: 1,
          decision_type: 'tech_choice',
          subject: 'database',
          decision: 'Use SQLite with sqlite-vec',
          rationale: 'Local-first, no server needed',
          alternatives_rejected: ['PostgreSQL', 'MongoDB'],
          context_tags: ['storage', 'privacy'],
          confidence: 0.95,
          status: 'active',
          superseded_by: null,
          created_at: '2024-01-01',
          updated_at: '2024-01-01',
        },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockDecisions);

      await useAppStore.getState().loadDecisions();

      expect(invoke).toHaveBeenCalledWith('get_decisions', {});
      expect(useAppStore.getState().decisions).toEqual(mockDecisions);
      expect(useAppStore.getState().decisionsLoading).toBe(false);
    });

    it('sets loading true during fetch', async () => {
      let resolvePromise: (v: unknown) => void;
      const pendingPromise = new Promise((resolve) => { resolvePromise = resolve; });
      vi.mocked(invoke).mockReturnValueOnce(pendingPromise as ReturnType<typeof invoke>);

      const loadPromise = useAppStore.getState().loadDecisions();

      expect(useAppStore.getState().decisionsLoading).toBe(true);

      resolvePromise!([]);
      await loadPromise;

      expect(useAppStore.getState().decisionsLoading).toBe(false);
    });

    it('resets loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadDecisions();

      expect(useAppStore.getState().decisionsLoading).toBe(false);
      expect(useAppStore.getState().decisions).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // recordDecision
  // ---------------------------------------------------------------------------
  describe('recordDecision', () => {
    it('calls invoke with correct parameters and reloads', async () => {
      const reloadedDecisions = [{ id: 1, subject: 'database' }];
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)         // record_developer_decision
        .mockResolvedValueOnce(reloadedDecisions); // get_decisions (reload)

      await useAppStore.getState().recordDecision({
        decision_type: 'tech_choice',
        subject: 'database',
        decision: 'Use SQLite',
        rationale: 'Local-first',
        alternatives_rejected: ['PostgreSQL'],
        context_tags: ['storage'],
        confidence: 0.9,
      });

      expect(invoke).toHaveBeenCalledWith('record_developer_decision', {
        decisionType: 'tech_choice',
        subject: 'database',
        decision: 'Use SQLite',
        rationale: 'Local-first',
        alternativesRejected: ['PostgreSQL'],
        contextTags: ['storage'],
        confidence: 0.9,
      });
      expect(useAppStore.getState().decisions).toEqual(reloadedDecisions);
    });

    it('uses default values for optional parameters', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce([]);

      await useAppStore.getState().recordDecision({
        decision_type: 'pattern',
        subject: 'error-handling',
        decision: 'Use Result type',
      });

      expect(invoke).toHaveBeenCalledWith('record_developer_decision', {
        decisionType: 'pattern',
        subject: 'error-handling',
        decision: 'Use Result type',
        rationale: null,
        alternativesRejected: [],
        contextTags: [],
        confidence: 0.8,
      });
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().recordDecision({
        decision_type: 'tech_choice',
        subject: 'test',
        decision: 'test',
      });

      // Should not throw, decisions remain empty
      expect(useAppStore.getState().decisions).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // updateDecision
  // ---------------------------------------------------------------------------
  describe('updateDecision', () => {
    it('calls invoke with correct parameters and reloads', async () => {
      const reloadedDecisions = [{ id: 1, status: 'superseded' }];
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)         // update_developer_decision
        .mockResolvedValueOnce(reloadedDecisions); // get_decisions (reload)

      await useAppStore.getState().updateDecision(1, {
        status: 'superseded',
        rationale: 'Updated rationale',
      });

      expect(invoke).toHaveBeenCalledWith('update_developer_decision', {
        id: 1,
        decision: null,
        rationale: 'Updated rationale',
        status: 'superseded',
        confidence: null,
      });
      expect(useAppStore.getState().decisions).toEqual(reloadedDecisions);
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().updateDecision(1, { decision: 'updated' });

      expect(useAppStore.getState().decisions).toEqual([]);
    });
  });
});
