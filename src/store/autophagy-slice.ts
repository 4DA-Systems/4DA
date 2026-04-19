// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AutophagyStatus, AutophagyCycleResult } from '../types/autophagy';

export interface AutophagySlice {
  autophagyStatus: AutophagyStatus | null;
  autophagyHistory: AutophagyCycleResult[];
  autophagyLoading: boolean;
  loadAutophagyStatus: () => Promise<void>;
  loadAutophagyHistory: (limit?: number) => Promise<void>;
}

export const createAutophagySlice: StateCreator<AutophagySlice, [], [], AutophagySlice> = (set) => ({
  autophagyStatus: null,
  autophagyHistory: [],
  autophagyLoading: false,

  loadAutophagyStatus: async () => {
    set({ autophagyLoading: true });
    try {
      const status = await cmd('get_autophagy_status');
      set({ autophagyStatus: status });
    } catch {
      // Silent — autophagy data may not exist yet
    } finally {
      set({ autophagyLoading: false });
    }
  },

  loadAutophagyHistory: async (limit = 10) => {
    try {
      const history = await cmd('get_autophagy_history', { limit });
      set({ autophagyHistory: history });
    } catch {
      // Silent — data may not exist yet
    }
  },
});
