import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { IntelligencePulseData } from '../types/autophagy';

export interface IntelligencePulseSlice {
  intelligencePulse: IntelligencePulseData | null;
  intelligencePulseLoading: boolean;
  loadIntelligencePulse: () => Promise<void>;
}

export const createIntelligencePulseSlice: StateCreator<
  IntelligencePulseSlice,
  [],
  [],
  IntelligencePulseSlice
> = (set) => ({
  intelligencePulse: null,
  intelligencePulseLoading: false,

  loadIntelligencePulse: async () => {
    set({ intelligencePulseLoading: true });
    try {
      const pulse = await invoke<IntelligencePulseData>('get_intelligence_pulse');
      set({ intelligencePulse: pulse });
    } catch {
      // Silent — intelligence pulse is supplementary
    } finally {
      set({ intelligencePulseLoading: false });
    }
  },
});
