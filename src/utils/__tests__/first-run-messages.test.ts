// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for first-run narration message utilities.
 *
 * Covers getStageNarration, getSourceNarration, getCelebrationMessage.
 */
import { describe, it, expect, vi } from 'vitest';

// Mock the sources config (imported by first-run-messages)
vi.mock('../../config/sources', () => ({
  getSourceLabel: (s: string) => s.charAt(0).toUpperCase() + s.slice(1),
}));

import { getStageNarration, getSourceNarration, getCelebrationMessage } from '../first-run-messages';

describe('getStageNarration', () => {
  it('returns narration for init stage', () => {
    expect(getStageNarration('init')).toContain('intelligence engine');
  });

  it('returns narration for context stage', () => {
    expect(getStageNarration('context')).toContain('project context');
  });

  it('returns narration for fetch stage', () => {
    expect(getStageNarration('fetch')).toContain('intelligence sources');
  });

  it('returns narration for scrape stage', () => {
    expect(getStageNarration('scrape')).toContain('article content');
  });

  it('returns narration for embed stage', () => {
    expect(getStageNarration('embed')).toContain('semantic understanding');
  });

  it('returns narration for relevance stage', () => {
    expect(getStageNarration('relevance')).toContain('Scoring');
  });

  it('returns narration for rerank stage', () => {
    expect(getStageNarration('rerank')).toContain('AI');
  });

  it('returns narration for complete stage', () => {
    expect(getStageNarration('complete')).toContain('complete');
  });

  it('returns fallback for unknown stage', () => {
    expect(getStageNarration('unknown')).toBe('Processing...');
  });
});

describe('getSourceNarration', () => {
  it('returns zero-count message', () => {
    const msg = getSourceNarration('hackernews', 0);
    expect(msg).toContain('nothing new');
  });

  it('returns singular message for 1 item', () => {
    const msg = getSourceNarration('reddit', 1);
    expect(msg).toContain('1 item');
  });

  it('returns plural message for multiple items', () => {
    const msg = getSourceNarration('github', 5);
    expect(msg).toContain('5 items');
  });
});

describe('getCelebrationMessage', () => {
  it('returns learning message when 0 relevant', () => {
    const msg = getCelebrationMessage(0, 50);
    expect(msg).toContain('learning');
    expect(msg).toContain('50');
  });

  it('returns tailored message for 1-3 items', () => {
    const msg = getCelebrationMessage(2, 30);
    expect(msg).toContain('2');
    expect(msg).toContain('tailored');
  });

  it('returns profile match message for 4-10 items', () => {
    const msg = getCelebrationMessage(7, 50);
    expect(msg).toContain('7');
    expect(msg).toContain('profile');
  });

  it('returns discovery message for 11+ items', () => {
    const msg = getCelebrationMessage(15, 100);
    expect(msg).toContain('15');
    expect(msg).toContain('relevant');
  });
});
