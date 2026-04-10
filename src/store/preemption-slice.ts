import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';

// ============================================================================
// Types (mirrors CommandMap definitions in commands.ts)
// ============================================================================

export interface PreemptionEvidence {
  source: string;
  title: string;
  url: string | null;
  freshness_days: number;
  relevance_score: number;
}

export interface PreemptionAction {
  action_type: string;
  label: string;
  description: string;
}

export type PreemptionUrgency = 'critical' | 'high' | 'medium' | 'watch';

export interface PreemptionAlert {
  id: string;
  alert_type: string;
  title: string;
  explanation: string;
  evidence: PreemptionEvidence[];
  affected_projects: string[];
  affected_dependencies: string[];
  urgency: PreemptionUrgency;
  confidence: number;
  predicted_window: string | null;
  suggested_actions: PreemptionAction[];
  created_at: string;
}

export interface PreemptionFeed {
  alerts: PreemptionAlert[];
  total: number;
  critical_count: number;
  high_count: number;
}

// ============================================================================
// Slice Interface
// ============================================================================

export interface PreemptionSlice {
  preemptionFeed: PreemptionFeed | null;
  preemptionLoading: boolean;
  preemptionError: string | null;
  loadPreemption: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

let preemptionInflight: Promise<void> | null = null;

export const createPreemptionSlice: StateCreator<
  AppStore,
  [],
  [],
  PreemptionSlice
> = (set) => ({
  preemptionFeed: null,
  preemptionLoading: false,
  preemptionError: null,

  loadPreemption: async () => {
    if (preemptionInflight) return preemptionInflight;
    const doLoad = async () => {
      set({ preemptionLoading: true, preemptionError: null });
      try {
        const feed = await cmd('get_preemption_alerts');
        set({ preemptionFeed: feed, preemptionLoading: false });
      } catch (error) {
        set({ preemptionError: String(error), preemptionLoading: false });
      } finally {
        preemptionInflight = null;
      }
    };
    preemptionInflight = doLoad();
    return preemptionInflight;
  },
});
