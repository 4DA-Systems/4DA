import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';

// ============================================================================
// Types
// ============================================================================

export interface TrustSummary {
  period_days: number;
  total_surfaced: number;
  acted_on: number;
  dismissed: number;
  false_positives: number;
  precision: number;
  action_conversion_rate: number;
  preemption_wins: number;
  avg_lead_time_hours: number | null;
  trend: string;
}

// ============================================================================
// Slice Interface
// ============================================================================

export interface TrustSlice {
  trustSummary: TrustSummary | null;
  trustLoading: boolean;
  trustError: string | null;
  loadTrustSummary: (days?: number) => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

export const createTrustSlice: StateCreator<
  TrustSlice, [], [], TrustSlice
> = (set) => ({
  trustSummary: null,
  trustLoading: false,
  trustError: null,

  loadTrustSummary: async (days = 30) => {
    set({ trustLoading: true, trustError: null });
    try {
      const summary = await cmd('get_trust_dashboard', { days });
      set({ trustSummary: summary as unknown as TrustSummary, trustLoading: false });
    } catch (error) {
      set({ trustError: String(error), trustLoading: false });
    }
  },
});
