/**
 * Tests for useBriefingDerived hook.
 *
 * Tests derived computations: gaps, lowQualitySources, healthSummary,
 * sections, isStale, signalItems, topItems.
 */
import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useBriefingDerived } from '../use-briefing-derived';
import type { SourceRelevance, SourceHealthStatus } from '../../types';
import type { BriefingState } from '../../store';

function makeResult(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 1,
    title: 'Test',
    url: 'https://example.com',
    top_score: 0.5,
    matches: [],
    relevant: true,
    source_type: 'hackernews',
    ...overrides,
  };
}

function makeBriefingState(overrides: Partial<BriefingState> = {}): BriefingState {
  return {
    content: null,
    loading: false,
    error: null,
    model: null,
    lastGenerated: null,
    ...overrides,
  };
}

describe('useBriefingDerived', () => {
  describe('gaps', () => {
    it('returns empty gaps when all sources are healthy', () => {
      const health: SourceHealthStatus[] = [
        { source_type: 'hackernews', status: 'healthy', last_success_relative: '2024-01-01', items_fetched: 10, gap_message: null },
      ];
      const { result } = renderHook(() =>
        useBriefingDerived([], health, makeBriefingState(), null),
      );
      expect(result.current.gaps).toEqual([]);
    });

    it('returns gaps for non-healthy sources with gap_message', () => {
      const health: SourceHealthStatus[] = [
        { source_type: 'reddit', status: 'error', gap_message: 'Rate limited', last_success_relative: null, items_fetched: 0 },
      ];
      const { result } = renderHook(() =>
        useBriefingDerived([], health, makeBriefingState(), null),
      );
      expect(result.current.gaps).toHaveLength(1);
    });
  });

  describe('lowQualitySources', () => {
    it('returns empty when fewer than 10 results', () => {
      const results = Array.from({ length: 5 }, (_, i) => makeResult({ id: i }));
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.lowQualitySources).toEqual([]);
    });

    it('flags sources with less than 5% relevance ratio', () => {
      // 10 items from 'spam_source', none relevant
      const results = Array.from({ length: 10 }, (_, i) =>
        makeResult({ id: i, source_type: 'spam_source', relevant: false }),
      );
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.lowQualitySources).toHaveLength(1);
      expect(result.current.lowQualitySources[0]!.source).toBe('spam_source');
      expect(result.current.lowQualitySources[0]!.ratio).toBe(0);
    });

    it('does not flag sources with good relevance ratio', () => {
      const results = Array.from({ length: 10 }, (_, i) =>
        makeResult({ id: i, source_type: 'hackernews', relevant: true }),
      );
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.lowQualitySources).toEqual([]);
    });
  });

  describe('healthSummary', () => {
    it('returns null when sourceHealth is empty', () => {
      const { result } = renderHook(() =>
        useBriefingDerived([], [], makeBriefingState(), null),
      );
      expect(result.current.healthSummary).toBeNull();
    });

    it('computes healthy/total counts correctly', () => {
      const health: SourceHealthStatus[] = [
        { source_type: 'hackernews', status: 'healthy', last_success_relative: '2024-01-01', items_fetched: 10, gap_message: null },
        { source_type: 'reddit', status: 'error', last_success_relative: null, items_fetched: 0, gap_message: null },
      ];
      const { result } = renderHook(() =>
        useBriefingDerived([], health, makeBriefingState(), null),
      );
      expect(result.current.healthSummary).toEqual({ healthy: 1, total: 2, allHealthy: false });
    });

    it('reports allHealthy when all sources are healthy', () => {
      const health: SourceHealthStatus[] = [
        { source_type: 'hackernews', status: 'healthy', last_success_relative: '2024-01-01', items_fetched: 10, gap_message: null },
        { source_type: 'reddit', status: 'healthy', last_success_relative: '2024-01-01', items_fetched: 5, gap_message: null },
      ];
      const { result } = renderHook(() =>
        useBriefingDerived([], health, makeBriefingState(), null),
      );
      expect(result.current.healthSummary!.allHealthy).toBe(true);
    });
  });

  describe('sections', () => {
    it('returns empty array when briefing content is null', () => {
      const { result } = renderHook(() =>
        useBriefingDerived([], [], makeBriefingState(), null),
      );
      expect(result.current.sections).toEqual([]);
    });

    it('parses briefing content into sections', () => {
      const briefing = makeBriefingState({ content: '## Action Required\n- Fix X\n## Worth Knowing\n- New Y' });
      const { result } = renderHook(() =>
        useBriefingDerived([], [], briefing, null),
      );
      expect(result.current.sections).toHaveLength(2);
      expect(result.current.sections[0]!.title).toBe('Action Required');
    });
  });

  describe('isStale', () => {
    it('returns false when no lastGenerated', () => {
      const { result } = renderHook(() =>
        useBriefingDerived([], [], makeBriefingState(), null),
      );
      expect(result.current.isStale).toBe(false);
    });

    it('returns false when no lastBackgroundResultsAt', () => {
      const briefing = makeBriefingState({ lastGenerated: new Date() });
      const { result } = renderHook(() =>
        useBriefingDerived([], [], briefing, null),
      );
      expect(result.current.isStale).toBe(false);
    });

    it('returns true when background results are newer than briefing', () => {
      const briefing = makeBriefingState({ lastGenerated: new Date(Date.now() - 60000) });
      const bgTime = new Date();
      const { result } = renderHook(() =>
        useBriefingDerived([], [], briefing, bgTime),
      );
      expect(result.current.isStale).toBe(true);
    });

    it('returns false when briefing is newer than background results', () => {
      const bgTime = new Date(Date.now() - 60000);
      const briefing = makeBriefingState({ lastGenerated: new Date() });
      const { result } = renderHook(() =>
        useBriefingDerived([], [], briefing, bgTime),
      );
      expect(result.current.isStale).toBe(false);
    });
  });

  describe('signalItems', () => {
    it('returns empty when no critical/high signals', () => {
      const results = [makeResult({ signal_priority: 'watch' })];
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.signalItems).toEqual([]);
    });

    it('returns critical and high priority items', () => {
      const results = [
        makeResult({ id: 1, signal_priority: 'critical' }),
        makeResult({ id: 2, signal_priority: 'alert' }),
        makeResult({ id: 3, signal_priority: 'watch' }),
      ];
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.signalItems).toHaveLength(2);
    });

    it('limits to 3 signal items maximum', () => {
      const results = Array.from({ length: 5 }, (_, i) =>
        makeResult({ id: i, signal_priority: 'critical' }),
      );
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.signalItems).toHaveLength(3);
    });
  });

  describe('topItems', () => {
    it('returns relevant items with score >= 0.5', () => {
      const results = [
        makeResult({ id: 1, top_score: 0.8, relevant: true }),
        makeResult({ id: 2, top_score: 0.3, relevant: true }),
        makeResult({ id: 3, top_score: 0.6, relevant: false }),
      ];
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.topItems).toHaveLength(1);
      expect(result.current.topItems[0]!.id).toBe(1);
    });

    it('excludes signal items from top picks to avoid duplicates', () => {
      const results = [
        makeResult({ id: 1, top_score: 0.9, relevant: true, signal_priority: 'critical' }),
        makeResult({ id: 2, top_score: 0.7, relevant: true }),
      ];
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.topItems).toHaveLength(1);
      expect(result.current.topItems[0]!.id).toBe(2);
    });

    it('limits to 8 top items', () => {
      const results = Array.from({ length: 15 }, (_, i) =>
        makeResult({ id: i, top_score: 0.8, relevant: true }),
      );
      const { result } = renderHook(() =>
        useBriefingDerived(results, [], makeBriefingState(), null),
      );
      expect(result.current.topItems.length).toBeLessThanOrEqual(8);
    });
  });
});
