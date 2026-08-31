// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';
import { isSignalGateError } from '../utils/error-messages';
import {
  loadPersistedDismissals,
  persistDismissal,
  removeDismissal,
} from '../lib/preemption-dismissals';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';

// ============================================================================
// Types
// ============================================================================
//
// Intelligence Reconciliation — Phase 3 (2026-04-17):
// The slice holds the canonical EvidenceFeed (EvidenceItem[] + summary counts).
//
// AD-035 (2026-08-31): the backend is the single source of truth for what the
// Preemption list shows. `loadPreemption` sends the persisted local dismissals
// and the plan-expansion flag; `get_preemption_alerts` applies THE visibility
// filter and returns counts that equal the rendered cards. Dismiss/undo
// persist locally, then refetch — the view never filters or counts on its own.

export type PreemptionAlert = EvidenceItem;
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
  /** Most recently dismissed item id — drives the undo affordance. */
  preemptionLastDismissed: string | null;
  /**
   * True once the user expanded the collapsed Upgrade Plan tail; subsequent
   * loads request the full plan (`fullPlan`) so the expansion survives
   * dismiss/undo refetches.
   */
  preemptionPlanExpanded: boolean;
  loadPreemption: () => Promise<void>;
  /** Persist a dismissal, then refetch so items AND counts move together. */
  dismissPreemptionItem: (id: string) => Promise<void>;
  /** Undo the most recent dismissal, then refetch. */
  undoPreemptionDismissal: () => Promise<void>;
  /** Expire the undo affordance without touching the dismissal itself. */
  clearPreemptionUndo: () => void;
  /** Fetch the plan steps the list transport held back (plan "show more"). */
  expandPreemptionPlan: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

interface InflightLoad {
  key: string;
  promise: Promise<void>;
}

let preemptionInflight: InflightLoad | null = null;

export const createPreemptionSlice: StateCreator<
  AppStore,
  [],
  [],
  PreemptionSlice
> = (set, get) => ({
  preemptionFeed: null,
  preemptionLoading: false,
  preemptionError: null,
  preemptionPaywalled: false,
  preemptionLastDismissed: null,
  preemptionPlanExpanded: false,

  loadPreemption: async () => {
    const dismissedIds = [...loadPersistedDismissals()].sort();
    const fullPlan = get().preemptionPlanExpanded;
    const key = `${fullPlan}|${dismissedIds.join(',')}`;
    // Join an identical in-flight request (double-mount); a request with a
    // DIFFERENT dismissal set or plan scope chains behind it instead — a
    // dismissal must never be swallowed by the initial load's promise.
    if (preemptionInflight?.key === key) return preemptionInflight.promise;
    const previous = preemptionInflight?.promise ?? Promise.resolve();

    const doLoad = async () => {
      set({ preemptionLoading: true, preemptionError: null, preemptionPaywalled: false });
      try {
        const feed = await cmd('get_preemption_alerts', { dismissedIds, fullPlan });
        set({ preemptionFeed: feed, preemptionLoading: false });
      } catch (error) {
        if (isSignalGateError(error)) {
          set({ preemptionPaywalled: true, preemptionLoading: false });
        } else {
          set({ preemptionError: String(error), preemptionLoading: false });
        }
      }
    };

    const entry: InflightLoad = { key, promise: Promise.resolve() };
    entry.promise = previous
      .catch(() => { /* the previous request reported its own error */ })
      .then(doLoad)
      .finally(() => {
        if (preemptionInflight === entry) preemptionInflight = null;
      });
    preemptionInflight = entry;
    return entry.promise;
  },

  dismissPreemptionItem: async (id: string) => {
    persistDismissal(id);
    set({ preemptionLastDismissed: id });
    await get().loadPreemption();
  },

  undoPreemptionDismissal: async () => {
    const id = get().preemptionLastDismissed;
    if (!id) return;
    removeDismissal(id);
    set({ preemptionLastDismissed: null });
    await get().loadPreemption();
  },

  clearPreemptionUndo: () => set({ preemptionLastDismissed: null }),

  expandPreemptionPlan: async () => {
    if (get().preemptionPlanExpanded) return;
    set({ preemptionPlanExpanded: true });
    await get().loadPreemption();
  },
});
