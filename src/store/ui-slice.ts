import type { StateCreator } from 'zustand';
import type { AppStore, UiSlice } from './types';

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  showSettings: false,
  showSplash: true,
  activeView: 'briefing',
  isFirstRun: false,
  firstRunDismissed: false,
  embeddingMode: null,

  setShowSettings: (show) => set({ showSettings: show }),
  setShowSplash: (show) => set({ showSplash: show }),
  setActiveView: (view) => set({ activeView: view }),
  setIsFirstRun: (v) => set({ isFirstRun: v }),
  setFirstRunDismissed: (v) => set({ firstRunDismissed: v }),
  setEmbeddingMode: (mode) => set({ embeddingMode: mode }),
});
