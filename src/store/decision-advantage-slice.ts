import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
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
      const windows = await invoke<DecisionWindow[]>('get_decision_windows');
      set({ decisionWindows: windows });
    } catch {
      // Silent — windows may not exist yet
    } finally {
      set({ decisionWindowsLoading: false });
    }
  },

  loadCompoundAdvantage: async () => {
    try {
      const score = await invoke<CompoundAdvantageScore>('get_compound_advantage');
      set({ compoundAdvantage: score });
    } catch {
      // Silent
    }
  },

  actOnWindow: async (windowId: number, outcome?: string) => {
    try {
      await invoke('act_on_decision_window', { windowId, outcome: outcome ?? null });
      // Refresh the list
      get().loadDecisionWindows();
      get().loadCompoundAdvantage();
    } catch {
      // Silent
    }
  },

  closeWindow: async (windowId: number) => {
    try {
      await invoke('close_decision_window', { windowId });
      get().loadDecisionWindows();
    } catch {
      // Silent
    }
  },
});
