// SPDX-License-Identifier: Apache-2.0
/**
 * get_actionable_signals tool
 *
 * Classifies source items into actionable signal types (security alerts,
 * breaking changes, tool discoveries, etc.) with priority levels.
 * Cross-references against the user's ACE-detected tech stack.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { VulnerabilityEntry } from "../live/types.js";
import { isMaintenanceNotice } from "../live/maintenance.js";
import { getLiveIntelligence } from "../live-singleton.js";
import {
  classify,
  normalizeStoredPriority,
  type SignalPriority,
  type SignalType,
} from "./signal-classifier.js";

// The keyword classifier and the priority vocabulary live in
// signal-classifier.ts; re-exported for existing importers.
export { classify, normalizeStoredPriority, type SignalPriority, type SignalType } from "./signal-classifier.js";

interface ClassifiedSignal {
  id: number;
  title: string;
  url: string | null;
  source_type: string;
  relevance_score: number;
  signal_type: SignalType;
  signal_priority: SignalPriority;
  action: string;
  triggers: string[];
  confidence: number;
  discovered_ago: string;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const getActionableSignalsTool = {
  name: "get_actionable_signals",
  description: `Get actionable signals classified from recent content.

Categorizes items into signal types: security_alert, breaking_change,
tool_discovery, tech_trend, learning, competitive_intel.
Each signal has a priority level (critical/high/medium/low) based on
signal type, relevance score, and tech stack match.

Use this to get prioritized, actionable intelligence from 4DA's feed.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      priority_filter: {
        type: "string",
        description: 'Filter by priority level: "critical", "high", "medium", "low". Leave empty for all.',
        enum: ["critical", "high", "medium", "low"],
      },
      signal_type: {
        type: "string",
        description: 'Filter by signal type. Leave empty for all.',
        enum: [
          "security_alert", "breaking_change", "tool_discovery",
          "tech_trend", "learning", "competitive_intel",
        ],
      },
      limit: {
        type: "number",
        description: "Maximum number of signals to return. Default: 20",
        default: 20,
      },
      since_hours: {
        type: "number",
        description: "Only include items from the last N hours. Default: 48, max: 720 (30 days)",
        default: 48,
      },
    },
  },
};

export interface GetActionableSignalsParams {
  priority_filter?: SignalPriority;
  signal_type?: SignalType;
  limit?: number;
  since_hours?: number;
}

/** The slice of the live layer this tool reads; a stub suffices in tests. */
export type SignalsLiveIntel = Pick<LiveIntelligence, "getVulnerabilities">;

/** Widest window a caller may ask for: 30 days. */
const MAX_SINCE_HOURS = 720;
/** Windows past this reach rows the current scoring pipeline may not have re-scored. */
const DEEP_WINDOW_HOURS = 168;

// ============================================================================
// Alias clustering for live vulnerabilities
// ============================================================================

/** One signal's worth of live findings: the entry to report and every id it is known by. */
export interface VulnerabilityCluster {
  representative: VulnerabilityEntry;
  ids: string[];
}

/**
 * Group scan entries that are the same vulnerability under different ids.
 *
 * OSV returns a GHSA record and a RUSTSEC (or CVE) record for one bug, each
 * naming the other in `aliases`; injected one-per-record they read as two
 * problems and doubled the advisory count in every briefing. Union-find over
 * `[vulnId, ...aliases]` connects the records transitively (A knows B, B knows
 * C), scoped to the affected (ecosystem, package, version) so one advisory
 * against two packages — or two pinned versions of one package — stays two
 * things to fix. The representative is the GHSA record when the cluster has
 * one (the richest and most stable id), else the first seen; `ids` lists
 * every identifier in first-seen order.
 */
export function clusterVulnerabilities(vulns: VulnerabilityEntry[]): VulnerabilityCluster[] {
  const parent = new Map<string, string>();
  const find = (key: string): string => {
    let root = key;
    while (parent.get(root) !== root) root = parent.get(root)!;
    // Path compression keeps repeated lookups flat.
    let cursor = key;
    while (parent.get(cursor) !== root) {
      const next = parent.get(cursor)!;
      parent.set(cursor, root);
      cursor = next;
    }
    return root;
  };
  const union = (a: string, b: string): void => {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent.set(ra, rb);
  };
  const scopeOf = (v: VulnerabilityEntry) => `${v.ecosystem}\0${v.package}\0${v.currentVersion}`;
  const idsOf = (v: VulnerabilityEntry) => [v.vulnId, ...v.aliases].filter(Boolean);

  for (const v of vulns) {
    const scope = scopeOf(v);
    const keys = idsOf(v).map((id) => `${scope}\0${id}`);
    for (const key of keys) if (!parent.has(key)) parent.set(key, key);
    for (let i = 1; i < keys.length; i++) union(keys[0], keys[i]);
  }

  const groups = new Map<string, VulnerabilityEntry[]>();
  for (const v of vulns) {
    const root = find(`${scopeOf(v)}\0${v.vulnId}`);
    const members = groups.get(root);
    if (members) members.push(v);
    else groups.set(root, [v]);
  }

  return [...groups.values()].map((members) => {
    const representative = members.find((m) => m.vulnId.startsWith("GHSA-")) ?? members[0];
    const ids = [...new Set(members.flatMap(idsOf))];
    return { representative, ids };
  });
}

// ============================================================================
// Execution
// ============================================================================

export function executeGetActionableSignals(
  db: FourDADatabase,
  params: GetActionableSignalsParams,
  liveIntel: SignalsLiveIntel | null = getLiveIntelligence(),
): { signals: ClassifiedSignal[]; total: number; summary: Record<string, number> } {
  const limit = Math.max(1, Math.min(100, params.limit ?? 20));
  const sinceHours = Math.max(1, Math.min(MAX_SINCE_HOURS, params.since_hours ?? 48));

  // Get items from DB (low min score to get more items). A signal_type filter
  // is pushed into the read as well: the general read is capped at the top
  // 200 ranked items across EVERY type, and on a real corpus that cap starved
  // a type-specific pass — live 2026-09-07, the two in-window stored security
  // alerts ranked 299th and 582nd, so a security-only read returned nothing.
  // Stored matches are fetched by type and merged ahead of the general read,
  // which still carries the unstamped rows for keyword classification.
  const fetchItems = (minScore: number, hours: number, requireCurrentVersion: boolean) => {
    const general = db.getRelevantContent(minScore, undefined, 200, hours, requireCurrentVersion);
    if (!params.signal_type) return general;
    const stored = db.getRelevantContent(minScore, undefined, 200, hours, requireCurrentVersion, params.signal_type);
    const seen = new Set(stored.map((item) => item.id));
    return [...stored, ...general.filter((item) => !seen.has(item.id))];
  };

  // Try progressively wider time windows if no items found. A window wider
  // than seven days reaches the stale-epoch tail, so it carries the same
  // current-pipeline-version guard as the deep fallback below.
  let items = fetchItems(0.1, sinceHours, sinceHours > DEEP_WINDOW_HOURS);
  if (items.length === 0 && sinceHours < 168) {
    items = fetchItems(0.1, 168, false); // Try 7 days
  }
  if (items.length === 0) {
    // Deep 30-day/any-score fallback reaches the stale-epoch tail after a
    // pipeline-version bump. This path trusts stored signal_type/signal_priority
    // verbatim (confidence 0.90 below), so it MUST only see current-version
    // rows — stale persisted signals are claims the live brain no longer makes.
    items = fetchItems(0.0, 720, true);
  }

  // Get user's detected tech for cross-referencing
  const context = db.getUserContext(true, false);
  const detectedTech = (context.ace?.detected_tech || []).map((t: { name: string }) => t.name);

  const signals: ClassifiedSignal[] = [];

  for (const item of items) {
    // Prefer pipeline-computed signals from DB over keyword re-classification.
    // The pipeline's priority vocabulary (critical/alert/advisory/watch) is
    // mapped onto this tool's tiers rather than cast through unchanged.
    if (item.signal_type && item.signal_priority) {
      const storedType = item.signal_type as SignalType;
      const storedPriority = normalizeStoredPriority(item.signal_priority);

      // Apply filters
      if (params.priority_filter && storedPriority !== params.priority_filter) continue;
      if (params.signal_type && storedType !== params.signal_type) continue;

      signals.push({
        id: item.id,
        title: item.title,
        url: item.url,
        source_type: item.source_type,
        relevance_score: item.relevance_score,
        signal_type: storedType,
        signal_priority: storedPriority,
        action: `${storedType}: ${item.title.substring(0, 60)}`,
        triggers: [],
        confidence: 0.90,
        discovered_ago: item.discovered_ago,
      });
      continue;
    }

    // Fallback: keyword classification for items without pipeline signals
    const result = classify(
      item.title,
      item.content || "",
      item.relevance_score,
      detectedTech
    );

    if (!result) continue;

    // Apply filters
    if (params.priority_filter && result.priority !== params.priority_filter) continue;
    if (params.signal_type && result.signalType !== params.signal_type) continue;

    signals.push({
      id: item.id,
      title: item.title,
      url: item.url,
      source_type: item.source_type,
      relevance_score: item.relevance_score,
      signal_type: result.signalType,
      signal_priority: result.priority,
      action: result.action,
      triggers: result.triggers,
      confidence: result.confidence,
      discovered_ago: item.discovered_ago,
    });
  }

  // Inject live vulnerability data as security_alert signals — one per
  // vulnerability (alias clusters), not one per OSV record.
  const vulnResult = liveIntel?.getVulnerabilities() ?? null;
  if (vulnResult && vulnResult.vulnerabilities.length > 0) {
    if (!params.signal_type || params.signal_type === "security_alert") {
      for (const { representative: vuln, ids } of clusterVulnerabilities(vulnResult.vulnerabilities)) {
        let priority: SignalPriority =
          vuln.severity === "critical" ? "critical" :
          vuln.severity === "high" ? "high" : "medium";
        let relevance = vuln.severity === "critical" ? 1.0 : vuln.severity === "high" ? 0.9 : 0.7;
        let action = vuln.fixedVersion
          ? `Upgrade ${vuln.package} to ${vuln.fixedVersion}`
          : `Review ${vuln.package} — no fix version published`;
        let label = vuln.severity.toUpperCase();

        // "X is unmaintained" is information to plan around, not a fix to
        // apply today; an advisory against a crate this host never builds is
        // not this host's exposure. Both stay visible at LOW so they cannot
        // outrank, or be mistaken for, a live CVE — and the briefing's
        // delegation verdict does not read them at all.
        const maintenance = isMaintenanceNotice(vuln);
        if (maintenance) {
          priority = "low";
          relevance = 0.3;
          action = "Maintenance notice — no fix to apply";
          label = "MAINTENANCE";
        }
        if (vuln.platformActive === false) {
          priority = "low";
          relevance = 0.3;
          action = `Not built on this host — ${action}`;
        }

        if (params.priority_filter && priority !== params.priority_filter) continue;

        signals.push({
          id: -1,
          title: `${label}: ${vuln.summary} (${vuln.package}@${vuln.currentVersion})`,
          url: vuln.references[0] || null,
          source_type: "osv_live",
          relevance_score: relevance,
          signal_type: "security_alert",
          signal_priority: priority,
          action,
          triggers: ids,
          confidence: 1.0,
          discovered_ago: vuln.published ? `published ${vuln.published}` : "recently",
        });
      }
    }
  }

  // Sort by priority (critical first), then by relevance score
  const priorityOrder: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
  signals.sort((a, b) => {
    const pd = (priorityOrder[b.signal_priority] || 0) - (priorityOrder[a.signal_priority] || 0);
    if (pd !== 0) return pd;
    return b.relevance_score - a.relevance_score;
  });

  // Summary counts by type
  const summary: Record<string, number> = {};
  for (const s of signals) {
    summary[s.signal_type] = (summary[s.signal_type] || 0) + 1;
  }

  return {
    signals: signals.slice(0, limit),
    total: signals.length,
    summary,
    ...(signals.length === 0 && items.length === 0 ? { note: "No source items in the database. Run the 4DA desktop app to fetch content, or use vulnerability_scan to check dependencies directly." } : {}),
  };
}
