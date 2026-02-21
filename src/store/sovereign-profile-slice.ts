import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';

// ============================================================================
// Types
// ============================================================================

export interface SovereignFact {
  category: string;
  key: string;
  value: string;
  source_lesson: string | null;
  confidence: number;
  updated_at: string;
}

export interface SovereignProfileData {
  facts: SovereignFact[];
  categories: Array<{ category: string; fact_count: number; last_updated: string | null }>;
}

export interface ProfileCompleteness {
  total_categories: number;
  filled_categories: number;
  percentage: number;
  missing: string[];
}

// ============================================================================
// Slice Interface
// ============================================================================

export interface SovereignProfileSlice {
  sovereignProfile: SovereignProfileData | null;
  profileCompleteness: ProfileCompleteness | null;
  profileLoading: boolean;
  generatedDocument: string | null;
  loadSovereignProfile: () => Promise<void>;
  loadProfileCompleteness: () => Promise<void>;
  saveFact: (category: string, key: string, value: string) => Promise<void>;
  generateDocument: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

export const createSovereignProfileSlice: StateCreator<AppStore, [], [], SovereignProfileSlice> = (set, get) => ({
  sovereignProfile: null,
  profileCompleteness: null,
  profileLoading: false,
  generatedDocument: null,

  loadSovereignProfile: async () => {
    set({ profileLoading: true });
    try {
      const data = await invoke<SovereignProfileData>('get_sovereign_profile');
      set({ sovereignProfile: data, profileLoading: false });
    } catch {
      set({ profileLoading: false });
    }
  },

  loadProfileCompleteness: async () => {
    try {
      const data = await invoke<ProfileCompleteness>('get_sovereign_profile_completeness');
      set({ profileCompleteness: data });
    } catch { /* non-fatal */ }
  },

  saveFact: async (category: string, key: string, value: string) => {
    try {
      await invoke('save_sovereign_fact', { category, key, value });
      get().loadSovereignProfile();
      get().loadProfileCompleteness();
    } catch { /* non-fatal */ }
  },

  generateDocument: async () => {
    try {
      const doc = await invoke<string>('generate_sovereign_stack_document');
      set({ generatedDocument: doc });
    } catch { /* non-fatal */ }
  },
});
