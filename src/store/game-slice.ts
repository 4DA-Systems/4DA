import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type AchievementTier = 'bronze' | 'silver' | 'gold';

export interface Achievement {
  id: string;
  name: string;
  description: string;
  icon: string;
  counter_type: string;
  threshold: number;
  tier: AchievementTier;
  current: number;
  unlocked: boolean;
  unlocked_at: string | null;
}

export interface AchievementUnlocked {
  id: string;
  name: string;
  description: string;
  icon: string;
  tier: AchievementTier;
  celebration_intensity: number;
  unlocked_at: string;
}

export interface CounterState {
  counter_type: string;
  value: number;
}

export interface GameState {
  counters: CounterState[];
  achievements: Achievement[];
  streak: number;
  last_active: string | null;
}

export interface GameSlice {
  gameState: GameState | null;
  celebration: AchievementUnlocked | null;
  loadGameState: () => Promise<void>;
  clearCelebration: () => void;
  initGameListeners: () => Promise<() => void>;
}

const EMPTY_GAME: GameState = {
  counters: [],
  achievements: [],
  streak: 0,
  last_active: null,
};

export const createGameSlice: StateCreator<AppStore, [], [], GameSlice> = (set) => ({
  gameState: null,
  celebration: null,

  loadGameState: async () => {
    try {
      const state = await invoke<GameState>('get_game_state');
      set({ gameState: state });
    } catch {
      set({ gameState: EMPTY_GAME });
    }
  },

  clearCelebration: () => set({ celebration: null }),

  initGameListeners: async () => {
    const unlisten = await listen<AchievementUnlocked>('achievement-unlocked', (event) => {
      set({ celebration: event.payload });

      // Auto-clear after animation completes (3 seconds)
      setTimeout(() => {
        set((state) => {
          // Only clear if it's still the same celebration
          if (state.celebration?.id === event.payload.id) {
            return { celebration: null };
          }
          return {};
        });
      }, 3000);

      // Refresh game state
      invoke<GameState>('get_game_state')
        .then((gs) => set({ gameState: gs }))
        .catch(() => {});
    });

    return unlisten;
  },
});
