// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Briefing synthesis text helpers.
 *
 * The Rust-side synthesizer emits one of two shapes:
 *
 *   1. A three-section briefing — "SITUATION\n...\n\nPRIORITY\n...\n\nPATTERN\n..."
 *   2. An abstention line — "Low signal — no noteworthy intelligence overnight."
 *      (optionally followed by a short telemetry tail after a blank line)
 *
 * The UI renders them differently: full briefings get a prominent
 * synthesis section plus the source-items list; abstentions render as
 * a minimal muted message with NO source-items list (because the brief
 * is deliberately saying "nothing worth saying today").
 *
 * Detecting the abstention shape is a string check because neither the
 * Rust side nor the LLM exposes a structured type for it — we gate on
 * the exact literal the Rust prompt uses. If the literal ever changes
 * in `monitoring_briefing.rs`, the corresponding string here must be
 * updated in lockstep. The guard test in
 * `briefing-synthesis-helpers.test.ts` documents the contract.
 */

/// A canonical, human-facing abstention marker (em-dash form). Detection does NOT rely
/// on this exact string — `isAbstentionSynthesis` tolerates both Rust shapes and any
/// dash normalization — it remains for display/snapshot use and as a representative example.
export const ABSTENTION_MARKER = 'Low signal — no noteworthy intelligence overnight.';

/**
 * Is this synthesis text an abstention response?
 *
 * Accepts both the exact marker and the marker-with-telemetry-tail
 * variant (e.g., "Low signal — ... \n\n(25 items scanned, synthesis
 * skipped: 4 ungrounded terms detected)"). Tolerates Unicode dash
 * variants because some LLMs normalize em-dash to hyphen or vice versa.
 */
export function isAbstentionSynthesis(synthesis: string | null | undefined): boolean {
  if (synthesis == null) return false;
  const trimmed = synthesis.trim();
  if (trimmed.length === 0) return false;
  // Normalize all dash-like characters to a plain hyphen for the check
  // so "—" "–" "-" all match equivalently.
  // Collapse every dash-like char \u2014 Unicode dashes AND the ASCII hyphen (incl. the
  // prompt's literal "--") \u2014 to a single hyphen, then accept both Rust shapes:
  //   "Low signal -- no noteworthy intelligence overnight." (monitoring_briefing.rs:2176/2323/2621)
  //   "Low signal \u2014 no new intelligence overnight."         (monitoring_briefing.rs:731)
  const normalizedFirstLine = trimmed
    .split('\n')[0]!
    .replace(/[\u2010-\u2015\u2212-]+/g, '-')
    .replace(/\s+/g, ' ')
    .toLowerCase();
  return /^low signal\s*-\s*no (noteworthy|new) intelligence/.test(normalizedFirstLine);
}

/**
 * Extract the user-facing abstention headline (first line only) and
 * the optional telemetry tail (everything after the first blank line)
 * separately so the UI can style them differently.
 */
export function parseAbstention(synthesis: string): { headline: string; telemetry: string | null } {
  const trimmed = synthesis.trim();
  const parts = trimmed.split(/\n\s*\n/, 2);
  const headline = parts[0]?.trim() ?? trimmed;
  const telemetry = parts[1]?.trim() ?? null;
  return { headline, telemetry: telemetry != null && telemetry.length > 0 ? telemetry : null };
}

/**
 * Strip the trailing telemetry suffix from a synthesis so a downstream
 * renderer that only wants the prose can get it without the bracket
 * metadata. For non-abstention briefings this is a no-op.
 */
export function stripSynthesisTelemetry(synthesis: string): string {
  if (!isAbstentionSynthesis(synthesis)) return synthesis;
  return parseAbstention(synthesis).headline;
}
