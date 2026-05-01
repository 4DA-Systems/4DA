// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * what_should_i_know tool
 *
 * Pre-task intelligence briefing for AI coding agents. Synthesizes:
 * - Live vulnerability data + actionable signals
 * - Decision windows (time-bounded opportunities)
 * - Ecosystem news (HN headlines relevant to tech stack)
 *
 * Filters everything for relevance to the described task and involved files.
 */

import type { FourDADatabase } from "../db.js";
import { executeGetActionableSignals } from "./get-actionable-signals.js";
import { getLiveIntelligence } from "../live-singleton.js";

// ============================================================================
// Types
// ============================================================================

export interface WhatShouldIKnowParams {
  task: string;
  files?: string[];
}

interface Advisory {
  title: string;
  signal_type: string;
  priority: string;
  action: string;
  url: string | null;
}

interface DecisionWindow {
  id: number;
  title: string;
  description: string | null;
  urgency: number;
}

interface WisdomEntry {
  type: string;
  subject: string;
  detail: string;
}

interface EcosystemNewsItem {
  title: string;
  url: string | null;
  points: number;
  relevance_reason: string;
}

type DelegationLevel = "safe_to_delegate" | "review_needed" | "human_only";

interface WhatShouldIKnowResult {
  task: string;
  files: string[];
  advisories: Advisory[];
  decision_windows: DecisionWindow[];
  relevant_wisdom: WisdomEntry[];
  ecosystem_news: EcosystemNewsItem[];
  delegation_assessment: {
    level: DelegationLevel;
    reason: string;
  };
  summary: string;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const whatShouldIKnowTool = {
  name: "what_should_i_know",
  description:
    "Pre-task intelligence briefing. Given a task description and optional file paths, returns filtered advisories, decision windows, signal chains, relevant wisdom, and a delegation assessment. Call before starting any non-trivial task.",
  inputSchema: {
    type: "object" as const,
    properties: {
      task: {
        type: "string",
        description:
          "Description of what you are about to work on",
      },
      files: {
        type: "array",
        items: { type: "string" },
        description:
          "File paths involved in the task (optional). Improves relevance filtering.",
      },
    },
    required: ["task"],
  },
};

// ============================================================================
// Relevance Filtering
// ============================================================================

/**
 * Build a set of lowercase keywords from the task description and file paths.
 * Used for simple keyword-based relevance matching against intelligence results.
 */
function buildKeywords(task: string, files: string[]): Set<string> {
  const words = new Set<string>();
  if (!task) return words;

  // Extract meaningful words from task (3+ chars, lowercased)
  for (const word of task.toLowerCase().split(/\s+/)) {
    const cleaned = word.replace(/[^a-z0-9_-]/g, "");
    if (cleaned.length >= 3) {
      words.add(cleaned);
    }
  }

  // Extract keywords from file paths
  for (const filePath of files) {
    // Get filename and directory segments
    const segments = filePath.replace(/\\/g, "/").split("/");
    for (const segment of segments) {
      // Split on dots, dashes, underscores for granular matching
      for (const part of segment.split(/[.\-_]/)) {
        const cleaned = part.toLowerCase().replace(/[^a-z0-9]/g, "");
        if (cleaned.length >= 3) {
          words.add(cleaned);
        }
      }
    }
  }

  return words;
}

/**
 * Check if a text matches any keywords from the task context.
 */
function matchesKeywords(text: string, keywords: Set<string>): boolean {
  if (!text) return false;
  const textLower = text.toLowerCase();
  for (const keyword of keywords) {
    if (textLower.includes(keyword)) {
      return true;
    }
  }
  return false;
}

// ============================================================================
// Decision Window Retrieval
// ============================================================================

interface WindowRow {
  id: number;
  title: string;
  description: string | null;
  urgency: number;
}

function getOpenDecisionWindows(db: FourDADatabase): WindowRow[] {
  try {
    const rawDb = db.getRawDb();
    return rawDb.prepare(
      `SELECT id, title, description, urgency
       FROM decision_windows WHERE status = 'open'
       ORDER BY urgency DESC, created_at DESC
       LIMIT 20`,
    ).all() as WindowRow[];
  } catch {
    // Table may not exist yet
    return [];
  }
}

// ============================================================================
// Execute
// ============================================================================

export function executeWhatShouldIKnow(
  db: FourDADatabase,
  params: WhatShouldIKnowParams,
): WhatShouldIKnowResult {
  const task = params.task;
  const files = params.files || [];
  const keywords = buildKeywords(task, files);

  // ── 1. Actionable Signals (security, breaking changes, etc.) ──────────
  let advisories: Advisory[] = [];
  try {
    const signalResult = executeGetActionableSignals(db, {
      limit: 50,
      since_hours: 72,
    });

    advisories = signalResult.signals
      .filter((s) => {
        // Include all critical/high security signals unconditionally
        if (s.signal_type === "security_alert" && (s.signal_priority === "critical" || s.signal_priority === "high")) {
          return true;
        }
        // Otherwise, filter by keyword relevance
        return matchesKeywords((s.title || "") + " " + (s.action || ""), keywords);
      })
      .slice(0, 10)
      .map((s) => ({
        title: s.title,
        signal_type: s.signal_type,
        priority: s.signal_priority,
        action: s.action,
        url: s.url,
      }));
  } catch {
    // Signals unavailable — non-fatal
  }

  // ── 1b. Live vulnerability data ──────────────────────────────────────
  try {
    const liveIntel = getLiveIntelligence();
    if (liveIntel) {
      const vulnResult = liveIntel.getVulnerabilities();
      if (vulnResult && vulnResult.totalVulnerable > 0) {
        const topVulns = vulnResult.vulnerabilities.slice(0, 3);
        const details = topVulns.map((v) =>
          `${v.package}@${v.currentVersion}: ${v.summary}`
        ).join("; ");

        advisories.unshift({
          title: `${vulnResult.totalVulnerable} dependenc${vulnResult.totalVulnerable !== 1 ? "ies have" : "y has"} known vulnerabilities`,
          signal_type: "security_alert",
          priority: vulnResult.bySeverity.critical > 0 ? "critical" :
                    vulnResult.bySeverity.high > 0 ? "high" : "medium",
          action: `Run vulnerability_scan for full details. ${details}`,
          url: null,
        });
      }
    }
  } catch {
    // Live intel unavailable — non-fatal
  }

  // ── 2. Decision Windows ───────────────────────────────────────────────
  let decisionWindows: DecisionWindow[] = [];
  try {
    const windows = getOpenDecisionWindows(db);
    decisionWindows = windows
      .filter((w) => matchesKeywords((w.title || "") + " " + (w.description || ""), keywords))
      .slice(0, 5)
      .map((w) => ({
        id: w.id,
        title: w.title,
        description: w.description,
        urgency: w.urgency,
      }));
  } catch {
    // Windows unavailable — non-fatal
  }

  // ── 3. Relevant Wisdom (decisions) ─────────────────────────────────────
  const relevantWisdom: WisdomEntry[] = [];

  // ── 4. Ecosystem News (HN headlines relevant to tech stack) ───────────
  let ecosystemNews: EcosystemNewsItem[] = [];
  try {
    const hnIntel = getLiveIntelligence();
    if (hnIntel) {
      const headlines = hnIntel.getHeadlines();
      ecosystemNews = headlines
        .filter((h) => h.relevanceScore > 0.3 || matchesKeywords(h.title, keywords))
        .slice(0, 5)
        .map((h) => ({
          title: h.title,
          url: h.url,
          points: h.points,
          relevance_reason: h.relevanceReason,
        }));
    }
  } catch {
    // Headlines unavailable — non-fatal
  }

  // ── 5. Delegation Assessment ──────────────────────────────────────────
  const signalDensity = advisories.length + decisionWindows.length;

  const hasSecuritySignals = advisories.some(
    (a) => a.signal_type === "security_alert" && (a.priority === "critical" || a.priority === "high"),
  );
  const hasHighUrgencyWindows = decisionWindows.some((w) => w.urgency >= 4);

  let delegationLevel: DelegationLevel;
  let delegationReason: string;

  if (hasSecuritySignals || hasHighUrgencyWindows) {
    delegationLevel = "human_only";
    delegationReason = hasSecuritySignals
      ? "Active security signals require human review before proceeding."
      : "High-urgency decision windows demand human judgment.";
  } else if (signalDensity > 3 || relevantWisdom.length > 3) {
    delegationLevel = "review_needed";
    delegationReason = `${signalDensity} active signal(s) and ${relevantWisdom.length} relevant decision(s) suggest review after completion.`;
  } else {
    delegationLevel = "safe_to_delegate";
    delegationReason = "No significant advisories or constraints detected for this task.";
  }

  // ── 6. Summary ────────────────────────────────────────────────────────
  const parts: string[] = [];
  if (advisories.length > 0) {
    parts.push(`${advisories.length} advisor${advisories.length !== 1 ? "ies" : "y"}`);
  }
  if (decisionWindows.length > 0) {
    parts.push(`${decisionWindows.length} decision window${decisionWindows.length !== 1 ? "s" : ""}`);
  }
  if (relevantWisdom.length > 0) {
    parts.push(`${relevantWisdom.length} relevant decision${relevantWisdom.length !== 1 ? "s" : ""}/memor${relevantWisdom.length !== 1 ? "ies" : "y"}`);
  }
  if (ecosystemNews.length > 0) {
    parts.push(`${ecosystemNews.length} ecosystem update${ecosystemNews.length !== 1 ? "s" : ""}`);
  }

  const summary =
    parts.length > 0
      ? `Found ${parts.join(", ")} relevant to this task. Delegation: ${delegationLevel}.`
      : "No active advisories or signals for this task. Proceed normally.";

  return {
    task,
    files,
    advisories,
    decision_windows: decisionWindows,
    relevant_wisdom: relevantWisdom,
    ecosystem_news: ecosystemNews,
    delegation_assessment: {
      level: delegationLevel,
      reason: delegationReason,
    },
    summary,
  };
}
