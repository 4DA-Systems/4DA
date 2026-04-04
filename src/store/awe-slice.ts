import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type {
  AweSummary,
  AwePatternMatch,
  AwePendingDecision,
  AweGrowthTrajectory,
  AweWisdomWell,
  AweBehavioralContext,
} from '../types/awe';

// ============================================================================
// Slice Interface
// ============================================================================

export interface AweSlice {
  aweSummary: AweSummary | null;
  awePatterns: AwePatternMatch | null;
  awePendingDecisions: AwePendingDecision[];
  aweGrowthTrajectory: AweGrowthTrajectory | null;
  aweWisdomWell: AweWisdomWell | null;
  aweBehavioralContext: AweBehavioralContext | null;
  aweWisdomSynthesis: string | null;
  aweLoading: boolean;
  aweLastSync: string | null;

  loadAweSummary: () => Promise<void>;
  loadAwePatterns: (signalTopics: string) => Promise<void>;
  loadAwePendingDecisions: (limit?: number) => Promise<void>;
  loadAweGrowthTrajectory: () => Promise<void>;
  loadAweWisdomWell: () => Promise<void>;
  loadBehavioralContext: () => Promise<void>;
  synthesizeWisdom: () => Promise<void>;
  submitAweBatchFeedback: (feedbacks: Array<{ decision_id: string; outcome: string; details: string }>) => Promise<void>;
  runAweAutoFeedback: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

export const createAweSlice: StateCreator<
  AweSlice, [], [], AweSlice
> = (set, get) => ({
  aweSummary: null,
  awePatterns: null,
  awePendingDecisions: [],
  aweGrowthTrajectory: null,
  aweWisdomWell: null,
  aweBehavioralContext: null,
  aweWisdomSynthesis: null,
  aweLoading: false,
  aweLastSync: null,

  loadAweSummary: async () => {
    set({ aweLoading: true });
    try {
      const raw = await cmd('get_awe_summary');
      const parsed: AweSummary = JSON.parse(raw);
      set({ aweSummary: parsed, aweLastSync: new Date().toISOString() });
    } catch {
      // AWE is optional — never block UI
    } finally {
      set({ aweLoading: false });
    }
  },

  loadAwePatterns: async (signalTopics: string) => {
    set({ aweLoading: true });
    try {
      const raw = await cmd('get_awe_pattern_match', {
        query: signalTopics,
        domain: 'software-engineering',
      });
      const parsed: AwePatternMatch = JSON.parse(raw);
      set({ awePatterns: parsed });
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },

  loadAwePendingDecisions: async (limit?: number) => {
    set({ aweLoading: true });
    try {
      const raw = await cmd('get_awe_pending_decisions', { limit: limit ?? 20 });
      const parsed: AwePendingDecision[] = JSON.parse(raw);
      set({ awePendingDecisions: parsed });
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },

  loadAweGrowthTrajectory: async () => {
    set({ aweLoading: true });
    try {
      const raw = await cmd('get_awe_growth_trajectory', {
        domain: 'software-engineering',
      });
      const parsed: AweGrowthTrajectory = JSON.parse(raw);
      set({ aweGrowthTrajectory: parsed });
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },

  loadAweWisdomWell: async () => {
    set({ aweLoading: true });
    try {
      const raw = await cmd('get_awe_wisdom_well', {
        domain: 'software-engineering',
      });
      const parsed: AweWisdomWell = JSON.parse(raw);
      set({ aweWisdomWell: parsed });
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },

  loadBehavioralContext: async () => {
    try {
      const raw = await cmd('get_behavioral_context');
      const parsed: AweBehavioralContext = JSON.parse(raw);
      set({ aweBehavioralContext: parsed });
    } catch {
      // Silent — behavioral context is enrichment, never blocks UI
    }
  },

  synthesizeWisdom: async () => {
    set({ aweLoading: true });
    try {
      const wisdom = await cmd('synthesize_wisdom');
      set({ aweWisdomSynthesis: wisdom });
    } catch {
      // Silent — synthesis requires LLM, may not be available
    } finally {
      set({ aweLoading: false });
    }
  },

  submitAweBatchFeedback: async (feedbacks) => {
    set({ aweLoading: true });
    try {
      await cmd('submit_awe_batch_feedback', { feedbacks });
      // Refresh pending decisions and summary after feedback
      void get().loadAwePendingDecisions();
      void get().loadAweSummary();
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },

  runAweAutoFeedback: async () => {
    set({ aweLoading: true });
    try {
      await cmd('run_awe_auto_feedback');
      // Refresh after auto-feedback
      void get().loadAweSummary();
      void get().loadAwePendingDecisions();
    } catch {
      // Silent
    } finally {
      set({ aweLoading: false });
    }
  },
});
