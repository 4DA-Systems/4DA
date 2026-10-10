// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Signal lanes. Lane 1 "Your stack" is NOT built here any more: AD-054 makes
// it the deterministic stack-change stream, served by the backend
// `get_stack_changes` command (see ./stack-change.ts and
// src-tauri/src/evidence/stack_change.rs). It used to be a client-side slice
// of this relevance-sorted feed — the Affects-You evidence pool, ordered by a
// keyword tier — so what it showed depended on what the scorer happened to
// keep. This file now splits only the READING feed:
//
//   "Worth knowing"  the first WORTH_LANE_SIZE feed items, in pool order
//                    (Affects You and In Your Orbit, then Ambient), by
//                    pipeline score, COLLAPSED by default (Decision 6,
//                    2026-10-05: it measured 0/10 useful with two blind
//                    labellers, and no stored ranking rescues it).
//   "More"           the remainder, collapsed behind "Show N more".
//
// Registry release rows and advisories are left out of both: Lane 1 states
// those facts, graded against what is installed, once per package. Showing
// the raw rows again below it would say the same thing twice, ungraded.
//
// Lane 2 ordering (documented choice, 2026-10-04): top_score (with the
// reconciled advisor; honouring the v19 score_ceiling) inside pool order. The
// necessity blend put an arXiv dataset paper the judge rated 0.2 at the head
// of this lane; the judge advisor is on too few rows to rank by alone. Ties
// keep the incoming composite order.

import type { SourceRelevance } from '../../types';
import { computeEvidencePool, type EvidencePool } from './evidence-pool';

export type LaneKey = 'worth' | 'more';

/** Lane 2 size; everything past it is Lane 3. */
export const WORTH_LANE_SIZE = 10;

/**
 * Feed sources whose facts Lane 1 owns: package registries (mirrors
 * `dep_linker::REGISTRY_SOURCE_TYPES`) and advisory databases.
 */
export const STACK_FACT_SOURCES: ReadonlySet<string> = new Set([
  'npm_registry', 'npm', 'crates_io', 'crates', 'pypi', 'go_modules', 'go',
  'maven', 'nuget', 'packagist', 'rubygems', 'cocoapods', 'osv', 'cve',
]);

export interface SignalLanes {
  worth: SourceRelevance[];
  more: SourceRelevance[];
}

/** Tier of a feed row: 0 security, 1 breaking / deprecation, 2 everything else. */
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

/**
 * Feed rows ordered security -> breaking -> other, then urgency, then the
 * incoming order. Used by "What you would have missed" to pick its hero.
 */
export function orderByStackTier(results: SourceRelevance[]): SourceRelevance[] {
  return results
    .map((r, i) => ({ r, i }))
    .sort((a, b) =>
      (stackTier(a.r) - stackTier(b.r)) || (urgencyRank(a.r) - urgencyRank(b.r)) || (a.i - b.i))
    .map((x) => x.r);
}

/** Pipeline score honouring the categorical ceiling (v19). */
export function laneScore(r: SourceRelevance): number {
  const ceiling = r.score_breakdown?.score_ceiling;
  return ceiling != null ? Math.min(r.top_score, ceiling) : r.top_score;
}

const NEWS_POOL_RANK: Record<EvidencePool, number> = { affects_you: 0, in_orbit: 0, ambient: 1 };

/** A row whose fact Lane 1 states (a registry release or an advisory). */
export function isStackFactRow(r: SourceRelevance): boolean {
  return STACK_FACT_SOURCES.has(r.source_type ?? '');
}

/**
 * Split an already score-sorted feed into the reading lanes. Pure; stable for
 * equal keys (Array.prototype.sort is stable), so the incoming order breaks
 * every tie.
 */
export function partitionLanes(results: SourceRelevance[]): SignalLanes {
  const ordered = results
    .map((r, i) => ({ r, i }))
    .filter(({ r }) => !isStackFactRow(r))
    .map((x) => ({ ...x, pool: computeEvidencePool(x.r) }))
    .sort((a, b) =>
      (NEWS_POOL_RANK[a.pool] - NEWS_POOL_RANK[b.pool]) || (laneScore(b.r) - laneScore(a.r)) || (a.i - b.i))
    .map((x) => x.r);
  return {
    worth: ordered.slice(0, WORTH_LANE_SIZE),
    more: ordered.slice(WORTH_LANE_SIZE),
  };
}

export interface LaneExpansion {
  worthExpanded: boolean;
  moreExpanded: boolean;
}

/** The rows a lane shows given the expansion state. */
export function visibleLaneItems(lanes: SignalLanes, exp: LaneExpansion): SignalLanes {
  return {
    worth: exp.worthExpanded ? lanes.worth : [],
    more: exp.moreExpanded ? lanes.more : [],
  };
}

/** Flattened visible order — what keyboard navigation (j/k, s, d, o) walks. */
export function flattenVisible(v: SignalLanes): SourceRelevance[] {
  return [...v.worth, ...v.more];
}

/** Which lane holds an item. */
export function locateInLanes(lanes: SignalLanes, id: number): { lane: LaneKey; index: number } | null {
  for (const lane of ['worth', 'more'] as const) {
    const index = lanes[lane].findIndex((r) => r.id === id);
    if (index >= 0) return { lane, index };
  }
  return null;
}
