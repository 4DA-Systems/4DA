import type { StateCreator } from 'zustand';
import { ALL_SOURCE_IDS } from '../config/sources';
import type { AppStore, FiltersSlice } from './types';

export const createFiltersSlice: StateCreator<AppStore, [], [], FiltersSlice> = (set) => ({
  sourceFilters: new Set(ALL_SOURCE_IDS),
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

  setSortBy: (sort) => set({ sortBy: sort }),
  setShowOnlyRelevant: (show) => set({ showOnlyRelevant: show }),
  setSearchQuery: (q) => set({ searchQuery: q }),
  setShowSavedOnly: (show) => set({ showSavedOnly: show }),
});
