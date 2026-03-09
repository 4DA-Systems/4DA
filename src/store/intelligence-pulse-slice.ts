import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { IntelligencePulseData } from '../types/autophagy';

export interface IntelligencePulseSlice {
  intelligencePulse: IntelligencePulseData | null;
  intelligencePulseLoading: boolean;
  loadIntelligencePulse: () => Promise<void>;
}

let pulseInflight: Promise<void> | null = null;

export const createIntelligencePulseSlice: StateCreator<
  IntelligencePulseSlice,
  [],
  [],
  IntelligencePulseSlice
> = (set) => ({
  intelligencePulse: null,
  intelligencePulseLoading: false,

  loadIntelligencePulse: async () => {
    if (pulseInflight) return pulseInflight;
    const doLoad = async () => {
      set({ intelligencePulseLoading: true });
      try {
        const pulse = await cmd('get_intelligence_pulse') as unknown as IntelligencePulseData;
        set({ intelligencePulse: pulse });
      } catch {
        // Silent — intelligence pulse is supplementary
      } finally {
        set({ intelligencePulseLoading: false });
        pulseInflight = null;
      }
    };
    pulseInflight = doLoad();
    return pulseInflight;
  },
});
