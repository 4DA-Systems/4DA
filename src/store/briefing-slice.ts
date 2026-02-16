import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore, BriefingSlice, BriefingState } from './types';

const initialBriefingState: BriefingState = {
  content: null,
  loading: false,
  error: null,
  model: null,
  lastGenerated: null,
};

export const createBriefingSlice: StateCreator<AppStore, [], [], BriefingSlice> = (set) => ({
  aiBriefing: { ...initialBriefingState },
  showBriefing: false,
  autoBriefingEnabled: true,
  lastBackgroundResultsAt: null,

  setShowBriefing: (show) => set({ showBriefing: show }),
  setAutoBriefingEnabled: (enabled) => set({ autoBriefingEnabled: enabled }),
  setLastBackgroundResultsAt: (date) => set({ lastBackgroundResultsAt: date }),

  loadPersistedBriefing: async () => {
    try {
      const result = await invoke<{
        content: string;
        model: string | null;
        item_count: number;
        created_at: string;
      } | null>('get_latest_briefing');

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

  generateBriefing: async () => {
    set(state => ({
      aiBriefing: { ...state.aiBriefing, loading: true, error: null },
    }));
    try {
      const result = await invoke<{
        success: boolean;
        briefing: string | null;
        error?: string;
        model?: string;
        item_count?: number;
        latency_ms?: number;
      }>('generate_ai_briefing');

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
});
