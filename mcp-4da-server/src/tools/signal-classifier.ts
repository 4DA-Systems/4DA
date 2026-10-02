// SPDX-License-Identifier: Apache-2.0
/**
 * Signal vocabulary for get_actionable_signals: the types, and the mapping
 * from the desktop pipeline's stored priority tiers onto this tool's.
 *
 * This file used to hold a keyword classifier that labelled unstamped feed
 * items. It was removed on 2026-10-02: two keywords such as "cve" and "patch"
 * made an item a security alert, and any language name in the stack
 * ("rust", "javascript") counted as a stack match, so russh and Angular CVEs
 * no project used came out "high — affects your rust stack" and drove the
 * briefing to human_only. The server no longer invents classifications;
 * it reports the pipeline's, gated by the relevance judge.
 */

export type SignalType =
  | "security_alert"
  | "breaking_change"
  | "tool_discovery"
  | "tech_trend"
  | "learning"
  | "competitive_intel";

export type SignalPriority = "critical" | "high" | "medium" | "low";

/**
 * The desktop pipeline stores its own notification tiers in
 * `source_items.signal_priority` — critical, alert, advisory, watch — not this
 * tool's critical/high/medium/low. The reader used to cast the stored string
 * straight through, so 92 of the 93 stamped rows in the live corpus carried a
 * priority no filter, sort, or briefing rule recognised (they sorted last and
 * an "alert"-tier security alert could never reach the human_only rule).
 * Mapped by tier order; an unrecognised value lands in the middle rather than
 * being silently dropped or silently promoted.
 */
export function normalizeStoredPriority(stored: string): SignalPriority {
  switch (stored.trim().toLowerCase()) {
    case "critical":
      return "critical";
    case "alert":
    case "high":
      return "high";
    case "advisory":
    case "medium":
      return "medium";
    case "watch":
    case "low":
      return "low";
    default:
      return "medium";
  }
}
