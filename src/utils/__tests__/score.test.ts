// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for score utility functions.
 *
 * Covers formatScore, getScoreColor, getScoreFactorKeys, formatRelativeAge, getStageLabel.
 */
import { describe, it, expect } from 'vitest';
import { formatScore, getScoreColor, getScoreFactorKeys, formatRelativeAge, getStageLabel } from '../score';
import type { SourceRelevance } from '../../types';

function makeMinimalItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
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

describe('formatScore', () => {
  it('formats 0 as 0%', () => {
    expect(formatScore(0)).toBe('0%');
  });

  it('formats 1 as 100%', () => {
    expect(formatScore(1)).toBe('100%');
  });

  it('formats 0.42 as 42%', () => {
    expect(formatScore(0.42)).toBe('42%');
  });

  it('rounds 0.555 to 56%', () => {
    expect(formatScore(0.555)).toBe('56%');
  });

  it('rounds 0.001 to 0%', () => {
    expect(formatScore(0.001)).toBe('0%');
  });

  it('rounds 0.999 to 100%', () => {
    expect(formatScore(0.999)).toBe('100%');
  });
});

describe('getScoreColor', () => {
  it('returns success for score >= 0.5', () => {
    expect(getScoreColor(0.5)).toBe('text-success');
    expect(getScoreColor(0.8)).toBe('text-success');
    expect(getScoreColor(1.0)).toBe('text-success');
  });

  it('returns gold for score between 0.35 and 0.5', () => {
    expect(getScoreColor(0.35)).toBe('text-accent-gold');
    expect(getScoreColor(0.45)).toBe('text-accent-gold');
    expect(getScoreColor(0.49)).toBe('text-accent-gold');
  });

  it('returns muted for score below 0.35', () => {
    expect(getScoreColor(0.0)).toBe('text-text-muted');
    expect(getScoreColor(0.2)).toBe('text-text-muted');
    expect(getScoreColor(0.34)).toBe('text-text-muted');
  });
});

describe('getScoreFactorKeys', () => {
  it('returns empty array when no breakdown', () => {
    const item = makeMinimalItem();
    expect(getScoreFactorKeys(item)).toEqual([]);
  });

  it('includes stackMatch when context_score > 0.3', () => {
    const item = makeMinimalItem({
      score_breakdown: { context_score: 0.5, interest_score: 0, ace_boost: 0, affinity_mult: 1.0, anti_penalty: 0, confidence_by_signal: {} },
    });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.stackMatch');
  });

  it('includes partialStackMatch when 0 < context_score <= 0.3', () => {
    const item = makeMinimalItem({
      score_breakdown: { context_score: 0.2, interest_score: 0, ace_boost: 0, affinity_mult: 1.0, anti_penalty: 0, confidence_by_signal: {} },
    });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.partialStackMatch');
  });

  it('includes interestMatch when interest_score > 0.3', () => {
    const item = makeMinimalItem({
      score_breakdown: { context_score: 0, interest_score: 0.5, ace_boost: 0, affinity_mult: 1.0, anti_penalty: 0, confidence_by_signal: {} },
    });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.interestMatch');
  });

  it('includes topicAffinity when affinity_mult > 1.05', () => {
    const item = makeMinimalItem({
      score_breakdown: { context_score: 0, interest_score: 0, ace_boost: 0, affinity_mult: 1.2, anti_penalty: 0, confidence_by_signal: {} },
    });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.topicAffinity');
  });

  it('includes decisionWindow when decision_window_match is true', () => {
    const item = makeMinimalItem({ decision_window_match: 'Adopt Rust for backend' });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.decisionWindow');
  });

  it('includes serendipity when item has serendipity flag', () => {
    const item = makeMinimalItem({ serendipity: true });
    expect(getScoreFactorKeys(item)).toContain('scoreTooltip.serendipity');
  });
});

describe('formatRelativeAge', () => {
  it('returns <1h for timestamps less than 1 hour ago', () => {
    const recent = new Date(Date.now() - 30 * 60 * 1000).toISOString();
    expect(formatRelativeAge(recent)).toBe('<1h');
  });

  it('returns hours for timestamps less than 24 hours ago', () => {
    const fiveHours = new Date(Date.now() - 5 * 3600 * 1000).toISOString();
    expect(formatRelativeAge(fiveHours)).toBe('5h');
  });

  it('returns days for timestamps less than 7 days ago', () => {
    const threeDays = new Date(Date.now() - 3 * 24 * 3600 * 1000).toISOString();
    expect(formatRelativeAge(threeDays)).toBe('3d');
  });

  it('returns weeks for timestamps less than 5 weeks ago', () => {
    const twoWeeks = new Date(Date.now() - 14 * 24 * 3600 * 1000).toISOString();
    expect(formatRelativeAge(twoWeeks)).toBe('2w');
  });

  it('returns months for older timestamps', () => {
    const twoMonths = new Date(Date.now() - 60 * 24 * 3600 * 1000).toISOString();
    expect(formatRelativeAge(twoMonths)).toBe('2mo');
  });

  it('returns empty string for invalid timestamps', () => {
    expect(formatRelativeAge('not-a-date')).toBe('');
  });
});

describe('getStageLabel', () => {
  it('maps known stages to labels', () => {
    expect(getStageLabel('init')).toBe('Initializing');
    expect(getStageLabel('context')).toBe('Loading Context');
    expect(getStageLabel('fetch')).toBe('Fetching Sources');
    expect(getStageLabel('scrape')).toBe('Extracting Content');
    expect(getStageLabel('embed')).toBe('Building Embeddings');
    expect(getStageLabel('relevance')).toBe('Scoring Relevance');
    expect(getStageLabel('rerank')).toBe('AI Re-ranking');
    expect(getStageLabel('complete')).toBe('Complete');
  });

  it('returns the stage string itself for unknown stages', () => {
    expect(getStageLabel('custom')).toBe('custom');
    expect(getStageLabel('')).toBe('');
  });
});
