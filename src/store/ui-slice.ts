// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import type { AppStore, UiSlice, ActiveView, PreemptionSubView } from './types';

const VALID_VIEWS: readonly string[] = ['briefing', 'results', 'preemption'] satisfies ActiveView[];
const PREEMPTION_SUB_VIEWS: readonly string[] = ['worklist', 'blindspots', 'knowledge'] satisfies PreemptionSubView[];

/** AD-054 folded these former main tabs into Preemption. An old id selects
 *  Preemption with the matching sub-view instead of being dropped. */
const LEGACY_VIEW_TO_SUB: Readonly<Record<string, PreemptionSubView>> = {
  blindspots: 'blindspots',
  knowledge: 'knowledge',
};

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  showSettings: false,
  settingsInitialTab: null,
  showSplash: true,
  activeView: 'briefing',
  signalViewMode: 'list',
  preemptionSubView: 'worklist',
  isFirstRun: false,
  firstRunDismissed: false,
  embeddingMode: null,
  embeddingStatus: undefined,
  searchFocusItemId: null,

  setShowSettings: (show) => set({ showSettings: show }),
  setSettingsInitialTab: (tab) => set({ settingsInitialTab: tab }),
  setActiveView: (view) => {
    const legacySub = Object.prototype.hasOwnProperty.call(LEGACY_VIEW_TO_SUB, view)
      ? LEGACY_VIEW_TO_SUB[view]
      : undefined;
    if (legacySub !== undefined) {
      set({ activeView: 'preemption', preemptionSubView: legacySub });
    } else if (VALID_VIEWS.includes(view)) {
      set({ activeView: view as ActiveView });
    }
  },
  setSignalViewMode: (mode) => set({ signalViewMode: mode }),
  setPreemptionSubView: (sub) => {
    if (PREEMPTION_SUB_VIEWS.includes(sub)) set({ preemptionSubView: sub });
  },
  openPreemption: (sub) => {
    if (PREEMPTION_SUB_VIEWS.includes(sub)) set({ activeView: 'preemption', preemptionSubView: sub });
  },
  setIsFirstRun: (v) => set({ isFirstRun: v }),
  setFirstRunDismissed: (v) => set({ firstRunDismissed: v }),
  setEmbeddingMode: (mode) => set({ embeddingMode: mode }),
  setEmbeddingStatus: (status) => set({ embeddingStatus: status }),
  setSearchFocusItemId: (id) => set({ searchFocusItemId: id }),
});
