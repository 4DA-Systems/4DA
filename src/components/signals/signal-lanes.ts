// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Signal lanes — the relevance-sorted Signal list split into three lanes
// (Decision 3, approved 2026-10-04).
//
//   Lane 1 "Your stack"    the Affects-You evidence pool (isGrounded). Measured
//                          84.6% useful (n=26, two blind labellers). Ordered
//                          security -> breaking -> other, then urgency, then
//                          the incoming display order (score). Capped with an
//                          explicit "Show all N" control — never a silent cut.
//   Lane 2 "Worth knowing" the first WORTH_LANE_SIZE of everything else
//                          (In Your Orbit, then Ambient), by pipeline score.
//   Lane 3 "More"          the remainder, collapsed behind "Show N more".
//
// News lanes measured 12-27% useful but hold 36 of the 60 useful items, so they
// are demoted, not cut. Pool assignment is computeEvidencePool — the same
// predicate Key Signals uses, so the two surfaces cannot disagree on what
// "affects you".
//
// Lane 2 ordering (documented choice, 2026-10-04). The candidates on the
// result were: the display composite (top_score + necessity*0.4), top_score,
// and the judge advisor signal (score_breakdown.advisor_signals, task "judge").
// - The judge advisor is on only 82 of 222 live non-stack items, and is ALREADY
//   folded into top_score by the advisor reconciler (+/-0.15). A mixed key
//   (judge where present, score elsewhere) would be a new ranking model.
// - The necessity blend is a "would regret missing" boost from a keyword
//   classifier; live it put an arXiv vulnerability-dataset paper the judge
//   rated 0.2 at the head of the news lane. Lane 1 already owns urgency.
// So Lane 2 uses top_score (with the reconciled advisor; honouring the v19
// score_ceiling) — the pipeline's quality verdict — inside pool order
// (In Your Orbit before Ambient). Ties keep the incoming composite order.

import type { SourceRelevance } from '../../types';
import { computeEvidencePool, type EvidencePool } from './evidence-pool';

export type LaneKey = 'stack' | 'worth' | 'more';

/** Lane 1 rows visible before "Show all N". */
export const STACK_LANE_CAP = 20;
/** Lane 2 size; everything past it is Lane 3. */
export const WORTH_LANE_SIZE = 10;

export interface SignalLanes {
  stack: SourceRelevance[];
  worth: SourceRelevance[];
  more: SourceRelevance[];
}

/** Lane-1 tier: 0 security, 1 breaking / deprecation, 2 everything else. */
export function stackTier(r: SourceRelevance): 0 | 1 | 2 {
  const sb = r.score_breakdown;
  if (
    r.is_critical_alert === true ||
    r.signal_type === 'security_alert' ||
    r.applicability === 'affected' ||
    r.applicability === 'likely_affected' ||
    sb?.necessity_category === 'security_vulnerability' ||
    sb?.content_type === 'security_advisory'
  ) {
    return 0;
  }
  if (
    r.signal_type === 'breaking_change' ||
    sb?.necessity_category === 'breaking_change' ||
    sb?.necessity_category === 'deprecation_notice'
  ) {
    return 1;
  }
  return 2;
}

const URGENCY_RANK: Record<string, number> = { immediate: 0, this_week: 1, awareness: 2 };

function urgencyRank(r: SourceRelevance): number {
  return URGENCY_RANK[r.score_breakdown?.necessity_urgency ?? ''] ?? 3;
}

/** Pipeline score honouring the categorical ceiling (v19). */
export function laneScore(r: SourceRelevance): number {
  const ceiling = r.score_breakdown?.score_ceiling;
  return ceiling != null ? Math.min(r.top_score, ceiling) : r.top_score;
}

const NEWS_POOL_RANK: Record<EvidencePool, number> = { affects_you: 0, in_orbit: 0, ambient: 1 };

/**
 * Split an already score-sorted list into lanes. Pure; stable for equal keys
 * (Array.prototype.sort is stable), so the incoming order breaks every tie.
 */
export function partitionLanes(results: SourceRelevance[]): SignalLanes {
  const stack: { r: SourceRelevance; i: number }[] = [];
  const rest: { r: SourceRelevance; i: number; pool: EvidencePool }[] = [];
  results.forEach((r, i) => {
    const pool = computeEvidencePool(r);
    if (pool === 'affects_you') stack.push({ r, i });
    else rest.push({ r, i, pool });
  });
  stack.sort((a, b) =>
    (stackTier(a.r) - stackTier(b.r)) || (urgencyRank(a.r) - urgencyRank(b.r)) || (a.i - b.i));
  rest.sort((a, b) =>
    (NEWS_POOL_RANK[a.pool] - NEWS_POOL_RANK[b.pool]) || (laneScore(b.r) - laneScore(a.r)) || (a.i - b.i));
  const ordered = rest.map((x) => x.r);
  return {
    stack: stack.map((x) => x.r),
    worth: ordered.slice(0, WORTH_LANE_SIZE),
    more: ordered.slice(WORTH_LANE_SIZE),
  };
}

export interface LaneExpansion {
  stackExpanded: boolean;
  moreExpanded: boolean;
}

/** The rows a lane shows given the expansion state. */
export function visibleLaneItems(lanes: SignalLanes, exp: LaneExpansion): SignalLanes {
  return {
    stack: exp.stackExpanded ? lanes.stack : lanes.stack.slice(0, STACK_LANE_CAP),
    worth: lanes.worth,
    more: exp.moreExpanded ? lanes.more : [],
  };
}

/** Flattened visible order — what keyboard navigation (j/k, s, d, o) walks. */
export function flattenVisible(v: SignalLanes): SourceRelevance[] {
  return [...v.stack, ...v.worth, ...v.more];
}

/** Which lane holds an item, and whether it is past Lane 1's cap. */
export function locateInLanes(lanes: SignalLanes, id: number): { lane: LaneKey; index: number } | null {
  for (const lane of ['stack', 'worth', 'more'] as const) {
    const index = lanes[lane].findIndex((r) => r.id === id);
    if (index >= 0) return { lane, index };
  }
  return null;
}
