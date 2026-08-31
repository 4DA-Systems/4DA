// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import { translateError } from '../utils/error-messages';
import type { AppStore, BriefingSlice, BriefingState, BriefVerdicts, FreeBriefingData, InstantBriefingSnapshot } from './types';

const initialBriefingState: BriefingState = {
  content: null,
  loading: false,
  error: null,
  model: null,
  lastGenerated: null,
};

/**
 * Sovereign Cold Boot — read the pre-loaded snapshot stashed by main.tsx
 * BEFORE React mounted. The synchronous fetch happens once at module load
 * (in main.tsx), and we just pick up the result here on first store init.
 *
 * This is the entry point that turns 4DA from "fast" to "instant" on cold
 * boot: by the time the store is constructed, the snapshot is already in
 * memory, so the briefing card has data on the very first render.
 */
function readPreloadedSnapshot(): InstantBriefingSnapshot | null {
  if (typeof window === 'undefined') return null;
  const w = window as Window & { __4DA_INSTANT_SNAPSHOT__?: InstantBriefingSnapshot | null };
  const snap = w.__4DA_INSTANT_SNAPSHOT__ ?? null;
  // Consume it — once the store has it, the global is no longer needed.
  if (snap) {
    w.__4DA_INSTANT_SNAPSHOT__ = null;
  }
  return snap;
}

/**
 * AD-035: timer that clears expired brief verdicts so an idle screen stops
 * suppressing the moment the freshness window closes (state change forces
 * the selection memos to recompute). Module-scoped — one active verdict set
 * exists at a time by construction (latest briefing only).
 */
let verdictExpiryTimer: ReturnType<typeof setTimeout> | undefined;

export const createBriefingSlice: StateCreator<AppStore, [], [], BriefingSlice> = (set, get) => ({
  aiBriefing: { ...initialBriefingState },
  autoBriefingEnabled: true,
  lastBackgroundResultsAt: null,
  sourceHealth: [],
  freeBriefing: null,
  freeBriefingLoading: false,
  morningBriefSynthesis: null,
  morningBriefClusters: null,
  morningBriefData: null,
  // Sovereign Cold Boot: hydrate from the pre-mount fetch in main.tsx so the
  // first render already has yesterday's briefing on screen.
  instantSnapshot: readPreloadedSnapshot(),
  briefVerdicts: null,

  setMorningBriefSynthesis: (synthesis) => set({ morningBriefSynthesis: synthesis }),
  setMorningBriefClusters: (clusters) => set({ morningBriefClusters: clusters }),
  setMorningBriefData: (data) => set({ morningBriefData: data }),
  setAutoBriefingEnabled: (enabled) => set({ autoBriefingEnabled: enabled }),
  setLastBackgroundResultsAt: (date) => set({ lastBackgroundResultsAt: date }),
  setInstantSnapshot: (snapshot) => set({ instantSnapshot: snapshot }),

  loadPersistedBriefing: async () => {
    try {
      const result = await cmd('get_latest_briefing');

      if (result) {
        set({
          aiBriefing: {
            content: result.content,
            loading: false,
            error: null,
            model: result.model,
            lastGenerated: new Date(result.created_at + 'Z'),
          },
        });
      }
    } catch {
      // Silently ignore — no persisted briefing available
    }
  },

  loadSourceHealth: async () => {
    try {
      const health = await cmd('get_source_health_status');
      set({ sourceHealth: health });
    } catch {
      // Silently ignore — source health is supplementary
    }
  },

  loadBriefVerdicts: async () => {
    // AD-035: fetch the LATEST briefing's filter verdicts. Fail-open in
    // every branch — a fetch problem must never suppress anything, and an
    // empty answer clears any verdicts we were holding (a newer verdict-less
    // briefing unbinds its predecessor).
    if (verdictExpiryTimer) {
      clearTimeout(verdictExpiryTimer);
      verdictExpiryTimer = undefined;
    }
    try {
      const result = await cmd('get_brief_display_verdicts');
      const entries = result?.filtered ?? [];
      const expiresInMs = (result?.expires_in_seconds ?? 0) * 1000;
      if (entries.length === 0 || expiresInMs <= 0) {
        set({ briefVerdicts: null });
        return;
      }
      const filtered: Record<number, string> = {};
      for (const entry of entries) {
        filtered[entry.id] = entry.reason;
      }
      const verdicts: BriefVerdicts = { filtered, expiresAtMs: Date.now() + expiresInMs };
      console.info(
        `[brief-verdicts] latest briefing binds ${entries.length} filtered item(s) for ${Math.round(expiresInMs / 60_000)}m (demote-only)`,
        entries.map((e) => e.id),
      );
      set({ briefVerdicts: verdicts });
      // At expiry the verdicts bind nothing — clear so the selection memos
      // recompute even on an otherwise idle screen.
      verdictExpiryTimer = setTimeout(() => {
        verdictExpiryTimer = undefined;
        console.info('[brief-verdicts] briefing left its freshness window — verdicts expired, nothing suppressed');
        set({ briefVerdicts: null });
      }, expiresInMs);
    } catch {
      set({ briefVerdicts: null });
    }
  },

  generateBriefing: async () => {
    set(state => ({
      aiBriefing: { ...state.aiBriefing, loading: true, error: null },
    }));
    try {
      const result = await cmd('generate_ai_briefing');

      if (result.success && result.briefing) {
        set({
          aiBriefing: {
            content: result.briefing,
            loading: false,
            error: null,
            model: result.model || null,
            lastGenerated: new Date(),
          },
        });
        // AD-035: the generation just recorded (or superseded) the display-
        // binding verdicts — refresh so the same screen the briefing renders
        // on already honors them. Fire-and-forget: verdicts are supplementary
        // and must never delay or fail the briefing itself.
        void get().loadBriefVerdicts();
      } else {
        set(state => ({
          aiBriefing: {
            ...state.aiBriefing,
            loading: false,
            error: result.error || 'Failed to generate briefing',
          },
        }));
      }
    } catch (error) {
      set(state => ({
        aiBriefing: {
          ...state.aiBriefing,
          loading: false,
          error: translateError(error),
        },
      }));
    }
  },

  generateFreeBriefing: async () => {
    set({ freeBriefingLoading: true });
    try {
      const result = await cmd('generate_free_briefing') as unknown as FreeBriefingData;
      set({ freeBriefing: result, freeBriefingLoading: false });
    } catch {
      set({ freeBriefingLoading: false });
    }
  },
});
