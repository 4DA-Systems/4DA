// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore, UiSlice, ActiveView } from './types';

const VALID_VIEWS: ActiveView[] = ['briefing', 'results', 'preemption', 'blindspots', 'playbook'];

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  showSettings: false,
  showSplash: true,
  activeView: 'briefing',
  isFirstRun: false,
  firstRunDismissed: false,
  embeddingMode: null,
  embeddingStatus: undefined,

  setShowSettings: (show) => set({ showSettings: show }),
  setShowSplash: (show) => set({ showSplash: show }),
  setActiveView: (view) => {
    if (VALID_VIEWS.includes(view)) set({ activeView: view });
  },
  setIsFirstRun: (v) => set({ isFirstRun: v }),
  setFirstRunDismissed: (v) => set({ firstRunDismissed: v }),
  setEmbeddingMode: (mode) => set({ embeddingMode: mode }),
  setEmbeddingStatus: (status) => set({ embeddingStatus: status }),
});
