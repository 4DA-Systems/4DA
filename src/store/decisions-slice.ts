import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore, DecisionsSlice } from './types';

export interface DeveloperDecision {
  id: number;
  decision_type: 'tech_choice' | 'architecture' | 'workflow' | 'pattern' | 'dependency';
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string[];
  context_tags: string[];
  confidence: number;
  status: 'active' | 'superseded' | 'reconsidering';
  superseded_by: number | null;
  created_at: string;
  updated_at: string;
}

export const createDecisionsSlice: StateCreator<AppStore, [], [], DecisionsSlice> = (set) => ({
  decisions: [],
  decisionsLoading: false,

  loadDecisions: async () => {
    set({ decisionsLoading: true });
    try {
      const decisions = await invoke<DeveloperDecision[]>('get_decisions', {});
      set({ decisions, decisionsLoading: false });
    } catch {
      set({ decisionsLoading: false });
    }
  },

  recordDecision: async (params: {
    decision_type: string;
    subject: string;
    decision: string;
    rationale?: string;
    alternatives_rejected?: string[];
    context_tags?: string[];
    confidence?: number;
  }) => {
    try {
      await invoke('record_developer_decision', {
        decisionType: params.decision_type,
        subject: params.subject,
        decision: params.decision,
        rationale: params.rationale || null,
        alternativesRejected: params.alternatives_rejected || [],
        contextTags: params.context_tags || [],
        confidence: params.confidence ?? 0.8,
      });
      // Reload decisions after recording
      const decisions = await invoke<DeveloperDecision[]>('get_decisions', {});
      set({ decisions });
    } catch (error) {
      console.error('Failed to record decision:', error);
    }
  },

  updateDecision: async (id: number, updates: {
    decision?: string;
    rationale?: string;
    status?: string;
    confidence?: number;
  }) => {
    try {
      await invoke('update_developer_decision', {
        id,
        decision: updates.decision || null,
        rationale: updates.rationale || null,
        status: updates.status || null,
        confidence: updates.confidence ?? null,
      });
      const decisions = await invoke<DeveloperDecision[]>('get_decisions', {});
      set({ decisions });
    } catch (error) {
      console.error('Failed to update decision:', error);
    }
  },
});
