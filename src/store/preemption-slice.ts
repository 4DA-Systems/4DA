// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { cmd } from '../lib/commands';
import { isSignalGateError, translateError } from '../utils/error-messages';
import {
  dismissAlert,
  evidenceAlertKey,
  moveLegacyPreemptionDismissals,
  undoDismissal,
} from '../lib/alert-dismissals';
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
// Preemption list shows. Dismissals live in the backend triage store shared
// with the critical banner (lib/alert-dismissals, 7-day expiry, undo);
// `get_preemption_alerts` reads them, applies THE visibility filter and
// returns counts that equal the rendered cards. Dismiss/undo write the store,
// then refetch — the view never filters or counts on its own.

export type PreemptionAlert = EvidenceItem;
/** What a Preemption dismissal says; it sets how long it holds. */
export type PreemptionDismissAction = 'not_applicable' | 'snoozed' | 'acknowledged';
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
  /**
   * Re-fetch without the loading state, keeping the shown feed on failure.
   * For a feed served while its replacement builds in the background (the
   * previous run's, after a restart): the rebuilt one swaps in without
   * blanking the tab. Same dismissals and plan scope as a normal load.
   */
  refreshPreemptionQuietly: () => Promise<void>;
  /** Persist a dismissal, then refetch so items AND counts move together.
   *  "Not affected" (`not_applicable`) holds 90 days; a snooze 7. */
  dismissPreemptionItem: (id: string, action?: PreemptionDismissAction) => Promise<void>;
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
let dismissalGeneration = 0;

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
    const fullPlan = get().preemptionPlanExpanded;
    // A load after a dismissal must not join the load it raced with: the
    // generation moves on every dismissal and undo.
    const key = `${fullPlan}|${dismissalGeneration}`;
    // Join an identical in-flight request (double-mount); a request with a
    // DIFFERENT dismissal set or plan scope chains behind it instead — a
    // dismissal must never be swallowed by the initial load's promise.
    if (preemptionInflight?.key === key) return preemptionInflight.promise;
    const previous = preemptionInflight?.promise ?? Promise.resolve();

    const doLoad = async () => {
      set({ preemptionLoading: true, preemptionError: null, preemptionPaywalled: false });
      try {
        await moveLegacyPreemptionDismissals();
        const feed = await cmd('get_preemption_alerts', { fullPlan });
        set({ preemptionFeed: feed, preemptionLoading: false });
      } catch (error) {
        if (isSignalGateError(error)) {
          set({ preemptionPaywalled: true, preemptionLoading: false });
        } else {
          // Same user-facing wording as the Blind Spots slice — a raw
          // `String(error)` put "TypeError: ..." / backend internals in the banner.
          set({ preemptionError: translateError(error), preemptionLoading: false });
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

  refreshPreemptionQuietly: async () => {
    // A normal load (first open, dismiss, undo) owns the feed while it runs.
    if (get().preemptionLoading || preemptionInflight) return;
    const fullPlan = get().preemptionPlanExpanded;
    try {
      const feed = await cmd('get_preemption_alerts', { fullPlan });
      // A dismissal made while this was in flight wins: its load refetches.
      if (!get().preemptionLoading && !preemptionInflight) set({ preemptionFeed: feed });
    } catch {
      // Keep what is shown; the next open or reload reports any real error.
    }
  },

  dismissPreemptionItem: async (id: string, action: PreemptionDismissAction = 'acknowledged') => {
    try {
      await dismissAlert({ alertKey: evidenceAlertKey(id) }, action);
    } catch (error) {
      // Not stored, so not hidden: say so instead of pretending.
      set({ preemptionError: translateError(error) });
      return;
    }
    dismissalGeneration += 1;
    set({ preemptionLastDismissed: id });
    await get().loadPreemption();
  },

  undoPreemptionDismissal: async () => {
    const id = get().preemptionLastDismissed;
    if (!id) return;
    try {
      await undoDismissal({ alertKey: evidenceAlertKey(id) });
    } catch (error) {
      set({ preemptionError: translateError(error) });
      return;
    }
    dismissalGeneration += 1;
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
