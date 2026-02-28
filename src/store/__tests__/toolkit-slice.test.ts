import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const initialState = useAppStore.getState();

describe('toolkit-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    localStorage.clear();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has recentTools as array', () => {
      expect(Array.isArray(useAppStore.getState().recentTools)).toBe(true);
    });

    it('has pinnedTools as array', () => {
      expect(Array.isArray(useAppStore.getState().pinnedTools)).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // addRecentTool
  // ---------------------------------------------------------------------------
  describe('addRecentTool', () => {
    it('adds a tool to the front of recentTools', () => {
      useAppStore.getState().addRecentTool('tool-a');
      useAppStore.getState().addRecentTool('tool-b');

      const recent = useAppStore.getState().recentTools;
      expect(recent[0]).toBe('tool-b');
      expect(recent[1]).toBe('tool-a');
    });

    it('deduplicates and moves existing tool to front', () => {
      useAppStore.getState().addRecentTool('tool-a');
      useAppStore.getState().addRecentTool('tool-b');
      useAppStore.getState().addRecentTool('tool-a');

      const recent = useAppStore.getState().recentTools;
      expect(recent[0]).toBe('tool-a');
      expect(recent.filter((id: string) => id === 'tool-a')).toHaveLength(1);
    });

    it('limits to 8 recent tools', () => {
      for (let i = 0; i < 10; i++) {
        useAppStore.getState().addRecentTool(`tool-${i}`);
      }

      expect(useAppStore.getState().recentTools).toHaveLength(8);
    });

    it('persists to localStorage', () => {
      useAppStore.getState().addRecentTool('tool-x');

      const stored = JSON.parse(localStorage.getItem('toolkit_recent') || '[]');
      expect(stored).toContain('tool-x');
    });
  });

  // ---------------------------------------------------------------------------
  // togglePinnedTool
  // ---------------------------------------------------------------------------
  describe('togglePinnedTool', () => {
    it('pins a tool', () => {
      useAppStore.getState().togglePinnedTool('tool-a');

      expect(useAppStore.getState().pinnedTools).toContain('tool-a');
    });

    it('unpins a previously pinned tool', () => {
      useAppStore.getState().togglePinnedTool('tool-a');
      useAppStore.getState().togglePinnedTool('tool-a');

      expect(useAppStore.getState().pinnedTools).not.toContain('tool-a');
    });

    it('persists to localStorage', () => {
      useAppStore.getState().togglePinnedTool('tool-y');

      const stored = JSON.parse(localStorage.getItem('toolkit_pinned') || '[]');
      expect(stored).toContain('tool-y');
    });
  });
});
