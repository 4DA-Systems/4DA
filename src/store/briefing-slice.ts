import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, BriefingSlice, BriefingState, FreeBriefingData, InstantBriefingSnapshot } from './types';

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

export const createBriefingSlice: StateCreator<AppStore, [], [], BriefingSlice> = (set) => ({
  aiBriefing: { ...initialBriefingState },
  showBriefing: false,
  autoBriefingEnabled: true,
  lastBackgroundResultsAt: null,
  sourceHealth: [],
  freeBriefing: null,
  freeBriefingLoading: false,
  morningBriefSynthesis: null,
  // Sovereign Cold Boot: hydrate from the pre-mount fetch in main.tsx so the
  // first render already has yesterday's briefing on screen.
  instantSnapshot: readPreloadedSnapshot(),

  setShowBriefing: (show) => set({ showBriefing: show }),
  setMorningBriefSynthesis: (synthesis) => set({ morningBriefSynthesis: synthesis }),
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
          showBriefing: true,
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
          showBriefing: true,
        });
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
          error: `Error: ${error}`,
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
