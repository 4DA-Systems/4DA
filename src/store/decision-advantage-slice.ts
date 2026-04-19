// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { DecisionWindow, CompoundAdvantageScore } from '../types/autophagy';

export interface DecisionAdvantageSlice {
  decisionWindows: DecisionWindow[];
  compoundAdvantage: CompoundAdvantageScore | null;
  decisionWindowsLoading: boolean;
  loadDecisionWindows: () => Promise<void>;
  loadCompoundAdvantage: () => Promise<void>;
  actOnWindow: (windowId: number, outcome?: string) => Promise<void>;
  closeWindow: (windowId: number) => Promise<void>;
}

export const createDecisionAdvantageSlice: StateCreator<
  DecisionAdvantageSlice, [], [], DecisionAdvantageSlice
> = (set, get) => ({
  decisionWindows: [],
  compoundAdvantage: null,
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

  loadCompoundAdvantage: async () => {
    try {
      const score = await cmd('get_compound_advantage');
      set({ compoundAdvantage: score });
    } catch {
      // Silent
    }
  },

  actOnWindow: async (windowId: number, outcome?: string) => {
    try {
      await cmd('act_on_decision_window', { windowId, outcome: outcome ?? null });
      // Refresh the list
      get().loadDecisionWindows();
      get().loadCompoundAdvantage();
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
