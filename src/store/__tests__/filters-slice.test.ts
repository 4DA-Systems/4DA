import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('filters-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has sourceFilters containing all sources', () => {
      const filters = useAppStore.getState().sourceFilters;
      expect(filters).toBeInstanceOf(Set);
      expect(filters.size).toBeGreaterThan(0);
      expect(filters.has('hackernews')).toBe(true);
      expect(filters.has('github')).toBe(true);
      expect(filters.has('reddit')).toBe(true);
    });

    it('has sortBy defaulting to score', () => {
      expect(useAppStore.getState().sortBy).toBe('score');
    });

    it('has showOnlyRelevant defaulting to true', () => {
      expect(useAppStore.getState().showOnlyRelevant).toBe(true);
    });

    it('has empty searchQuery', () => {
      expect(useAppStore.getState().searchQuery).toBe('');
    });

    it('has showSavedOnly defaulting to false', () => {
      expect(useAppStore.getState().showSavedOnly).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // toggleSourceFilter
  // ---------------------------------------------------------------------------
  describe('toggleSourceFilter', () => {
    it('removes a source from filters when present', () => {
      useAppStore.getState().toggleSourceFilter('hackernews');
      expect(useAppStore.getState().sourceFilters.has('hackernews')).toBe(false);
    });

    it('adds a source back when toggled again', () => {
      useAppStore.getState().toggleSourceFilter('hackernews');
      useAppStore.getState().toggleSourceFilter('hackernews');
      expect(useAppStore.getState().sourceFilters.has('hackernews')).toBe(true);
    });

    it('does not allow removing the last source', () => {
      // Remove all sources except one
      const filters = useAppStore.getState().sourceFilters;
      const sources = Array.from(filters);

      // Remove all but the last
      for (let i = 0; i < sources.length - 1; i++) {
        useAppStore.getState().toggleSourceFilter(sources[i]);
      }

      // The last source should remain
      const remaining = useAppStore.getState().sourceFilters;
      expect(remaining.size).toBe(1);

      // Trying to toggle the last source should keep it
      const lastSource = Array.from(remaining)[0];
      useAppStore.getState().toggleSourceFilter(lastSource);
      expect(useAppStore.getState().sourceFilters.has(lastSource)).toBe(true);
      expect(useAppStore.getState().sourceFilters.size).toBe(1);
    });
  });

  // ---------------------------------------------------------------------------
  // setSortBy
  // ---------------------------------------------------------------------------
  describe('setSortBy', () => {
    it('sets sort to date', () => {
      useAppStore.getState().setSortBy('date');
      expect(useAppStore.getState().sortBy).toBe('date');
    });

    it('sets sort back to score', () => {
      useAppStore.getState().setSortBy('date');
      useAppStore.getState().setSortBy('score');
      expect(useAppStore.getState().sortBy).toBe('score');
    });
  });

  // ---------------------------------------------------------------------------
  // setShowOnlyRelevant
  // ---------------------------------------------------------------------------
  describe('setShowOnlyRelevant', () => {
    it('sets showOnlyRelevant to false', () => {
      useAppStore.getState().setShowOnlyRelevant(false);
      expect(useAppStore.getState().showOnlyRelevant).toBe(false);
    });

    it('sets showOnlyRelevant back to true', () => {
      useAppStore.getState().setShowOnlyRelevant(false);
      useAppStore.getState().setShowOnlyRelevant(true);
      expect(useAppStore.getState().showOnlyRelevant).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // setSearchQuery
  // ---------------------------------------------------------------------------
  describe('setSearchQuery', () => {
    it('sets a search query', () => {
      useAppStore.getState().setSearchQuery('rust async');
      expect(useAppStore.getState().searchQuery).toBe('rust async');
    });

    it('can clear the search query', () => {
      useAppStore.getState().setSearchQuery('test');
      useAppStore.getState().setSearchQuery('');
      expect(useAppStore.getState().searchQuery).toBe('');
    });

    it('replaces previous query', () => {
      useAppStore.getState().setSearchQuery('first');
      useAppStore.getState().setSearchQuery('second');
      expect(useAppStore.getState().searchQuery).toBe('second');
    });
  });

  // ---------------------------------------------------------------------------
  // setShowSavedOnly
  // ---------------------------------------------------------------------------
  describe('setShowSavedOnly', () => {
    it('sets showSavedOnly to true', () => {
      useAppStore.getState().setShowSavedOnly(true);
      expect(useAppStore.getState().showSavedOnly).toBe(true);
    });

    it('sets showSavedOnly back to false', () => {
      useAppStore.getState().setShowSavedOnly(true);
      useAppStore.getState().setShowSavedOnly(false);
      expect(useAppStore.getState().showSavedOnly).toBe(false);
    });
  });
});
