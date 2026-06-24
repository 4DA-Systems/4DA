// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Evidence pools — group surfaced signals by HOW they are grounded to the
// user's world, not by their raw relevance score.
//
// Why grounding and not score: measured on the live corpus, ~77% of items
// scoring >= 0.85 have no tie to the user's actual stack, and a genuinely
// stack-relevant item (e.g. a Tauri-v2 fix) scores the same ~0.91 as pure
// noise (e.g. an unrelated conference post). A score threshold therefore
// cannot separate signal from noise — they sit at the same height. The axis
// that DOES separate them is grounding: a real dependency/CVE edge to the
// user's installed packages, which a content author cannot fabricate by
// stuffing keywords or hashtags. Pool assignment anchors on that edge.

import type { SourceRelevance } from '../../types';

export type EvidencePool = 'affects_you' | 'in_orbit' | 'ambient';

/**
 * Domain-relevance cutoff for the "In Your Orbit" pool. Per
 * ScoreBreakdown.domain_relevance the scale runs 0.15 (off-domain) →
 * 0.70 (adjacent tech) → 0.85 (a declared dependency) → 1.0 (primary stack).
 * >= 0.70 means the item's extracted topics matched the user's declared or
 * adjacent stack even without a concrete dependency edge.
 */
export const ORBIT_DOMAIN_THRESHOLD = 0.7;

/**
 * True when the item has a verifiable edge to the user's own machine state —
 * a matched dependency, or a security advisory the backend confirmed affects
 * an installed version. This is the trust boundary for the highlighted pool.
 */
export function isGrounded(r: SourceRelevance): boolean {
  return (
    // Independent advisory routes — a backend-confirmed CVE edge grounds the
    // item regardless of dep-name matching (kept so a real advisory whose title
    // doesn't name the package still surfaces).
    r.is_critical_alert === true ||
    r.applicability === 'affected' ||
    r.applicability === 'likely_affected' ||
    // Canonical dependency grounding: the backend's single verdict (strong,
    // non-dev, non-ambiguous edge). NOT matched_deps.length — a bare word-like
    // subterm hit (e.g. "windows" from windows-sys on a "Windows 0-day" OS
    // headline) populates matched_deps but is not real grounding.
    r.score_breakdown?.strongly_grounded === true
  );
}

/** Package names from the user's dependency graph that matched this item. */
export function groundingDeps(r: SourceRelevance): string[] {
  return r.score_breakdown?.matched_deps ?? [];
}

/**
 * Assign an item to an evidence pool.
 *   affects_you — grounded in the user's dependencies / a confirmed advisory
 *   in_orbit    — no dependency edge, but topically inside the declared stack
 *   ambient     — topically similar only; lowest confidence
 */
export function computeEvidencePool(r: SourceRelevance): EvidencePool {
  if (isGrounded(r)) return 'affects_you';
  const domain = r.score_breakdown?.domain_relevance ?? 0;
  if (domain >= ORBIT_DOMAIN_THRESHOLD) return 'in_orbit';
  return 'ambient';
}
