// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { DecisionWindow } from '../types/autophagy';

export interface DecisionAdvantageSlice {
  decisionWindows: DecisionWindow[];
  decisionWindowsLoading: boolean;
  loadDecisionWindows: () => Promise<void>;
  actOnWindow: (windowId: number, outcome?: string) => Promise<void>;
  closeWindow: (windowId: number) => Promise<void>;
}

export const createDecisionAdvantageSlice: StateCreator<
  DecisionAdvantageSlice, [], [], DecisionAdvantageSlice
> = (set, get) => ({
  decisionWindows: [],
  decisionWindowsLoading: false,

  loadDecisionWindows: async () => {
    set({ decisionWindowsLoading: true });
    try {
      const windows = await cmd('get_decision_windows');
      set({ decisionWindows: windows });
    } catch {
      // Silent — windows may not exist yet
    } finally {
      set({ decisionWindowsLoading: false });
    }
  },

  actOnWindow: async (windowId: number, outcome?: string) => {
    try {
      await cmd('act_on_decision_window', { windowId, outcome: outcome ?? null });
      get().loadDecisionWindows();
    } catch {
      // Silent
    }
  },

  closeWindow: async (windowId: number) => {
    try {
      await cmd('close_decision_window', { windowId, outcome: null });
      get().loadDecisionWindows();
    } catch {
      // Silent
    }
  },
});
