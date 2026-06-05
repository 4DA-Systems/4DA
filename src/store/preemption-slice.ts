// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';
import { isSignalGateError } from '../utils/error-messages';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';
import type { Urgency } from '../../src-tauri/bindings/bindings/Urgency';

// ============================================================================
// Types
// ============================================================================
//
// Intelligence Reconciliation — Phase 3 (2026-04-17):
// The slice now holds the canonical EvidenceFeed (EvidenceItem[] + summary
// counts). Legacy shape aliases are exported for any lingering consumers,
// but new code should import from the canonical bindings directly.

export type PreemptionUrgency = Urgency;
export type PreemptionAlert = EvidenceItem;
export type PreemptionEvidence = EvidenceItem['evidence'][number];
export type PreemptionAction = EvidenceItem['suggested_actions'][number];
export type { EvidenceFeed as PreemptionFeed };

// ============================================================================
// Slice Interface
// ============================================================================

export interface PreemptionSlice {
  preemptionFeed: EvidenceFeed | null;
  preemptionLoading: boolean;
  preemptionError: string | null;
  /**
   * True when the load failed solely because the user's tier doesn't include
   * Preemption Radar. This is a paywall, not a fault — the view renders an
   * upgrade CTA for it rather than a red error banner.
   */
  preemptionPaywalled: boolean;
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
  preemptionPaywalled: false,

  loadPreemption: async () => {
    if (preemptionInflight) return preemptionInflight;
    const doLoad = async () => {
      set({ preemptionLoading: true, preemptionError: null, preemptionPaywalled: false });
      try {
        const feed = await cmd('get_preemption_alerts');
        set({ preemptionFeed: feed, preemptionLoading: false });
      } catch (error) {
        if (isSignalGateError(error)) {
          set({ preemptionPaywalled: true, preemptionLoading: false });
        } else {
          set({ preemptionError: String(error), preemptionLoading: false });
        }
      } finally {
        preemptionInflight = null;
      }
    };
    preemptionInflight = doLoad();
    return preemptionInflight;
  },
});
