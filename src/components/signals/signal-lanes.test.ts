// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import {
  STACK_FACT_SOURCES, WORTH_LANE_SIZE, flattenVisible, isStackFactRow, laneScore, locateInLanes,
  partitionLanes, visibleLaneItems,
} from './signal-lanes';
import type { SourceRelevance } from '../../types';

let nextId = 1;
function item(partial: Partial<SourceRelevance> & { sb?: Record<string, unknown> } = {}): SourceRelevance {
  const { sb, ...rest } = partial;
  return {
    id: nextId++,
    title: 't',
    url: null,
    top_score: 0.8,
    matches: [],
    relevant: true,
    source_type: 'hackernews',
    ...(sb ? { score_breakdown: sb as never } : {}),
    ...rest,
  };
}
const stackItem = (sb: Record<string, unknown> = {}, extra: Partial<SourceRelevance> = {}) =>
  item({ sb: { strongly_grounded: true, dependency_event: true, ...sb }, ...extra });
const orbit = (top_score: number, sb: Record<string, unknown> = {}) =>
  item({ top_score, sb: { domain_relevance: 0.85, ...sb } });
const ambient = (top_score: number) => item({ top_score, sb: { domain_relevance: 0.2 } });

describe('partitionLanes — the reading feed only (Lane 1 is the backend stack-change stream)', () => {
  it('leaves registry releases and advisories out: Lane 1 states those facts', () => {
    const release = stackItem({}, { source_type: 'crates_io' });
    const npm = stackItem({}, { source_type: 'npm_registry' });
    const advisory = item({ source_type: 'osv', applicability: 'affected' });
    const article = orbit(0.9);
    const lanes = partitionLanes([release, npm, advisory, article]);
    expect(lanes.worth).toEqual([article]);
    expect(lanes.more).toEqual([]);
    for (const r of [release, npm, advisory]) expect(isStackFactRow(r)).toBe(true);
    expect(isStackFactRow(article)).toBe(false);
  });

  it('mirrors the backend registry list plus the advisory sources', () => {
    for (const s of ['npm_registry', 'npm', 'crates_io', 'crates', 'pypi', 'go_modules', 'go', 'osv', 'cve']) {
      expect(STACK_FACT_SOURCES.has(s)).toBe(true);
    }
    expect(STACK_FACT_SOURCES.has('hackernews')).toBe(false);
  });

  it('keeps every non-stack row in exactly one lane', () => {
    const all = [
      ...Array.from({ length: 5 }, () => stackItem()),
      ...Array.from({ length: 14 }, (_, i) => orbit(0.5 + i / 100)),
      ...Array.from({ length: 7 }, (_, i) => ambient(0.9 - i / 100)),
    ];
    const lanes = partitionLanes(all);
    expect(lanes.worth.length + lanes.more.length).toBe(all.length);
    expect(new Set([...lanes.worth, ...lanes.more]).size).toBe(all.length);
  });

  it('Lane 2 holds the first WORTH_LANE_SIZE items, Lane 3 the rest', () => {
    const news = Array.from({ length: WORTH_LANE_SIZE + 5 }, (_, i) => orbit(0.9 - i / 100));
    const lanes = partitionLanes(news);
    expect(lanes.worth).toHaveLength(WORTH_LANE_SIZE);
    expect(lanes.more).toHaveLength(5);
    expect(lanes.worth).toEqual(news.slice(0, WORTH_LANE_SIZE));
  });

  it('is empty when the feed holds only stack facts', () => {
    const lanes = partitionLanes([stackItem({}, { source_type: 'crates_io' }), item({ source_type: 'cve' })]);
    expect(lanes).toEqual({ worth: [], more: [] });
  });
});

describe('partitionLanes — Lane 2 ordering', () => {
  it('puts grounded and In Your Orbit before Ambient, then orders by pipeline score', () => {
    const amb = ambient(0.99);
    const lo = orbit(0.6);
    const hi = orbit(0.9);
    const grounded = stackItem({}, { top_score: 0.7 });
    expect(partitionLanes([amb, lo, hi, grounded]).worth).toEqual([hi, grounded, lo, amb]);
  });

  it('ignores the necessity blend: a necessity-boosted low score does not jump the lane', () => {
    const boosted = orbit(0.65, { necessity_score: 1 });
    const better = orbit(0.9);
    expect(partitionLanes([boosted, better]).worth).toEqual([better, boosted]);
  });

  it('honours the categorical score ceiling', () => {
    const capped = orbit(0.95, { score_ceiling: 0.4 });
    const plain = orbit(0.6);
    expect(laneScore(capped)).toBe(0.4);
    expect(partitionLanes([capped, plain]).worth).toEqual([plain, capped]);
  });

  it('keeps the incoming order on equal scores (stable)', () => {
    const a = orbit(0.9);
    const b = orbit(0.9);
    const c = orbit(0.9);
    expect(partitionLanes([b, c, a]).worth).toEqual([b, c, a]);
  });
});

describe('visibleLaneItems / flattenVisible / locateInLanes', () => {
  const lanes = partitionLanes(Array.from({ length: WORTH_LANE_SIZE + 3 }, (_, i) => orbit(0.9 - i / 100)));

  it('hides Lanes 2 and 3 by default', () => {
    const v = visibleLaneItems(lanes, { worthExpanded: false, moreExpanded: false });
    expect(v.worth).toHaveLength(0);
    expect(v.more).toHaveLength(0);
    expect(flattenVisible(v)).toEqual([]);
  });

  it('shows Lane 2 once it is opened', () => {
    const v = visibleLaneItems(lanes, { worthExpanded: true, moreExpanded: false });
    expect(v.worth).toHaveLength(WORTH_LANE_SIZE);
    expect(flattenVisible(v)).toEqual(v.worth);
  });

  it('shows every row once expanded', () => {
    const v = visibleLaneItems(lanes, { worthExpanded: true, moreExpanded: true });
    expect(v.more).toHaveLength(3);
    expect(flattenVisible(v)).toHaveLength(WORTH_LANE_SIZE + 3);
  });

  it('locates an item in Lane 3', () => {
    expect(locateInLanes(lanes, lanes.more[0]!.id)).toEqual({ lane: 'more', index: 0 });
    expect(locateInLanes(lanes, lanes.worth[2]!.id)).toEqual({ lane: 'worth', index: 2 });
    expect(locateInLanes(lanes, -1)).toBeNull();
  });
});
