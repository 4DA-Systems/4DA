// SPDX-License-Identifier: Apache-2.0
/**
 * Keyword signal classifier for get_actionable_signals.
 *
 * Classifies a source item into a signal type with a priority when the
 * scoring pipeline has not stamped one, and normalises the pipeline's own
 * priority vocabulary when it has. Split out of get-actionable-signals.ts,
 * which owns the database read and the live-scan injection.
 */

// ============================================================================
// Signal Classification Types
// ============================================================================

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

interface SignalPattern {
  keywords: string[];
  boostWords: string[];
  weight: number;
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

export interface Classification {
  signalType: SignalType;
  priority: SignalPriority;
  confidence: number;
  action: string;
  triggers: string[];
}

export function classify(
  title: string,
  content: string,
  relevanceScore: number,
  detectedTech: string[]
): Classification | null {
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

  // Security alerts MUST mention something in the user's stack to be relevant.
  // A CVE for "Prometheus" or "PraisonAI" is noise if the user doesn't use them.
  // Only keep security alerts that explicitly name a technology the user has.
  if (bestType === "security_alert" && !techMatch) {
    return null;
  }
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
