// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';
import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';

// ============================================================================
// Intelligence Reconciliation — Phase 4 (2026-04-17)
// ============================================================================
// The slice now holds the canonical `EvidenceFeed`. The legacy 4-shape
// `BlindSpotReport` is gone at the command boundary; `feed.score` carries
// the overall coverage index, items carry the former uncovered_dependencies /
// stale_topics / missed_signals / recommendations fused into EvidenceItem
// shape.

export interface BlindSpotsSlice {
  blindSpotReport: EvidenceFeed | null;
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
