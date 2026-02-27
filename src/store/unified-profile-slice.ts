import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppStore } from './types';
import type { SovereignDeveloperProfile } from '../types/profile';

// ============================================================================
// Slice Interface
// ============================================================================

export interface UnifiedProfileSlice {
  unifiedProfile: SovereignDeveloperProfile | null;
  unifiedProfileLoading: boolean;
  loadUnifiedProfile: () => Promise<void>;
  exportProfileMarkdown: () => Promise<string>;
  exportProfileJson: () => Promise<string>;
}

// ============================================================================
// Slice Creator
// ============================================================================

export const createUnifiedProfileSlice: StateCreator<AppStore, [], [], UnifiedProfileSlice> = (set) => {
  // Listen for profile-updated events for auto-refresh
  listen<string>('profile-updated', () => {
    invoke<SovereignDeveloperProfile>('get_sovereign_developer_profile')
      .then((data) => set({ unifiedProfile: data }))
      .catch(() => { /* non-fatal */ });
  }).catch(() => { /* listener setup failure is non-fatal */ });

  return {
    unifiedProfile: null,
    unifiedProfileLoading: false,

    loadUnifiedProfile: async () => {
      set({ unifiedProfileLoading: true });
      try {
        const data = await invoke<SovereignDeveloperProfile>('get_sovereign_developer_profile');
        set({ unifiedProfile: data, unifiedProfileLoading: false });
      } catch {
        set({ unifiedProfileLoading: false });
      }
    },

    exportProfileMarkdown: async () => {
      return invoke<string>('export_sovereign_profile_markdown');
    },

    exportProfileJson: async () => {
      return invoke<string>('export_sovereign_profile_json');
    },
  };
};
