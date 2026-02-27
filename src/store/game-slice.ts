import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Achievement {
  id: string;
  title: string;
  description: string;
  icon: string;
  threshold: number;
  unlocked: boolean;
  unlocked_at: string | null;
  progress: number;
}

export interface AchievementUnlocked {
  id: string;
  title: string;
  description: string;
  icon: string;
}

export interface GameState {
  total_unlocked: number;
  total_achievements: number;
  current_streak: number;
  achievements: Achievement[];
}

export interface GameSlice {
  gameState: GameState | null;
  celebration: AchievementUnlocked | null;
  loadGameState: () => Promise<void>;
  clearCelebration: () => void;
  initGameListeners: () => Promise<() => void>;
}

const EMPTY_GAME: GameState = {
  total_unlocked: 0,
  total_achievements: 0,
  current_streak: 0,
  achievements: [],
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
