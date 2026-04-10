import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';

// Types match CommandMap in commands.ts
interface BlindSpotReport {
  overall_score: number;
  uncovered_dependencies: Array<{
    name: string;
    dep_type: string;
    projects_using: string[];
    days_since_last_signal: number;
    available_signal_count: number;
    risk_level: string;
  }>;
  stale_topics: Array<{
    topic: string;
    last_engagement_days: number;
    active_deps_in_topic: number;
    missed_signal_count: number;
  }>;
  missed_signals: Array<{
    item_id: number;
    title: string;
    url: string | null;
    source_type: string;
    relevance_score: number;
    created_at: string;
    why_relevant: string;
  }>;
  recommendations: Array<{ action: string; reason: string; priority: string }>;
  generated_at: string;
}

export interface BlindSpotsSlice {
  blindSpotReport: BlindSpotReport | null;
  blindSpotsLoading: boolean;
  blindSpotsError: string | null;
  loadBlindSpots: () => Promise<void>;
}

export const createBlindSpotsSlice: StateCreator<AppStore, [], [], BlindSpotsSlice> = (set) => ({
  blindSpotReport: null,
  blindSpotsLoading: false,
  blindSpotsError: null,

  loadBlindSpots: async () => {
    set({ blindSpotsLoading: true, blindSpotsError: null });
    try {
      const report = await cmd('get_blind_spots');
      set({ blindSpotReport: report, blindSpotsLoading: false });
    } catch (error) {
      set({ blindSpotsError: String(error), blindSpotsLoading: false });
    }
  },
});
