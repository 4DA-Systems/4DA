// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import {
  STACK_LANE_CAP, WORTH_LANE_SIZE, flattenVisible, laneScore, locateInLanes,
  partitionLanes, stackTier, visibleLaneItems,
} from './signal-lanes';
import { isGrounded } from './evidence-pool';
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
    ...(sb ? { score_breakdown: sb as never } : {}),
    ...rest,
  };
}
const stackItem = (sb: Record<string, unknown> = {}, extra: Partial<SourceRelevance> = {}) =>
  item({ sb: { strongly_grounded: true, dependency_event: true, ...sb }, ...extra });
const orbit = (top_score: number, sb: Record<string, unknown> = {}) =>
  item({ top_score, sb: { domain_relevance: 0.85, ...sb } });
const ambient = (top_score: number) => item({ top_score, sb: { domain_relevance: 0.2 } });

describe('partitionLanes — lane assignment', () => {
  it('puts exactly the isGrounded (Affects You) items in Lane 1 — the Key Signals predicate', () => {
    const grounded = stackItem();
    const advisory = item({ applicability: 'affected', sb: { domain_relevance: 0.2 } });
    const tutorial = item({ sb: { strongly_grounded: true, dependency_event: false, domain_relevance: 0.85 } });
    const notAffected = item({ applicability: 'not_affected', is_critical_alert: true, sb: { domain_relevance: 0.2 } });
    const all = [grounded, advisory, tutorial, notAffected, orbit(0.9), ambient(0.9)];
    const lanes = partitionLanes(all);
    expect(lanes.stack.map((r) => r.id).sort()).toEqual([grounded.id, advisory.id].sort());
    for (const r of all) expect(lanes.stack.includes(r)).toBe(isGrounded(r));
    expect(lanes.worth).toContain(tutorial);
    expect(lanes.worth).toContain(notAffected);
  });

  it('drops nothing: every input lands in exactly one lane', () => {
    const all = [
      ...Array.from({ length: 25 }, () => stackItem()),
      ...Array.from({ length: 14 }, (_, i) => orbit(0.5 + i / 100)),
      ...Array.from({ length: 7 }, (_, i) => ambient(0.9 - i / 100)),
    ];
    const lanes = partitionLanes(all);
    expect(lanes.stack.length + lanes.worth.length + lanes.more.length).toBe(all.length);
    expect(new Set([...lanes.stack, ...lanes.worth, ...lanes.more]).size).toBe(all.length);
  });

  it('Lane 2 holds the first WORTH_LANE_SIZE non-stack items, Lane 3 the rest', () => {
    const news = Array.from({ length: WORTH_LANE_SIZE + 5 }, (_, i) => orbit(0.9 - i / 100));
    const lanes = partitionLanes(news);
    expect(lanes.worth).toHaveLength(WORTH_LANE_SIZE);
    expect(lanes.more).toHaveLength(5);
    expect(lanes.worth).toEqual(news.slice(0, WORTH_LANE_SIZE));
  });

  it('leaves Lane 3 empty when there are few news items', () => {
    const lanes = partitionLanes([stackItem(), orbit(0.7)]);
    expect(lanes.worth).toHaveLength(1);
    expect(lanes.more).toHaveLength(0);
  });
});

describe('partitionLanes — Lane 1 ordering', () => {
  it('orders security, then breaking, then other — regardless of incoming score order', () => {
    const minor = stackItem({ necessity_category: 'ecosystem_shift' });
    const breaking = stackItem({ necessity_category: 'breaking_change' });
    const security = stackItem({}, { signal_type: 'security_alert' });
    const lanes = partitionLanes([minor, breaking, security]);
    expect(lanes.stack).toEqual([security, breaking, minor]);
  });

  it('orders by urgency inside a tier, then keeps the incoming (score) order', () => {
    const aware = stackItem({ necessity_category: 'breaking_change', necessity_urgency: 'awareness' });
    const weekA = stackItem({ necessity_category: 'breaking_change', necessity_urgency: 'this_week' });
    const weekB = stackItem({ necessity_category: 'breaking_change', necessity_urgency: 'this_week' });
    const now = stackItem({ necessity_category: 'breaking_change', necessity_urgency: 'immediate' });
    const lanes = partitionLanes([aware, weekA, weekB, now]);
    expect(lanes.stack).toEqual([now, weekA, weekB, aware]);
  });

  it('classifies tiers from every security / breaking signal on the result', () => {
    expect(stackTier(item({ is_critical_alert: true }))).toBe(0);
    expect(stackTier(item({ applicability: 'likely_affected' }))).toBe(0);
    expect(stackTier(item({ sb: { necessity_category: 'security_vulnerability' } }))).toBe(0);
    expect(stackTier(item({ sb: { content_type: 'security_advisory' } }))).toBe(0);
    expect(stackTier(item({ signal_type: 'breaking_change' }))).toBe(1);
    expect(stackTier(item({ sb: { necessity_category: 'deprecation_notice' } }))).toBe(1);
    expect(stackTier(item({ sb: { necessity_category: 'ecosystem_shift' } }))).toBe(2);
    expect(stackTier(item())).toBe(2);
  });
});

describe('partitionLanes — Lane 2 ordering', () => {
  it('puts In Your Orbit before Ambient, then orders by pipeline score', () => {
    const amb = ambient(0.99);
    const lo = orbit(0.6);
    const hi = orbit(0.9);
    expect(partitionLanes([amb, lo, hi]).worth).toEqual([hi, lo, amb]);
  });

  it('ignores the necessity blend: a necessity-boosted low score does not jump the lane', () => {
    // Composite order (incoming) put `boosted` first via necessity*0.4.
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
  const lanes = partitionLanes([
    ...Array.from({ length: STACK_LANE_CAP + 4 }, () => stackItem()),
    ...Array.from({ length: WORTH_LANE_SIZE + 3 }, (_, i) => orbit(0.9 - i / 100)),
  ]);

  it('caps Lane 1 and hides Lanes 2 and 3 by default', () => {
    const v = visibleLaneItems(lanes, { stackExpanded: false, worthExpanded: false, moreExpanded: false });
    expect(v.stack).toHaveLength(STACK_LANE_CAP);
    expect(v.worth).toHaveLength(0);
    expect(v.more).toHaveLength(0);
    expect(flattenVisible(v)).toEqual(v.stack);
  });

  it('shows Lane 2 once it is opened', () => {
    const v = visibleLaneItems(lanes, { stackExpanded: false, worthExpanded: true, moreExpanded: false });
    expect(v.worth).toHaveLength(WORTH_LANE_SIZE);
    expect(flattenVisible(v)).toEqual([...v.stack, ...v.worth]);
  });

  it('shows every row once expanded', () => {
    const v = visibleLaneItems(lanes, { stackExpanded: true, worthExpanded: true, moreExpanded: true });
    expect(v.stack).toHaveLength(STACK_LANE_CAP + 4);
    expect(v.more).toHaveLength(3);
    expect(flattenVisible(v)).toHaveLength(STACK_LANE_CAP + 4 + WORTH_LANE_SIZE + 3);
  });

  it('locates an item past the Lane 1 cap and in Lane 3', () => {
    const past = lanes.stack[STACK_LANE_CAP + 1]!;
    expect(locateInLanes(lanes, past.id)).toEqual({ lane: 'stack', index: STACK_LANE_CAP + 1 });
    expect(locateInLanes(lanes, lanes.more[0]!.id)).toEqual({ lane: 'more', index: 0 });
    expect(locateInLanes(lanes, -1)).toBeNull();
  });
});
