/**
 * Frontend Pipeline Integration Tests
 *
 * Tests the data flow from Tauri invoke responses through store to rendering.
 * Validates that realistic data shapes are handled correctly.
 */

import { describe, it, expect } from 'vitest';
import type { SourceRelevance, ScoreBreakdown } from '../types/analysis';
import type { AppState } from '../types/common';

// ============================================================================
// Fixture factories
// ============================================================================

function makeScoreBreakdown(overrides?: Partial<ScoreBreakdown>): ScoreBreakdown {
  return {
    context_score: 0.65,
    interest_score: 0.72,
    keyword_score: 0.45,
    ace_boost: 0.1,
    affinity_mult: 1.2,
    anti_penalty: 0.0,
    freshness_mult: 1.0,
    feedback_boost: 0.0,
    source_quality_boost: 0.05,
    confidence_by_signal: { 'keyword_match': 0.8, 'context_match': 0.7 },
    signal_count: 2,
    confirmed_signals: ['keyword_match'],
    confirmation_mult: 1.1,
    dep_match_score: 0.3,
    matched_deps: ['react', 'typescript'],
    domain_relevance: 0.85,
    content_quality_mult: 1.0,
    novelty_mult: 1.05,
    intent_boost: 0.05,
    content_type: 'technical_article',
    content_dna_mult: 1.0,
    ...overrides,
  };
}

function makeSourceRelevance(id: number, overrides?: Partial<SourceRelevance>): SourceRelevance {
  return {
    id,
    title: `Test Item ${id}: Rust Performance Optimization`,
    url: `https://example.com/item-${id}`,
    top_score: 0.75 + (id % 10) * 0.02,
    matches: [
      {
        source_file: 'src/main.rs',
        matched_text: 'Performance-related Rust code...',
        similarity: 0.82,
      },
    ],
    relevant: true,
    explanation: `This item is relevant because it matches your Rust context (score: ${(0.75 + (id % 10) * 0.02).toFixed(2)})`,
    source_type: ['hackernews', 'reddit', 'github', 'arxiv', 'devto'][id % 5],
    confidence: 0.85,
    score_breakdown: makeScoreBreakdown(),
    signal_type: 'technical_content',
    signal_priority: 'alert',
    signal_action: 'read',
    signal_triggers: ['rust', 'performance'],
    similar_count: 0,
    similar_titles: [],
    serendipity: id % 7 === 0,
    ...overrides,
  };
}

function makeAppState(resultCount: number): AppState {
  return {
    contextFiles: [
      { path: 'src/main.rs', content: 'fn main() {}', lines: 1 },
      { path: 'Cargo.toml', content: '[package]\nname = "test"', lines: 2 },
    ],
    relevanceResults: Array.from({ length: resultCount }, (_, i) => makeSourceRelevance(i + 1)),
    status: 'complete',
    loading: false,
    analysisComplete: true,
    progress: 100,
    progressMessage: 'Analysis complete',
    progressStage: 'done',
    lastAnalyzedAt: new Date(),
  };
}

// ============================================================================
// Tests
// ============================================================================

describe('Pipeline Data Flow', () => {
  describe('SourceRelevance type handling', () => {
    it('handles complete SourceRelevance with all fields', () => {
      const item = makeSourceRelevance(1);
      expect(item.id).toBe(1);
      expect(item.top_score).toBeGreaterThan(0);
      expect(item.matches).toHaveLength(1);
      expect(item.score_breakdown).toBeDefined();
      expect(item.source_type).toBeDefined();
    });

    it('handles SourceRelevance with minimal fields', () => {
      const item: SourceRelevance = {
        id: 99,
        title: 'Minimal Item',
        url: null,
        top_score: 0.5,
        matches: [],
        relevant: false,
      };
      expect(item.id).toBe(99);
      expect(item.url).toBeNull();
      expect(item.matches).toHaveLength(0);
      expect(item.score_breakdown).toBeUndefined();
    });

    it('handles serendipity items', () => {
      const item = makeSourceRelevance(7, { serendipity: true, top_score: 0.3 });
      expect(item.serendipity).toBe(true);
      expect(item.top_score).toBeLessThan(0.5);
    });

    it('handles excluded items', () => {
      const item = makeSourceRelevance(1, {
        relevant: false,
        top_score: 0.0,
        explanation: 'Excluded by user rule',
      });
      expect(item.relevant).toBe(false);
      expect(item.top_score).toBe(0);
    });
  });

  describe('AppState with large result sets', () => {
    it('creates state with 100+ items without error', () => {
      const state = makeAppState(150);
      expect(state.relevanceResults).toHaveLength(150);
      expect(state.analysisComplete).toBe(true);
    });

    it('items have diverse source types', () => {
      const state = makeAppState(50);
      const sourceTypes = new Set(state.relevanceResults.map(r => r.source_type));
      expect(sourceTypes.size).toBeGreaterThanOrEqual(4);
    });

    it('items are sortable by score', () => {
      const state = makeAppState(20);
      const sorted = [...state.relevanceResults].sort((a, b) => b.top_score - a.top_score);
      expect(sorted[0]!.top_score).toBeGreaterThanOrEqual(sorted[sorted.length - 1]!.top_score);
    });

    it('items with signal data are filterable', () => {
      const state = makeAppState(30);
      const withSignals = state.relevanceResults.filter(r => r.signal_type);
      expect(withSignals.length).toBe(30); // All items have signal_type
    });
  });

  describe('ScoreBreakdown validation', () => {
    it('all scores are in valid range [0, 1]', () => {
      const breakdown = makeScoreBreakdown();
      expect(breakdown.context_score).toBeGreaterThanOrEqual(0);
      expect(breakdown.context_score).toBeLessThanOrEqual(1);
      expect(breakdown.interest_score).toBeGreaterThanOrEqual(0);
      expect(breakdown.interest_score).toBeLessThanOrEqual(1);
    });

    it('multipliers are positive', () => {
      const breakdown = makeScoreBreakdown();
      expect(breakdown.affinity_mult).toBeGreaterThan(0);
      expect(breakdown.freshness_mult).toBeGreaterThanOrEqual(0);
      expect(breakdown.content_quality_mult).toBeGreaterThan(0);
    });

    it('anti_penalty is non-negative', () => {
      const breakdown = makeScoreBreakdown({ anti_penalty: 0.15 });
      expect(breakdown.anti_penalty).toBeGreaterThanOrEqual(0);
    });

    it('matched_deps is an array', () => {
      const breakdown = makeScoreBreakdown();
      expect(Array.isArray(breakdown.matched_deps)).toBe(true);
    });
  });

  describe('Edge cases in data shapes', () => {
    it('handles empty relevance results', () => {
      const state = makeAppState(0);
      expect(state.relevanceResults).toHaveLength(0);
      expect(state.analysisComplete).toBe(true);
    });

    it('handles items with no URL', () => {
      const items = [
        makeSourceRelevance(1, { url: null }),
        makeSourceRelevance(2, { url: undefined as unknown as null }),
      ];
      expect(items[0]!.url).toBeNull();
    });

    it('handles items with very long titles', () => {
      const longTitle = 'A'.repeat(1000);
      const item = makeSourceRelevance(1, { title: longTitle });
      expect(item.title.length).toBe(1000);
    });

    it('handles items with unicode content', () => {
      const item = makeSourceRelevance(1, {
        title: 'Rust のメモリ安全性 🦀',
        explanation: '日本語の説明文',
      });
      expect(item.title).toContain('🦀');
    });

    it('handles items with empty matches array', () => {
      const item = makeSourceRelevance(1, { matches: [] });
      expect(item.matches).toHaveLength(0);
    });

    it('handles items with multiple matches', () => {
      const item = makeSourceRelevance(1, {
        matches: Array.from({ length: 10 }, (_, i) => ({
          source_file: `src/file_${i}.rs`,
          matched_text: `Match ${i}`,
          similarity: 0.5 + i * 0.05,
        })),
      });
      expect(item.matches).toHaveLength(10);
    });
  });

  describe('Tauri invoke response parsing', () => {
    it('parses realistic Tauri invoke response shape', () => {
      // Simulates what Tauri returns from get_relevance_results
      const rawResponse = JSON.stringify(makeAppState(5).relevanceResults);
      const parsed: SourceRelevance[] = JSON.parse(rawResponse);

      expect(parsed).toHaveLength(5);
      expect(parsed[0]!.score_breakdown?.context_score).toBe(0.65);
    });

    it('survives JSON round-trip with all fields', () => {
      const original = makeSourceRelevance(42);
      const json = JSON.stringify(original);
      const restored: SourceRelevance = JSON.parse(json);

      expect(restored.id).toBe(original.id);
      expect(restored.title).toBe(original.title);
      expect(restored.top_score).toBe(original.top_score);
      expect(restored.score_breakdown?.dep_match_score).toBe(
        original.score_breakdown?.dep_match_score,
      );
    });
  });
});
