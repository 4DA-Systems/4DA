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
import { executeGetActionableSignals } from "./get-actionable-signals.js";
import { getLiveIntelligence } from "../live-singleton.js";
import {
  rankRowsByRecall,
  createRelevanceScorer,
  type RankedRecall,
  type RecallField,
} from "./recall.js";
import { getEmbeddingConfig, semanticScores, type EmbeddingConfig } from "../embeddings.js";
import { decisionEmbedText } from "./decision-recall.js";
import { memoryEmbedText } from "./agent-memory.js";

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

interface WisdomDecisionRow {
  id: number;
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string;
  context_tags: string;
  updated_at: string;
}

interface WisdomMemoryRow {
  id: number;
  memory_type: string;
  subject: string;
  content: string;
  context_tags: string;
  created_at: string;
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
  /** How relevant_wisdom was retrieved: "hybrid" when an embedding provider is active. */
  wisdom_recall_mode: "hybrid" | "ranked_lexical";
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

/** Weighted fields for ranking decisions (mirrors the decision tools' weights). */
const WISDOM_DECISION_FIELDS: RecallField<WisdomDecisionRow>[] = [
  { name: "alternatives", weight: 5, value: (row) => row.alternatives_rejected },
  { name: "subject", weight: 4, value: (row) => row.subject },
  { name: "tags", weight: 3, value: (row) => row.context_tags },
  { name: "decision", weight: 2, value: (row) => row.decision },
  { name: "rationale", weight: 1, value: (row) => row.rationale },
];

/** Weighted fields for ranking memories. */
const WISDOM_MEMORY_FIELDS: RecallField<WisdomMemoryRow>[] = [
  { name: "subject", weight: 4, value: (row) => row.subject },
  { name: "tags", weight: 3, value: (row) => row.context_tags },
  { name: "content", weight: 2, value: (row) => row.content },
  { name: "type", weight: 1, value: (row) => row.memory_type },
];

const WISDOM_LIMIT = 6;
/** Blend weight for semantic cosine vs normalized lexical score in hybrid wisdom. */
const WISDOM_BLEND = 0.65;

function loadWisdomDecisions(rawDb: ReturnType<FourDADatabase["getRawDb"]>): WisdomDecisionRow[] {
  try {
    return rawDb
      .prepare(
        `SELECT id, subject, decision, rationale, alternatives_rejected, context_tags, updated_at
         FROM developer_decisions
         WHERE status = 'active'
         ORDER BY updated_at DESC
         LIMIT 300`,
      )
      .all() as WisdomDecisionRow[];
  } catch {
    return []; // Older DBs may not have decision memory yet.
  }
}

function loadWisdomMemories(rawDb: ReturnType<FourDADatabase["getRawDb"]>): WisdomMemoryRow[] {
  try {
    return rawDb
      .prepare(
        `SELECT id, memory_type, subject, content, context_tags, created_at
         FROM agent_memory
         WHERE (expires_at IS NULL OR expires_at > datetime('now'))
         ORDER BY created_at DESC
         LIMIT 300`,
      )
      .all() as WisdomMemoryRow[];
  } catch {
    return []; // Older DBs may not have agent memory yet.
  }
}

/** Map a ranked decision/memory row to the public WisdomEntry shape. */
function toWisdomEntry(item: {
  type: "decision" | "memory";
  row: WisdomDecisionRow | WisdomMemoryRow;
}): WisdomEntry {
  if (item.type === "decision") {
    const row = item.row as WisdomDecisionRow;
    return {
      type: "decision",
      subject: row.subject,
      detail: row.rationale ? `${row.decision} Rationale: ${row.rationale}` : row.decision,
    };
  }
  const row = item.row as WisdomMemoryRow;
  return {
    type: `memory:${row.memory_type}`,
    subject: row.subject,
    detail: row.content,
  };
}

/**
 * Lexical wisdom retrieval (the default, provider-free path): rank decisions and
 * memories independently, merge by score, take the top entries.
 */
function getRelevantWisdom(db: FourDADatabase, task: string, files: string[]): WisdomEntry[] {
  const rawDb = db.getRawDb();
  const query = [task, ...files].join(" ");

  const entries: Array<RankedRecall<WisdomDecisionRow | WisdomMemoryRow> & {
    type: "decision" | "memory";
  }> = [
    ...rankRowsByRecall(loadWisdomDecisions(rawDb), query, WISDOM_DECISION_FIELDS, WISDOM_LIMIT).map(
      (item) => ({ ...item, type: "decision" as const }),
    ),
    ...rankRowsByRecall(loadWisdomMemories(rawDb), query, WISDOM_MEMORY_FIELDS, WISDOM_LIMIT).map(
      (item) => ({ ...item, type: "memory" as const }),
    ),
  ];

  return entries
    .sort((a, b) => b.score - a.score)
    .slice(0, WISDOM_LIMIT)
    .map(toWisdomEntry);
}

/**
 * Hybrid wisdom retrieval: blends alias-aware lexical scoring with embedding
 * cosine similarity (per table, since each is normalized independently), so a
 * paraphrased prior decision or memory surfaces in the briefing even with no
 * shared words. Falls back to pure lexical (and reports it) when nothing embeds.
 */
async function getRelevantWisdomHybrid(
  db: FourDADatabase,
  task: string,
  files: string[],
  config: EmbeddingConfig,
): Promise<{ wisdom: WisdomEntry[]; recall_mode: "hybrid" | "ranked_lexical" }> {
  const rawDb = db.getRawDb();
  const query = [task, ...files].join(" ");
  const decisions = loadWisdomDecisions(rawDb);
  const memories = loadWisdomMemories(rawDb);

  // Lexical baselines over ALL rows, per table, for normalization + fallback.
  const decLex = rankRowsByRecall(decisions, query, WISDOM_DECISION_FIELDS, decisions.length);
  const memLex = rankRowsByRecall(memories, query, WISDOM_MEMORY_FIELDS, memories.length);
  const decLexById = new Map<number, number>(decLex.map((i) => [i.row.id, i.score]));
  const memLexById = new Map<number, number>(memLex.map((i) => [i.row.id, i.score]));
  const decMax = decLex.length ? decLex[0].score : 0;
  const memMax = memLex.length ? memLex[0].score : 0;

  const decSem = decisions.length
    ? await semanticScores(
        db,
        "developer_decisions",
        query,
        decisions.map((d) => ({ id: d.id, text: decisionEmbedText(d) })),
        config,
      )
    : null;
  const memSem = memories.length
    ? await semanticScores(
        db,
        "agent_memory",
        query,
        memories.map((m) => ({ id: m.id, text: memoryEmbedText(m) })),
        config,
      )
    : null;

  const anySemantic = (decSem?.embeddedCount ?? 0) > 0 || (memSem?.embeddedCount ?? 0) > 0;
  if (!anySemantic) {
    // Provider unreachable / nothing embedded -> behave exactly like lexical.
    const entries = [
      ...decLex.slice(0, WISDOM_LIMIT).map((i) => ({ ...i, type: "decision" as const })),
      ...memLex.slice(0, WISDOM_LIMIT).map((i) => ({ ...i, type: "memory" as const })),
    ];
    return {
      wisdom: entries.sort((a, b) => b.score - a.score).slice(0, WISDOM_LIMIT).map(toWisdomEntry),
      recall_mode: "ranked_lexical",
    };
  }

  const scored: Array<{
    type: "decision" | "memory";
    row: WisdomDecisionRow | WisdomMemoryRow;
    score: number;
  }> = [];

  for (const d of decisions) {
    const semantic = Math.max(0, decSem?.semanticById.get(d.id) ?? 0);
    const lexNorm = decMax > 0 ? (decLexById.get(d.id) ?? 0) / decMax : 0;
    const score = WISDOM_BLEND * semantic + (1 - WISDOM_BLEND) * lexNorm;
    if (score > 0) scored.push({ type: "decision", row: d, score });
  }
  for (const m of memories) {
    const semantic = Math.max(0, memSem?.semanticById.get(m.id) ?? 0);
    const lexNorm = memMax > 0 ? (memLexById.get(m.id) ?? 0) / memMax : 0;
    const score = WISDOM_BLEND * semantic + (1 - WISDOM_BLEND) * lexNorm;
    if (score > 0) scored.push({ type: "memory", row: m, score });
  }

  return {
    wisdom: scored.sort((a, b) => b.score - a.score).slice(0, WISDOM_LIMIT).map(toWisdomEntry),
    recall_mode: "hybrid",
  };
}

// ============================================================================
// Execute
// ============================================================================

export function executeWhatShouldIKnow(
  db: FourDADatabase,
  params: WhatShouldIKnowParams,
): WhatShouldIKnowResult | Promise<WhatShouldIKnowResult> {
  const task = params.task;
  const files = params.files || [];
  // One alias-aware scorer for the whole briefing, so advisories, decision
  // windows and ecosystem news share the SAME relevance model as relevant_wisdom
  // (an "auth" task now matches a "jwt"/"oauth" advisory; substring matching did not).
  const relevance = createRelevanceScorer([task, ...files].join(" "));

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
    const hnIntel = getLiveIntelligence();
    if (hnIntel) {
      const headlines = hnIntel.getHeadlines();
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

  // ── 4. Assembly ────────────────────────────────────────────────────────
  // Wisdom retrieval is lexical by default and hybrid (async) when a provider is
  // configured, so delegation + summary are deferred into finalize() and the
  // function returns synchronously OR a Promise accordingly.
  const finalize = (
    relevantWisdom: WisdomEntry[],
    wisdomMode: "hybrid" | "ranked_lexical",
  ): WhatShouldIKnowResult => {
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
      wisdom_recall_mode: wisdomMode,
    };
  };

  // No provider -> synchronous lexical wisdom (classic behaviour preserved).
  const embedConfig = getEmbeddingConfig();
  if (!embedConfig) {
    return finalize(getRelevantWisdom(db, task, files), "ranked_lexical");
  }

  // Provider configured -> hybrid wisdom (returns a Promise; dispatch awaits).
  return getRelevantWisdomHybrid(db, task, files, embedConfig).then(
    ({ wisdom, recall_mode }) => finalize(wisdom, recall_mode),
  );
}
