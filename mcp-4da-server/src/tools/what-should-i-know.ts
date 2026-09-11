// SPDX-License-Identifier: Apache-2.0
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
import type { LiveIntelligence } from "../live/index.js";
import { isActionableVulnerability } from "../live/maintenance.js";
import { presentedSeverity } from "../live/severity-scope.js";
import { executeGetActionableSignals } from "./get-actionable-signals.js";
import { installDriftAdvisories } from "./install-drift-notes.js";
import { getLiveIntelligence } from "../live-singleton.js";
import { createRelevanceScorer } from "./recall.js";
import { getEmbeddingConfig } from "../embeddings.js";
import {
  getRelevantWisdom,
  getRelevantWisdomHybrid,
  type WisdomEntry,
  type WisdomRecallMode,
} from "./briefing-wisdom.js";

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

interface EcosystemNewsItem {
  title: string;
  url: string | null;
  points: number;
  relevance_reason: string;
}

/**
 * "unknown" is the verdict when the vulnerability scan could not be consulted:
 * the briefing has not reviewed the task, which is a different claim from
 * "reviewed and found nothing". "safe_to_delegate" is only ever emitted over a
 * ready scan.
 */
export type DelegationLevel = "safe_to_delegate" | "review_needed" | "human_only" | "unknown";

/**
 * Whether the live vulnerability scan backed this briefing.
 * - ready:       a completed, online scan was consulted
 * - unavailable: live intelligence is on but the scan is still running past
 *                the wait budget, finished offline, or covered no resolvable
 *                dependency versions
 * - disabled:    no live intelligence (FOURDA_OFFLINE, or not initialised)
 */
export type ScanStatus = "ready" | "unavailable" | "disabled";

/** What backed the briefing's vulnerability claims, and how current it is. */
export interface BriefingScan {
  status: ScanStatus;
  /** When the scan the briefing read ran, or null when none backed it. */
  scanned_at: string | null;
  /** When the dependency versions it covers were read from the lockfiles. */
  resolved_at: string | null;
  /** True when this call noticed a changed lockfile and re-resolved first. */
  re_resolved_this_call: boolean;
}

export interface WhatShouldIKnowResult {
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
  /** Kept for existing callers; `scan.status` carries the same value. */
  scan_status: ScanStatus;
  scan: BriefingScan;
  summary: string;
  /** How relevant_wisdom was retrieved: "hybrid" when an embedding provider is active. */
  wisdom_recall_mode: WisdomRecallMode;
}

/**
 * The slice of the live layer the briefing reads; a stub suffices in tests.
 * Re-resolution and provenance are optional so a minimal stub still works.
 */
export type BriefingLiveIntel = Pick<
  LiveIntelligence,
  "ensureVulnerabilities" | "isEnabled" | "getProjectRoot" | "getHeadlines" | "getVulnerabilities"
> &
  Partial<Pick<LiveIntelligence, "refreshIfLockfilesChanged" | "getResolutionProvenance">>;

/**
 * How long a briefing waits for the startup vulnerability scan. OSV's batch
 * query plus advisory hydration for a real project finishes well inside this
 * on a warm network; past it the briefing answers "unknown" rather than block.
 */
const SCAN_WAIT_MS = 8_000;

const SCAN_UNAVAILABLE_REASON =
  "Vulnerability scan unavailable — treat this task as unreviewed, not as safe";

const PRIORITY_ORDER: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };

// ============================================================================
// Tool Definition
// ============================================================================

export const whatShouldIKnowTool = {
  name: "what_should_i_know",
  description:
    "Pre-task intelligence briefing. Given a task description and optional file paths, returns filtered advisories, decision windows, signal chains, relevant wisdom, a delegation assessment (safe_to_delegate | review_needed | human_only | unknown) and scan_status (ready | unavailable | disabled). Waits for the live vulnerability scan; when it is not available the verdict is \"unknown\" — never \"safe\". Call before starting any non-trivial task. If the task involves upgrading, adding, or auditing dependencies, follow with upgrade_planner for the ranked plan.",
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
      // opened_at, not created_at — the wrong column name made this query
      // throw (swallowed below) on EVERY schema in existence, so the
      // decision-windows section had never returned a row for anyone.
      `SELECT id, title, description, urgency
       FROM decision_windows WHERE status = 'open'
       ORDER BY urgency DESC, opened_at DESC
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

export async function executeWhatShouldIKnow(
  db: FourDADatabase,
  params: WhatShouldIKnowParams,
  liveIntel: BriefingLiveIntel | null = getLiveIntelligence(),
): Promise<WhatShouldIKnowResult> {
  const task = params.task;
  const files = params.files || [];
  // One alias-aware scorer for the whole briefing, so advisories, decision
  // windows and ecosystem news share the SAME relevance model as relevant_wisdom
  // (an "auth" task now matches a "jwt"/"oauth" advisory; substring matching did not).
  const relevance = createRelevanceScorer([task, ...files].join(" "));

  // ── 0. The vulnerability scan — awaited, bounded, never assumed ───────
  // The first briefing of a session used to read the scan cache before the
  // startup scan had finished (or, in full-database mode, before any scan had
  // been started at all), found nothing, and reported safe_to_delegate for a
  // task naming a package with an open advisory. Now the briefing waits for
  // the warmup, and records whether a scan actually backed the answer.
  let scanStatus: ScanStatus = "disabled";
  let scan: Awaited<ReturnType<BriefingLiveIntel["ensureVulnerabilities"]>> = null;
  let reResolvedThisCall = false;
  if (liveIntel && liveIntel.isEnabled()) {
    try {
      // A lockfile changed since the versions were read? Re-resolve before
      // anything answers from them (stat calls only).
      reResolvedThisCall = liveIntel.refreshIfLockfilesChanged?.() ?? false;
    } catch {
      reResolvedThisCall = false;
    }
    try {
      const projectRoot = liveIntel.getProjectRoot() ?? process.cwd();
      scan = await liveIntel.ensureVulnerabilities(projectRoot, SCAN_WAIT_MS);
    } catch {
      scan = null;
    }
    // A scan that resolved no dependency versions checked nothing.
    scanStatus = scan && scan.totalScanned > 0 ? "ready" : "unavailable";
  }
  let resolvedAt: string | null = null;
  try {
    resolvedAt = liveIntel?.getResolutionProvenance?.()?.resolvedAt ?? null;
  } catch {
    resolvedAt = null;
  }

  // ── 1. Actionable Signals (security, breaking changes, etc.) ──────────
  // Two passes over the feed: the 72-hour window for everything, and a
  // 30-day window for security alerts alone — a three-day-old advisory was
  // being cut by the short window. Merged by id; live scan rows (id -1)
  // appear in both passes and merge by title. Live rows at LOW priority are
  // platform-inactive or maintenance notices: kept out of the briefing so
  // they cannot drive the delegation verdict.
  let advisories: Advisory[] = [];
  try {
    const seen = new Set<string>();
    const merged: ReturnType<typeof executeGetActionableSignals>["signals"] = [];
    const passes = [
      executeGetActionableSignals(db, { limit: 50, since_hours: 72 }, liveIntel),
      executeGetActionableSignals(
        db,
        { signal_type: "security_alert", since_hours: 720, limit: 50 },
        liveIntel,
      ),
    ];
    for (const pass of passes) {
      for (const s of pass.signals) {
        if (s.id === -1 && s.signal_priority === "low") continue;
        const key = s.id === -1 ? `live:${s.title}` : `id:${s.id}`;
        if (seen.has(key)) continue;
        seen.add(key);
        merged.push(s);
      }
    }
    merged.sort((a, b) => {
      const pd = (PRIORITY_ORDER[b.signal_priority] || 0) - (PRIORITY_ORDER[a.signal_priority] || 0);
      return pd !== 0 ? pd : b.relevance_score - a.relevance_score;
    });

    advisories = merged
      .filter((s) => {
        // Include all critical/high security signals unconditionally
        if (s.signal_type === "security_alert" && (s.signal_priority === "critical" || s.signal_priority === "high")) {
          return true;
        }
        // Otherwise, filter by alias-aware relevance to the task
        return relevance((s.title || "") + " " + (s.action || "")) > 0;
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

  // ── 1b. Live vulnerability summary ───────────────────────────────────
  // Counted over the actionable set only (built on this host, not a
  // maintenance notice) — the same set vulnerability_scan reports. Graded by
  // the presented severity every surface shares, not the raw advisory tier.
  if (scan) {
    const actionable = scan.vulnerabilities.filter(isActionableVulnerability);
    const packages = new Set(actionable.map((v) => v.package));
    if (packages.size > 0) {
      const details = actionable
        .slice(0, 3)
        .map((v) => `${v.package}@${v.currentVersion}: ${v.summary}`)
        .join("; ");
      const severities = new Set(actionable.map((v) => presentedSeverity(v)));
      advisories.unshift({
        title: `${packages.size} dependenc${packages.size !== 1 ? "ies have" : "y has"} known vulnerabilities`,
        signal_type: "security_alert",
        priority: severities.has("critical") ? "critical" : severities.has("high") ? "high" : "medium",
        action: `Run vulnerability_scan for full details. ${details}`,
        url: null,
      });
    }
    // An installed copy that the lockfile does not pin, and that OSV lists as
    // vulnerable, is named with its reinstall command whatever the task: a
    // relevance filter must not be what stands between the reader and a
    // vulnerable copy that is actually running.
    advisories.splice(1, 0, ...installDriftAdvisories(scan));
  }

  // ── 2. Decision Windows ───────────────────────────────────────────────
  let decisionWindows: DecisionWindow[] = [];
  try {
    const windows = getOpenDecisionWindows(db);
    decisionWindows = windows
      .filter((w) => relevance((w.title || "") + " " + (w.description || "")) > 0)
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

  // ── 3. Ecosystem News (HN headlines relevant to tech stack) ───────────
  let ecosystemNews: EcosystemNewsItem[] = [];
  try {
    if (liveIntel) {
      const headlines = liveIntel.getHeadlines();
      ecosystemNews = headlines
        .filter((h) => h.relevanceScore > 0.3 || relevance(h.title) > 0)
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

  const scanBlock: BriefingScan = {
    status: scanStatus,
    scanned_at: scan?.scannedAt ?? null,
    resolved_at: resolvedAt,
    re_resolved_this_call: reResolvedThisCall,
  };

  // ── 4. Assembly ────────────────────────────────────────────────────────
  const finalize = (
    relevantWisdom: WisdomEntry[],
    wisdomMode: WisdomRecallMode,
  ): WhatShouldIKnowResult => {
    const signalDensity = advisories.length + decisionWindows.length;

    const hasSecuritySignals = advisories.some(
      (a) => a.signal_type === "security_alert" && (a.priority === "critical" || a.priority === "high"),
    );
    const hasHighUrgencyWindows = decisionWindows.some((w) => w.urgency >= 4);
    // Consequence-bearing advisories that survived the relevance filter: a
    // medium advisory in the very package being upgraded is not "nothing".
    const consequential = advisories.filter(
      (a) => a.signal_type === "security_alert" || a.signal_type === "breaking_change",
    ).length;

    let delegationLevel: DelegationLevel;
    let delegationReason: string;

    if (hasSecuritySignals || hasHighUrgencyWindows) {
      // Evidence already in hand wins regardless of scan status.
      delegationLevel = "human_only";
      delegationReason = hasSecuritySignals
        ? "Active security signals require human review before proceeding."
        : "High-urgency decision windows demand human judgment.";
    } else if (scanStatus !== "ready") {
      // No scan, no "safe": the task is unreviewed, which is not the same
      // claim as reviewed-and-clean.
      delegationLevel = "unknown";
      delegationReason =
        scanStatus === "disabled"
          ? `${SCAN_UNAVAILABLE_REASON} (live intelligence is disabled).`
          : `${SCAN_UNAVAILABLE_REASON}.`;
    } else if (signalDensity > 3 || relevantWisdom.length > 3) {
      delegationLevel = "review_needed";
      delegationReason = `${signalDensity} active signal(s) and ${relevantWisdom.length} relevant decision(s) suggest review after completion.`;
    } else if (consequential > 0) {
      delegationLevel = "review_needed";
      delegationReason = `${consequential} security/breaking-change advisor${consequential !== 1 ? "ies" : "y"} relevant to this task suggest review after completion.`;
    } else {
      delegationLevel = "safe_to_delegate";
      delegationReason = "Vulnerability scan ready; no significant advisories or constraints detected for this task.";
    }

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

    let summary: string;
    if (parts.length > 0) {
      summary = `Found ${parts.join(", ")} relevant to this task. Delegation: ${delegationLevel}.`;
    } else if (scanStatus === "ready") {
      summary = "No active advisories or signals for this task. Proceed normally.";
    } else {
      summary = `No active advisories or signals found, but the vulnerability scan is ${scanStatus}. Delegation: ${delegationLevel}.`;
    }

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
      scan_status: scanStatus,
      scan: scanBlock,
      summary,
      wisdom_recall_mode: wisdomMode,
    };
  };

  // Wisdom retrieval is lexical by default and hybrid when an embedding
  // provider is configured.
  const embedConfig = getEmbeddingConfig();
  if (!embedConfig) {
    return finalize(getRelevantWisdom(db, task, files), "ranked_lexical");
  }
  const { wisdom, recall_mode } = await getRelevantWisdomHybrid(db, task, files, embedConfig);
  return finalize(wisdom, recall_mode);
}
