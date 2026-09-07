// SPDX-License-Identifier: Apache-2.0
/**
 * Wisdom retrieval for the what_should_i_know briefing.
 *
 * Ranks the developer's recorded decisions and agent memories against the
 * task — lexically by default, blended with embedding similarity when a
 * provider is configured. Split out of what-should-i-know.ts, which is the
 * briefing's assembly and delegation logic.
 */

import type { FourDADatabase } from "../db.js";
import { rankRowsByRecall, type RankedRecall, type RecallField } from "./recall.js";
import { semanticScores, type EmbeddingConfig } from "../embeddings.js";
import { decisionEmbedText } from "./decision-recall.js";
import { memoryEmbedText } from "./agent-memory.js";

export interface WisdomEntry {
  type: string;
  subject: string;
  detail: string;
}

export type WisdomRecallMode = "hybrid" | "ranked_lexical";

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
export function getRelevantWisdom(db: FourDADatabase, task: string, files: string[]): WisdomEntry[] {
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
export async function getRelevantWisdomHybrid(
  db: FourDADatabase,
  task: string,
  files: string[],
  config: EmbeddingConfig,
): Promise<{ wisdom: WisdomEntry[]; recall_mode: WisdomRecallMode }> {
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
