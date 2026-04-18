// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * what_should_i_know tool
 *
 * Single entry point for external AI agents to get task-relevant intelligence
 * before starting work. Synthesizes multiple intelligence layers:
 * - Actionable signals (security alerts, breaking changes)
 * - Decision windows (time-bounded opportunities)
 * - Signal chains (escalating causal narratives)
 * - Session brief (decisions, concerns, memories)
 *
 * Filters everything for relevance to the described task and involved files.
 */

import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import type { FourDADatabase } from "../db.js";
import { executeGetActionableSignals } from "./get-actionable-signals.js";
import { executeSourceHealth } from "./source-health.js";
import { executeSignalChains } from "./signal-chains.js";
import { executeAgentSessionBrief } from "./agent-session-brief.js";
import { findAweBinary } from "./decision-memory.js";

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

interface ActiveSignal {
  chain_name: string;
  priority: string;
  suggested_action: string;
  link_count: number;
}

interface WisdomEntry {
  type: string;
  subject: string;
  detail: string;
}

type DelegationLevel = "safe_to_delegate" | "review_needed" | "human_only";

interface WhatShouldIKnowResult {
  task: string;
  files: string[];
  advisories: Advisory[];
  decision_windows: DecisionWindow[];
  active_signals: ActiveSignal[];
  relevant_wisdom: WisdomEntry[];
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
// AWE Wisdom Engine
// ============================================================================

/**
 * Retrieve AWE Wisdom Engine status and validated principles.
 * Uses the AWE CLI binary — no direct database access, no new dependencies.
 */
function getAweWisdom(): string | null {
  try {
    const aweBin = findAweBinary();
    if (!aweBin) return null;

    // Quick existence check for the wisdom database
    const dbPath = process.platform === "win32"
      ? `${process.env.APPDATA || ""}\\awe\\wisdom.db`
      : `${process.env.HOME || ""}/.local/share/awe/wisdom.db`;

    if (!existsSync(dbPath)) return null;

    let section = "AWE WISDOM ENGINE\n";

    try {
      const statusOutput = execSync(`"${aweBin}" status`, {
        timeout: 5000,
        encoding: "utf-8",
        stdio: ["ignore", "pipe", "ignore"],
      });
      if (statusOutput) section += statusOutput.trim() + "\n";
    } catch {
      // status command failed — continue without it
    }

    try {
      const wisdomOutput = execSync(
        `"${aweBin}" wisdom --domain software-engineering`,
        {
          timeout: 5000,
          encoding: "utf-8",
          stdio: ["ignore", "pipe", "ignore"],
        },
      );
      if (wisdomOutput && wisdomOutput.includes("VALIDATED")) {
        section += wisdomOutput.trim() + "\n";
      }
    } catch {
      // wisdom command failed — continue without it
    }

    // Only return if we got something beyond the header
    return section.length > "AWE WISDOM ENGINE\n".length ? section : null;
  } catch {
    return null;
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
        return matchesKeywords(s.title + " " + s.action, keywords);
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

  // ── 2. Source Health (CVE/security check) ─────────────────────────────
  try {
    const healthResult = executeSourceHealth(db, { hours: 48 });
    if (healthResult.overall_status === "critical") {
      advisories.unshift({
        title: "Source health critical — some data sources are failing",
        signal_type: "system_health",
        priority: "high",
        action: "Run source_health for details. Content may be stale.",
        url: null,
      });
    }
  } catch {
    // Health check unavailable — non-fatal
  }

  // ── 3. Signal Chains ──────────────────────────────────────────────────
  let activeSignals: ActiveSignal[] = [];
  try {
    const chainResult = executeSignalChains(db, {
      resolution: "open",
      min_priority: "medium",
    });

    activeSignals = chainResult.chains
      .filter((c) => matchesKeywords(c.chain_name + " " + c.suggested_action, keywords))
      .slice(0, 5)
      .map((c) => ({
        chain_name: c.chain_name,
        priority: c.overall_priority,
        suggested_action: c.suggested_action,
        link_count: Array.isArray(c.links) ? c.links.length : 0,
      }));
  } catch {
    // Chains unavailable — non-fatal
  }

  // ── 4. Decision Windows ───────────────────────────────────────────────
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

  // ── 5. Session Brief (decisions, memories, ecosystem) ─────────────────
  let relevantWisdom: WisdomEntry[] = [];
  try {
    const brief = executeAgentSessionBrief(db, {
      include_decisions: true,
      include_memories: true,
    }) as {
      active_decisions?: Array<{ subject: string; decision: string; type: string }>;
      ecosystem_changes?: Array<{ change_type: string; subject: string; summary: string }>;
      agent_memories?: Array<{ memory_type: string; subject: string; content: string }>;
    };

    // Filter active decisions for relevance
    if (brief.active_decisions) {
      for (const d of brief.active_decisions) {
        if (matchesKeywords(d.subject + " " + d.decision, keywords)) {
          relevantWisdom.push({
            type: "decision",
            subject: d.subject,
            detail: d.decision,
          });
        }
      }
    }

    // Filter ecosystem changes for relevance
    if (brief.ecosystem_changes) {
      for (const ec of brief.ecosystem_changes) {
        if (matchesKeywords(ec.subject + " " + ec.summary, keywords)) {
          relevantWisdom.push({
            type: ec.change_type,
            subject: ec.subject,
            detail: ec.summary,
          });
        }
      }
    }

    // Filter agent memories for relevance
    if (brief.agent_memories) {
      for (const m of brief.agent_memories) {
        if (matchesKeywords(m.subject + " " + m.content, keywords)) {
          relevantWisdom.push({
            type: m.memory_type,
            subject: m.subject,
            detail: m.content,
          });
        }
      }
    }

    // Cap wisdom entries
    relevantWisdom = relevantWisdom.slice(0, 10);
  } catch {
    // Brief unavailable — non-fatal
  }

  // ── 5b. AWE Wisdom Engine (validated principles, decision stats) ──────
  try {
    const aweWisdom = getAweWisdom();
    if (aweWisdom) {
      relevantWisdom.push({
        type: "awe_wisdom",
        subject: "AWE Wisdom Engine",
        detail: aweWisdom,
      });
    }
  } catch {
    // AWE unavailable — non-fatal
  }

  // ── 6. Delegation Assessment ──────────────────────────────────────────
  const signalDensity =
    advisories.length + decisionWindows.length + activeSignals.length;

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

  // ── 7. Summary ────────────────────────────────────────────────────────
  const parts: string[] = [];
  if (advisories.length > 0) {
    parts.push(`${advisories.length} advisor${advisories.length !== 1 ? "ies" : "y"}`);
  }
  if (decisionWindows.length > 0) {
    parts.push(`${decisionWindows.length} decision window${decisionWindows.length !== 1 ? "s" : ""}`);
  }
  if (activeSignals.length > 0) {
    parts.push(`${activeSignals.length} signal chain${activeSignals.length !== 1 ? "s" : ""}`);
  }
  if (relevantWisdom.length > 0) {
    parts.push(`${relevantWisdom.length} relevant decision${relevantWisdom.length !== 1 ? "s" : ""}/memor${relevantWisdom.length !== 1 ? "ies" : "y"}`);
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
    active_signals: activeSignals,
    relevant_wisdom: relevantWisdom,
    delegation_assessment: {
      level: delegationLevel,
      reason: delegationReason,
    },
    summary,
  };
}
