import type { StateCreator } from 'zustand';
import type { AppStore, ToolkitSlice } from './types';

const MAX_RECENT = 8;

function safeParseArray(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export const createToolkitSlice: StateCreator<AppStore, [], [], ToolkitSlice> = (set, get) => ({
  recentTools: safeParseArray('toolkit_recent'),
  pinnedTools: safeParseArray('toolkit_pinned'),

  addRecentTool: (toolId) => {
    const current = get().recentTools.filter((id) => id !== toolId);
    const next = [toolId, ...current].slice(0, MAX_RECENT);
    localStorage.setItem('toolkit_recent', JSON.stringify(next));
    set({ recentTools: next });
  },

  togglePinnedTool: (toolId) => {
    const current = get().pinnedTools;
    const next = current.includes(toolId)
      ? current.filter((id) => id !== toolId)
      : [...current, toolId];
    localStorage.setItem('toolkit_pinned', JSON.stringify(next));
    set({ pinnedTools: next });
  },
});
