// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_actionable_signals tool
 *
 * Classifies source items into actionable signal types (security alerts,
 * breaking changes, tool discoveries, etc.) with priority levels.
 * Cross-references against the user's ACE-detected tech stack.
 */

import type { FourDADatabase } from "../db.js";
import { getLiveIntelligence } from "../live-singleton.js";

// ============================================================================
// Signal Classification Types
// ============================================================================

type SignalType =
  | "security_alert"
  | "breaking_change"
  | "tool_discovery"
  | "tech_trend"
  | "learning"
  | "competitive_intel";

type SignalPriority = "critical" | "high" | "medium" | "low";

interface SignalPattern {
  keywords: string[];
  boostWords: string[];
  weight: number;
}

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
// Pattern Definitions
// ============================================================================

const SIGNAL_PATTERNS: Record<SignalType, SignalPattern> = {
  security_alert: {
    keywords: [
      "cve", "vulnerability", "exploit", "breach", "security flaw",
      "zero-day", "zero day", "0-day", "patch", "ransomware",
      "malware", "rce", "injection attack", "xss", "csrf",
      "privilege escalation", "backdoor", "supply chain attack",
    ],
    boostWords: ["critical", "urgent", "severe", "actively exploited", "emergency"],
    weight: 1.0,
  },
  breaking_change: {
    keywords: [
      "breaking change", "deprecated", "end of life", "eol",
      "migration guide", "major release", "drops support",
      "removed in", "no longer supported", "sunset",
      "backwards incompatible", "api change",
    ],
    boostWords: ["v2", "v3", "v4", "v5", "major version", "upgrade required"],
    weight: 0.9,
  },
  tool_discovery: {
    keywords: [
      "new release", "just released", "announcing", "launch",
      "alternative to", "built with", "replacement for",
      "open source", "open-source", "introducing",
      "we built", "i built", "show hn",
    ],
    boostWords: ["faster", "better", "simpler", "lightweight", "blazing"],
    weight: 0.7,
  },
  tech_trend: {
    keywords: [
      "adoption", "growing", "trending", "benchmark",
      "comparison", "state of", "survey", "report",
      "market share", "ecosystem", "roadmap",
    ],
    boostWords: ["2025", "2026", "accelerating", "mainstream", "industry"],
    weight: 0.6,
  },
  learning: {
    keywords: [
      "tutorial", "how to", "guide", "deep dive",
      "explained", "best practices", "patterns",
      "architecture", "lessons learned", "walkthrough",
      "step by step", "from scratch",
    ],
    boostWords: ["advanced", "production", "real-world", "comprehensive"],
    weight: 0.5,
  },
  competitive_intel: {
    keywords: [
      "acquired", "funding", "raised", "ipo",
      "valuation", "market share", "competitor",
      "pivots", "pivot", "layoffs", "shutdown",
      "acqui-hire", "series a", "series b",
    ],
    boostWords: ["million", "billion", "disrupts", "overtakes"],
    weight: 0.6,
  },
};

const BASE_WEIGHTS: Record<SignalType, number> = {
  security_alert: 2,
  breaking_change: 2,
  tool_discovery: 1,
  tech_trend: 1,
  learning: 1,
  competitive_intel: 1,
};

// Word-boundary matching prevents "rce" matching inside "source", "xss" inside "success", etc.
function hasWordBoundary(text: string, term: string): boolean {
  let searchFrom = 0;
  while (true) {
    const pos = text.indexOf(term, searchFrom);
    if (pos === -1) return false;
    const beforeOk = pos === 0 || !/[a-zA-Z0-9]/.test(text[pos - 1]);
    const afterIdx = pos + term.length;
    const afterOk = afterIdx >= text.length || !/[a-zA-Z0-9]/.test(text[afterIdx]);
    if (beforeOk && afterOk) return true;
    searchFrom = pos + 1;
  }
}

const PRIORITY_LABELS: Record<string, string> = {
  security_alert: "Security Alert",
  breaking_change: "Breaking Change",
  tool_discovery: "Tool Discovery",
  tech_trend: "Tech Trend",
  learning: "Learning",
  competitive_intel: "Competitive Intel",
};

// ============================================================================
// Classifier - Optimized with flat keyword map
// ============================================================================

// Build flat keyword→signal_type lookup map at module load (one-time cost)
interface KeywordEntry {
  type: SignalType;
  weight: number;
  isBoost: boolean;
}

const KEYWORD_MAP: Map<string, KeywordEntry[]> = new Map();

// Initialize keyword map
for (const [type, pattern] of Object.entries(SIGNAL_PATTERNS) as [SignalType, SignalPattern][]) {
  for (const kw of pattern.keywords) {
    if (!KEYWORD_MAP.has(kw)) {
      KEYWORD_MAP.set(kw, []);
    }
    KEYWORD_MAP.get(kw)!.push({ type, weight: pattern.weight, isBoost: false });
  }
  for (const bw of pattern.boostWords) {
    if (!KEYWORD_MAP.has(bw)) {
      KEYWORD_MAP.set(bw, []);
    }
    KEYWORD_MAP.get(bw)!.push({ type, weight: 0.2, isBoost: true });
  }
}

function classify(
  title: string,
  content: string,
  relevanceScore: number,
  detectedTech: string[]
): {
  signalType: SignalType;
  priority: SignalPriority;
  confidence: number;
  action: string;
  triggers: string[];
} | null {
  // Pre-compute lowercased text once
  const textLower = `${title} ${content}`.toLowerCase();
  const titleLower = title.toLowerCase();

  // Track scores per signal type
  const typeScores: Record<string, { score: number; matched: string[] }> = {};

  // Single pass through keyword map (word-boundary matched to prevent false positives)
  for (const [keyword, entries] of KEYWORD_MAP.entries()) {
    if (hasWordBoundary(textLower, keyword)) {
      for (const entry of entries) {
        if (!typeScores[entry.type]) {
          typeScores[entry.type] = { score: 0, matched: [] };
        }

        typeScores[entry.type].score += entry.weight;
        typeScores[entry.type].matched.push(keyword);

        // Boost if keyword is in title (only for non-boost words)
        if (!entry.isBoost && hasWordBoundary(titleLower, keyword)) {
          typeScores[entry.type].score += entry.weight * 0.5;
        }
      }
    }
  }

  // Find best type
  let bestType: SignalType | null = null;
  let bestConfidence = 0;
  let bestTriggers: string[] = [];

  for (const [type, data] of Object.entries(typeScores)) {
    const confidence = Math.min(data.score / 3.0, 1.0);
    if (confidence > bestConfidence) {
      bestType = type as SignalType;
      bestConfidence = confidence;
      bestTriggers = data.matched;
    }
  }

  if (!bestType) return null;

  // Require at least 2 keyword matches — single keyword produces too many false positives
  if (bestTriggers.length < 2) return null;

  // Compute priority
  let priorityScore = BASE_WEIGHTS[bestType];
  const techMatch = detectedTech.find((t) => hasWordBoundary(textLower, t.toLowerCase()));
  if (techMatch) priorityScore += 1;
  if (relevanceScore > 0.7) priorityScore += 1;
  priorityScore = Math.min(priorityScore, 4);

  const priority: SignalPriority =
    priorityScore >= 4 ? "critical" : priorityScore === 3 ? "high" : priorityScore === 2 ? "medium" : "low";

  // Generate action
  const shortTitle = title.length > 60 ? title.substring(0, 60) + "..." : title;
  let action: string;
  if (bestType === "security_alert" && techMatch) {
    action = `Review ${shortTitle} - affects your ${techMatch} stack`;
  } else if (bestType === "breaking_change" && techMatch) {
    action = `Check migration path - ${techMatch} breaking change`;
  } else if (bestType === "tool_discovery" && techMatch) {
    action = `Evaluate for your ${techMatch} workflow: ${shortTitle}`;
  } else {
    action = `${PRIORITY_LABELS[bestType] || bestType}: ${shortTitle}`;
  }

  return {
    signalType: bestType,
    priority,
    confidence: Math.round(bestConfidence * 100) / 100,
    action,
    triggers: bestTriggers,
  };
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
        description: "Only include items from the last N hours. Default: 48",
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

// ============================================================================
// Execution
// ============================================================================

export function executeGetActionableSignals(
  db: FourDADatabase,
  params: GetActionableSignalsParams
): { signals: ClassifiedSignal[]; total: number; summary: Record<string, number> } {
  const limit = Math.max(1, Math.min(100, params.limit ?? 20));
  const sinceHours = Math.max(1, Math.min(168, params.since_hours ?? 48));

  // Get items from DB (reuse existing method with low min score to get more items)
  const items = db.getRelevantContent(0.1, undefined, 200, sinceHours);

  // Get user's detected tech for cross-referencing
  const context = db.getUserContext(true, false);
  const detectedTech = (context.ace?.detected_tech || []).map((t: { name: string }) => t.name);

  const signals: ClassifiedSignal[] = [];

  for (const item of items) {
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

  // Inject live vulnerability data as security_alert signals
  const liveIntel = getLiveIntelligence();
  if (liveIntel) {
    const vulnResult = liveIntel.getVulnerabilities();
    if (vulnResult && vulnResult.vulnerabilities.length > 0) {
      for (const vuln of vulnResult.vulnerabilities) {
        // Apply filters
        const vulnPriority: SignalPriority =
          vuln.severity === "critical" ? "critical" :
          vuln.severity === "high" ? "high" : "medium";

        if (params.priority_filter && vulnPriority !== params.priority_filter) continue;
        if (params.signal_type && params.signal_type !== "security_alert") continue;

        signals.push({
          id: -1,
          title: `${vuln.severity.toUpperCase()}: ${vuln.summary} (${vuln.package}@${vuln.currentVersion})`,
          url: vuln.references[0] || null,
          source_type: "osv_live",
          relevance_score: vuln.severity === "critical" ? 1.0 : vuln.severity === "high" ? 0.9 : 0.7,
          signal_type: "security_alert",
          signal_priority: vulnPriority,
          action: vuln.fixedVersion
            ? `Upgrade ${vuln.package} to ${vuln.fixedVersion}`
            : `Review ${vuln.package} — no fix version published`,
          triggers: [vuln.vulnId, ...vuln.aliases].filter(Boolean),
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
  };
}
