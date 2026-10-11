// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { ENABLED_SOURCE_IDS, markSourceEnabled } from '../config/sources';
import type { AppStore, FiltersSlice } from './types';

export const createFiltersSlice: StateCreator<AppStore, [], [], FiltersSlice> = (set, get) => ({
  sourceFilters: new Set(ENABLED_SOURCE_IDS),
  sortBy: 'score',
  showOnlyRelevant: true,
  searchQuery: '',
  showSavedOnly: false,

  toggleSourceFilter: (source) => {
    set(state => {
      const next = new Set(state.sourceFilters);
      if (next.has(source)) {
        if (next.size > 1) next.delete(source);
      } else {
        next.add(source);
      }
      return { sourceFilters: next };
    });
  },

  // Re-read ENABLED_SOURCE_IDS at call time (it's populated dynamically by loadSourceMeta)
  resetSourceFilters: () => set({ sourceFilters: new Set(ENABLED_SOURCE_IDS) }),

  // Settings turned a source on or off (AD-054). Keep the Signal filter and
  // the displayed results in step with it, without waiting for a reload.
  applySourceEnabled: (source, enabled) => {
    set(state => {
      // Was the filter showing everything the user reads? Checked before the
      // change, so a newly turned-on source joins an unfiltered view but does
      // not widen a view the user narrowed by hand.
      const wasUnfiltered = [...ENABLED_SOURCE_IDS].every(id => state.sourceFilters.has(id));
      markSourceEnabled(source, enabled);
      // Always a new Set: the chips read the enabled list from source
      // metadata, so a new identity is what re-renders them.
      let sourceFilters = new Set(state.sourceFilters);
      if (enabled) {
        if (wasUnfiltered) sourceFilters.add(source);
      } else {
        sourceFilters.delete(source);
        if (sourceFilters.size === 0) sourceFilters = new Set(ENABLED_SOURCE_IDS);
      }
      if (enabled) return { sourceFilters };
      // Mirror the backend's drop from the display set (set_source_enabled):
      // a turned-off source's items leave the list and every count at once.
      const keep = (r: { source_type?: string }) => (r.source_type || 'hackernews') !== source;
      return {
        sourceFilters,
        appState: {
          ...state.appState,
          relevanceResults: state.appState.relevanceResults.filter(keep),
          nearMisses: state.appState.nearMisses?.filter(keep) ?? null,
        },
      };
    });
    // Source health lists only enabled sources, so the cold-start gate and
    // the health rows follow the toggle now, not at the next analysis.
    void get().loadSourceHealth();
  },

  setSortBy: (sort) => set({ sortBy: sort }),
  setShowOnlyRelevant: (show) => set({ showOnlyRelevant: show }),
  setSearchQuery: (q) => set({ searchQuery: q }),
  setShowSavedOnly: (show) => set({ showSavedOnly: show }),
});
