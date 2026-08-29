// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';
import { translateError, isSignalGateError } from '../utils/error-messages';
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
  /**
   * True when the load failed solely because the user's tier doesn't include
   * Blind Spots. A paywall, not a fault — the view renders an upgrade CTA
   * rather than a red error banner. (Mirrors preemptionPaywalled.)
   */
  blindSpotsPaywalled: boolean;
  loadBlindSpots: () => Promise<void>;
}

export const createBlindSpotsSlice: StateCreator<AppStore, [], [], BlindSpotsSlice> = (set, get) => ({
  blindSpotReport: null,
  blindSpotsLoading: false,
  blindSpotsError: null,
  blindSpotsPaywalled: false,

  loadBlindSpots: async () => {
    // In-flight guard: `get_blind_spots` is a ~9s backend query, and
    // StrictMode's double-mount (plus any second consumer) fired it twice in
    // parallel for identical results (2026-08-30 audit, IPC log ids 272/275).
    if (get().blindSpotsLoading) return;
    set({ blindSpotsLoading: true, blindSpotsError: null, blindSpotsPaywalled: false });
    try {
      const report = await cmd('get_blind_spots');
      set({ blindSpotReport: report, blindSpotsLoading: false });
    } catch (error) {
      if (isSignalGateError(error)) {
        set({ blindSpotsPaywalled: true, blindSpotsLoading: false });
      } else {
        set({ blindSpotsError: translateError(error), blindSpotsLoading: false });
      }
    }
  },
});
