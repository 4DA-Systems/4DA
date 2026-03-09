import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
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

// Deferred listener setup — only registers once on first profile load
let profileListenerSetup = false;

export const createUnifiedProfileSlice: StateCreator<AppStore, [], [], UnifiedProfileSlice> = (set) => {
  return {
    unifiedProfile: null,
    unifiedProfileLoading: false,

    loadUnifiedProfile: async () => {
      // Set up profile-updated listener on first load (deferred from store creation)
      if (!profileListenerSetup) {
        profileListenerSetup = true;
        listen<string>('profile-updated', () => {
          (cmd('get_sovereign_developer_profile') as Promise<unknown>)
            .then((data) => set({ unifiedProfile: data as SovereignDeveloperProfile }))
            .catch(() => { /* non-fatal */ });
        }).catch(() => { /* listener setup failure is non-fatal */ });
      }

      set({ unifiedProfileLoading: true });
      try {
        const data = await cmd('get_sovereign_developer_profile') as unknown as SovereignDeveloperProfile;
        set({ unifiedProfile: data, unifiedProfileLoading: false });
      } catch {
        set({ unifiedProfileLoading: false });
      }
    },

    exportProfileMarkdown: async () => {
      return cmd('export_sovereign_profile_markdown');
    },

    exportProfileJson: async () => {
      return cmd('export_sovereign_profile_json');
    },
  };
};
