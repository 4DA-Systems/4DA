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
const ORBIT_DOMAIN_THRESHOLD = 0.7;

/** Registry advisory sources: one row per published advisory. */
const ADVISORY_SOURCES = new Set(['cve', 'osv']);

/** The advisory affects only projects outside the user's active stack. */
export function isAffectedInactive(r: SourceRelevance): boolean {
  return r.applicability === 'affected_inactive';
}

/**
 * True when the item has a verifiable edge to the user's own machine state —
 * a matched dependency, or a security advisory the backend confirmed affects
 * an installed version. This is the trust boundary for the highlighted pool.
 */
export function isGrounded(r: SourceRelevance): boolean {
  // CONFIRMED not-affected (installed version outside the affected range /
  // at-or-past the fix, verified by the backend version check): the advisory
  // is ABOUT the user's dependency but does not endanger their build — it
  // must never occupy the highest-trust pool, even though the dep-name match
  // is real and strong.
  if (r.applicability === 'not_affected') return false;
  // Affects ONLY inactive projects (dormant >90 days or a scratch tree its
  // repository gitignores — audit 2026-10-07, AD-043 amended): the advisory is
  // true and stays named on the item and under Preemption's "Dormant
  // projects", but it is not about code the user is working on. Explicit even
  // though the backend also clears the grounding flags, so a stale flag can
  // never put it back in the highest-trust pool.
  if (isAffectedInactive(r)) return false;
  return (
    // Independent advisory routes — a backend-confirmed CVE edge grounds the
    // item regardless of dep-name matching (kept so a real advisory whose title
    // doesn't name the package still surfaces).
    r.is_critical_alert === true ||
    r.applicability === 'affected' ||
    // `likely_affected` is a version-path signal only on a REGISTRY advisory
    // row (cve / osv). On an editorial security story it comes from a weak,
    // uncorroborated name match — live 2026-10-04, "Google Rewrites Critical C
    // Dependencies to Rust…" (HN, no matched dependency) sat in Affects You.
    // Editorial stories earn the pool through grounding + a dependency event.
    (r.applicability === 'likely_affected' && ADVISORY_SOURCES.has(r.source_type ?? '')) ||
    // Canonical dependency grounding: the backend's single verdict (strong,
    // non-dev, non-ambiguous edge). NOT matched_deps.length — a bare word-like
    // subterm hit (e.g. "windows" from windows-sys on a "Windows 0-day" OS
    // headline) populates matched_deps but is not real grounding.
    // ...AND a dependency EVENT (2026-10-04): grounding proves the item names
    // the package; "Affects You" claims something is happening TO it — a
    // release, a breaking change, a vulnerability. A tutorial that merely
    // uses the dependency ("Progressive Hydration in React") is grounded but
    // not an event (`dependency_event === false`) and belongs in Orbit. An
    // absent flag (a breakdown from a backend that predates the claim) keeps
    // the old grounding-only rule.
    (r.score_breakdown?.strongly_grounded === true && r.score_breakdown.dependency_event !== false)
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
