import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Configurable mock store state
let mockStoreState: Record<string, unknown> = {};

function setMockStoreState(overrides: Record<string, unknown>) {
  mockStoreState = {
    appState: { relevanceResults: [] },
    feedbackGiven: {},
    recordInteraction: vi.fn(),
    setSettingsStatus: vi.fn(),
    sourceFilters: new Set(['hackernews', 'arxiv', 'reddit', 'github']),
    sortBy: 'score' as const,
    showOnlyRelevant: false,
    showSavedOnly: false,
    searchQuery: '',
    toggleSourceFilter: vi.fn(),
    setSortBy: vi.fn(),
    setShowOnlyRelevant: vi.fn(),
    setShowSavedOnly: vi.fn(),
    setSearchQuery: vi.fn(),
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockStoreState)),
}));

// Import after mock setup
import { useResultFilters } from './use-result-filters';

function makeItem(id: number, overrides: Record<string, unknown> = {}) {
  return {
    id,
    title: `Item ${id}`,
    url: `https://example.com/${id}`,
    top_score: 0.5,
    relevant: true,
    explanation: 'Test',
    source_type: 'hackernews',
    ...overrides,
  };
}

describe('useResultFilters', () => {
  beforeEach(() => {
    setMockStoreState({});
  });

  describe('basic filtering', () => {
    it('returns all items when no filters active', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [makeItem(1), makeItem(2), makeItem(3)],
        },
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(3);
    });

    it('filters by source type', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { source_type: 'hackernews' }),
            makeItem(2, { source_type: 'arxiv' }),
            makeItem(3, { source_type: 'reddit' }),
          ],
        },
        sourceFilters: new Set(['hackernews', 'arxiv']),
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(2);
      expect(result.current.filteredResults.map((r: { id: number; source_type?: string }) => r.source_type)).toEqual(['hackernews', 'arxiv']);
    });

    it('filters by relevance when showOnlyRelevant is true', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { relevant: true, top_score: 0.8 }),
            makeItem(2, { relevant: false, top_score: 0.2 }),
            makeItem(3, { relevant: true, top_score: 0.6 }),
          ],
        },
        showOnlyRelevant: true,
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(2);
    });

    it('filters by saved only when showSavedOnly is true', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [makeItem(1), makeItem(2), makeItem(3)],
        },
        feedbackGiven: { 1: 'save', 2: 'dismiss' },
        showSavedOnly: true,
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(1);
      expect(result.current.filteredResults[0].id).toBe(1);
    });

    it('filters by search query matching title', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { title: 'Rust async runtime' }),
            makeItem(2, { title: 'Python web framework' }),
            makeItem(3, { title: 'Rust memory model' }),
          ],
        },
        searchQuery: 'rust',
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(2);
    });

    it('filters by search query matching explanation', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { title: 'Generic Title', explanation: 'Matches your Tauri project' }),
            makeItem(2, { title: 'Other', explanation: 'Not relevant' }),
          ],
        },
        searchQuery: 'tauri',
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(1);
    });
  });

  describe('deduplication', () => {
    it('deduplicates items with the same URL keeping highest score', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://example.com/article', top_score: 0.8, source_type: 'hackernews' }),
            makeItem(2, { url: 'https://example.com/article', top_score: 0.6, source_type: 'reddit' }),
          ],
        },
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(1);
      expect(result.current.filteredResults[0].top_score).toBe(0.8);
    });

    it('normalizes URLs for dedup (strips protocol, www, trailing slash)', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://www.example.com/article/', top_score: 0.6 }),
            makeItem(2, { url: 'http://example.com/article', top_score: 0.8 }),
          ],
        },
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(1);
      expect(result.current.filteredResults[0].top_score).toBe(0.8);
    });

    it('tags deduplicated items with seen_on sources', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://example.com/same', top_score: 0.8, source_type: 'hackernews' }),
            makeItem(2, { url: 'https://example.com/same', top_score: 0.6, source_type: 'reddit' }),
          ],
        },
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults[0].seen_on).toEqual(
        expect.arrayContaining(['hackernews', 'reddit']),
      );
    });

    it('keeps items with no URL without dedup', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: null }),
            makeItem(2, { url: null }),
          ],
        },
      });
      const { result } = renderHook(() => useResultFilters());
      expect(result.current.filteredResults).toHaveLength(2);
    });
  });

  describe('sorting', () => {
    it('sorts by score descending when sortBy is score', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://a.com', top_score: 0.3 }),
            makeItem(2, { url: 'https://b.com', top_score: 0.9 }),
            makeItem(3, { url: 'https://c.com', top_score: 0.6 }),
          ],
        },
        sortBy: 'score',
      });
      const { result } = renderHook(() => useResultFilters());
      const scores = result.current.filteredResults.map((r: { top_score: number }) => r.top_score);
      expect(scores).toEqual([0.9, 0.6, 0.3]);
    });

    it('sorts by id descending (newest first) when sortBy is date', () => {
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://a.com', top_score: 0.9 }),
            makeItem(3, { url: 'https://b.com', top_score: 0.3 }),
            makeItem(2, { url: 'https://c.com', top_score: 0.6 }),
          ],
        },
        sortBy: 'date',
      });
      const { result } = renderHook(() => useResultFilters());
      const ids = result.current.filteredResults.map((r: { id: number }) => r.id);
      expect(ids).toEqual([3, 2, 1]);
    });
  });

  describe('batch operations', () => {
    it('dismissAllBelow dismisses items below threshold', async () => {
      const recordFn = vi.fn().mockResolvedValue(undefined);
      const setStatus = vi.fn();
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://a.com', top_score: 0.8 }),
            makeItem(2, { url: 'https://b.com', top_score: 0.2 }),
            makeItem(3, { url: 'https://c.com', top_score: 0.1 }),
          ],
        },
        recordInteraction: recordFn,
        setSettingsStatus: setStatus,
      });
      const { result } = renderHook(() => useResultFilters());
      await act(async () => {
        await result.current.dismissAllBelow(0.3);
      });
      // Items 2 and 3 are below 0.3
      expect(recordFn).toHaveBeenCalledTimes(2);
      expect(setStatus).toHaveBeenCalledWith(expect.stringContaining('Dismissed 2'));
    });

    it('saveAllAbove saves items above threshold', async () => {
      const recordFn = vi.fn().mockResolvedValue(undefined);
      const setStatus = vi.fn();
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://a.com', top_score: 0.8 }),
            makeItem(2, { url: 'https://b.com', top_score: 0.7 }),
            makeItem(3, { url: 'https://c.com', top_score: 0.3 }),
          ],
        },
        recordInteraction: recordFn,
        setSettingsStatus: setStatus,
      });
      const { result } = renderHook(() => useResultFilters());
      await act(async () => {
        await result.current.saveAllAbove(0.6);
      });
      // Items 1 and 2 are above 0.6
      expect(recordFn).toHaveBeenCalledTimes(2);
      expect(setStatus).toHaveBeenCalledWith(expect.stringContaining('Saved 2'));
    });

    it('skips items that already have feedback', async () => {
      const recordFn = vi.fn().mockResolvedValue(undefined);
      setMockStoreState({
        appState: {
          relevanceResults: [
            makeItem(1, { url: 'https://a.com', top_score: 0.1 }),
            makeItem(2, { url: 'https://b.com', top_score: 0.1 }),
          ],
        },
        feedbackGiven: { 1: 'save' },
        recordInteraction: recordFn,
        setSettingsStatus: vi.fn(),
      });
      const { result } = renderHook(() => useResultFilters());
      await act(async () => {
        await result.current.dismissAllBelow(0.5);
      });
      // Only item 2 should be dismissed (item 1 already has feedback)
      expect(recordFn).toHaveBeenCalledTimes(1);
    });
  });
});
