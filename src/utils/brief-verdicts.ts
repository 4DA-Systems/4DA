// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// AD-035 — one item, one verdict: pure helpers for binding the LATEST
// briefing's structured filter verdicts to the promoted display surfaces
// (Brief attention cards, Key Signals pools, the "missed" hero). Demote-only:
// a filtered item loses promoted placement but stays in the ordinary feed; a
// kept item is NEVER promoted by a verdict; deterministic security truth
// (`is_critical_alert`, the OSV-version-checked class) is never suppressed
// by narration.
//
// The exemption here is DELIBERATELY narrower than the one on the feed-demotion
// path (`brief_rejections::apply_brief_rejection_demotions`), which also exempts
// `strongly_grounded`. AD-035 considered that exemption for display binding and
// rejected it: the audited items sat inside the grounded pool, so it would have
// nullified the fix for the observed defect. Feed ORDER and PROMOTED placement
// are different stakes. Do not align the two predicates without a new ADR.

import type { BriefVerdicts } from '../store/types';
import type { SourceRelevance } from '../types';

const EMPTY_IDS: ReadonlySet<number> = new Set<number>();

/**
 * The item ids the latest briefing filtered, IF its verdicts are still
 * inside the freshness window. Expired or absent verdicts yield the empty
 * set — a stale briefing binds nothing.
 */
export function activeBriefFilteredIds(
  verdicts: BriefVerdicts | null | undefined,
  nowMs: number = Date.now(),
): ReadonlySet<number> {
  if (!verdicts || nowMs >= verdicts.expiresAtMs) return EMPTY_IDS;
  const ids = Object.keys(verdicts.filtered).map(Number).filter(Number.isFinite);
  if (ids.length === 0) return EMPTY_IDS;
  return new Set(ids);
}

/**
 * True when the briefing's verdict demotes this item from PROMOTED
 * placement. Deterministic exemption: an OSV-version-checked critical
 * alert (`is_critical_alert`) is never suppressed — a narration verdict
 * does not outrank confirmed security truth.
 */
export function isBriefSuppressed(
  item: Pick<SourceRelevance, 'id' | 'is_critical_alert'>,
  filteredIds: ReadonlySet<number>,
): boolean {
  return filteredIds.size > 0 && filteredIds.has(item.id) && item.is_critical_alert !== true;
}
