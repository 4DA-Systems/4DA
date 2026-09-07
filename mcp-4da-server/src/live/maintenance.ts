// SPDX-License-Identifier: Apache-2.0
/**
 * Maintenance-notice predicate, shared by every consumer of a vulnerability
 * scan (vulnerability_scan, get_actionable_signals, what_should_i_know,
 * dependency_health) so they all draw the same line.
 */

import type { VulnerabilityEntry } from "./types.js";

/**
 * Is this advisory a maintenance notice rather than a vulnerability?
 *
 * RustSec publishes "X is unmaintained" as an advisory so tooling can see it,
 * and OSV carries it through with no severity and no fixed version. It is real
 * information — an unmaintained dependency is a risk to plan around — but it is
 * not something an attacker can exploit and not something you can upgrade away
 * today, so it does not belong in the same list, the same count, or the same
 * "Review X" recommendation as a live CVE.
 *
 * Live scan: 23 of 41 findings were these (the whole gtk-rs GTK3 family, all
 * five `unic-*` crates, `paste`, `proc-macro-error`, `ttf-parser`), producing
 * 27 "Review" lines around six genuinely actionable upgrades.
 *
 * Matched on the advisory's own wording, which RustSec keeps consistent, and
 * deliberately NOT on "deprecated" — that word appears in plenty of real
 * vulnerability summaries.
 */
export function isMaintenanceNotice(v: Pick<VulnerabilityEntry, "summary">): boolean {
  return /\b(unmaintained|no longer maintained)\b/i.test(v.summary || "");
}

/**
 * The advisories a scan can actually ask the user to act on today: built on
 * this host AND exploitable (not a maintenance notice). This is the set
 * `vulnerability_scan` reports as `vulnerabilities` / `total_vulnerable_packages`;
 * every other tool that counts "vulnerable" must count the same set or the
 * numbers disagree across tools over one scan.
 */
export function isActionableVulnerability(
  v: Pick<VulnerabilityEntry, "summary" | "platformActive">,
): boolean {
  return v.platformActive !== false && !isMaintenanceNotice(v);
}
