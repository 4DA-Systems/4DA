import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const initialState = useAppStore.getState();

describe('game-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has gameState null', () => {
      expect(useAppStore.getState().gameState).toBeNull();
    });

    it('has celebration null', () => {
      expect(useAppStore.getState().celebration).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadGameState
  // ---------------------------------------------------------------------------
  describe('loadGameState', () => {
    it('sets gameState from invoke result', async () => {
      const mockState = {
        counters: [{ counter_type: 'scans', value: 5 }],
        achievements: [
          {
            id: 'first_scan',
            name: 'First Light',
            description: 'Run your first content scan',
            icon: 'telescope',
            counter_type: 'scans',
            threshold: 1,
            current: 5,
            unlocked: true,
            unlocked_at: '2024-01-01T00:00:00Z',
          },
        ],
        streak: 5,
        last_active: '2024-01-01',
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockState);

      await useAppStore.getState().loadGameState();

      expect(invoke).toHaveBeenCalledWith('get_game_state');
      expect(useAppStore.getState().gameState).toEqual(mockState);
    });

    it('sets empty game state on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadGameState();

      const gameState = useAppStore.getState().gameState;
      expect(gameState).not.toBeNull();
      expect(gameState!.streak).toBe(0);
      expect(gameState!.counters).toEqual([]);
      expect(gameState!.achievements).toEqual([]);
      expect(gameState!.last_active).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // clearCelebration
  // ---------------------------------------------------------------------------
  describe('clearCelebration', () => {
    it('clears the celebration state', () => {
      // Manually set a celebration
      useAppStore.setState({
        celebration: {
          id: 'test',
          name: 'Test Achievement',
          description: 'You did it',
          icon: 'telescope',
          unlocked_at: '2024-01-01T00:00:00Z',
        },
      });

      expect(useAppStore.getState().celebration).not.toBeNull();

      useAppStore.getState().clearCelebration();

      expect(useAppStore.getState().celebration).toBeNull();
    });

    it('is safe to call when celebration is already null', () => {
      useAppStore.getState().clearCelebration();
      expect(useAppStore.getState().celebration).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // initGameListeners
  // ---------------------------------------------------------------------------
  describe('initGameListeners', () => {
    it('returns an unlisten function', async () => {
      const unlisten = await useAppStore.getState().initGameListeners();
      expect(typeof unlisten).toBe('function');
    });
  });
});
