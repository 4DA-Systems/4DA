// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * agent_session_brief tool
 *
 * Tailored session startup context for AI agents.
 * Call at the START of every agent session to get caught up on:
 * - Active developer decisions
 * - Ecosystem changes (security, breaking, releases)
 * - Recent agent memories from all agents
 * - Latest briefing text
 */

import type { FourDADatabase } from "../db.js";

// ============================================================================
// Types
// ============================================================================

export interface AgentSessionBriefParams {
  agent_type?: string;
  since?: string; // ISO datetime, default 24h ago
  include_decisions?: boolean; // default true
  include_memories?: boolean; // default true
}

interface DecisionBriefRow {
  id: number;
  subject: string;
  decision: string;
  decision_type: string;
  confidence: number;
}

interface SourceItemBriefRow {
  id: number;
  title: string;
  source_type: string;
  created_at: string;
}

interface AgentMemoryBriefRow {
  id: number;
  agent_type: string;
  memory_type: string;
  subject: string;
  content: string;
  created_at: string;
}

interface BriefingRow {
  content: string;
  created_at: string;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const agentSessionBriefTool = {
  name: "agent_session_brief",
  description:
    "Tailored session startup context for AI agents. Returns active decisions, ecosystem changes, recent agent memories, and latest briefing. Call at START of every agent session.",
  inputSchema: {
    type: "object" as const,
    properties: {
      agent_type: {
        type: "string",
        description:
          "Agent identifier (e.g. claude_code, cursor). Filters memories to this agent if provided.",
      },
      since: {
        type: "string",
        description:
          "ISO datetime to look back from. Default: 24 hours ago.",
      },
      include_decisions: {
        type: "boolean",
        description: "Include active developer decisions. Default: true.",
      },
      include_memories: {
        type: "boolean",
        description: "Include recent agent memories. Default: true.",
      },
    },
  },
};

// ============================================================================
// Execute
// ============================================================================

export function executeAgentSessionBrief(
  db: FourDADatabase,
  params: AgentSessionBriefParams,
): object {
  const rawDb = db.getRawDb();
  const agentType = params.agent_type || "unknown";
  const since =
    params.since ||
    new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString();
  const includeDecisions = params.include_decisions !== false;
  const includeMemories = params.include_memories !== false;

  // 1. Active decisions
  let activeDecisions: object[] = [];
  if (includeDecisions) {
    try {
      const rows = rawDb
        .prepare(
          `SELECT id, subject, decision, decision_type, confidence
           FROM developer_decisions
           WHERE status = 'active'
           ORDER BY confidence DESC
           LIMIT 20`,
        )
        .all() as DecisionBriefRow[];

      activeDecisions = rows.map((r) => ({
        id: r.id,
        subject: r.subject,
        decision: r.decision,
        type: r.decision_type,
        confidence: r.confidence,
      }));
    } catch {
      // developer_decisions table may not exist yet
    }
  }

  // 2. Ecosystem changes (security, breaking, releases, updates)
  let ecosystemChanges: object[] = [];
  try {
    const rows = rawDb
      .prepare(
        `SELECT id, title, source_type, created_at
         FROM source_items
         WHERE created_at > ?
         AND (
           LOWER(title) LIKE '%security%'
           OR LOWER(title) LIKE '%breaking%'
           OR LOWER(title) LIKE '%release%'
           OR LOWER(title) LIKE '%update%'
         )
         ORDER BY created_at DESC
         LIMIT 20`,
      )
      .all(since) as SourceItemBriefRow[];

    ecosystemChanges = rows.map((r) => {
      const titleLower = r.title.toLowerCase();
      let changeType = "update";
      if (titleLower.includes("security")) changeType = "security";
      else if (titleLower.includes("breaking")) changeType = "breaking_change";
      else if (titleLower.includes("release")) changeType = "release";

      return {
        change_type: changeType,
        subject: r.title,
        summary: `[${r.source_type}] ${r.title}`,
      };
    });
  } catch {
    // source_items table may not exist yet
  }

  // 3. Recent agent memories
  let agentMemories: object[] = [];
  if (includeMemories) {
    try {
      let memorySql = `SELECT id, agent_type, memory_type, subject, content, created_at
                        FROM agent_memory
                        WHERE created_at > ?`;
      const memoryParams: (string | number)[] = [since];

      if (params.agent_type) {
        memorySql += ` AND agent_type = ?`;
        memoryParams.push(params.agent_type);
      }

      memorySql += ` ORDER BY created_at DESC LIMIT 30`;

      const rows = rawDb
        .prepare(memorySql)
        .all(...memoryParams) as AgentMemoryBriefRow[];

      agentMemories = rows.map((r) => ({
        id: r.id,
        agent_type: r.agent_type,
        memory_type: r.memory_type,
        subject: r.subject,
        content:
          r.content.length > 200
            ? r.content.substring(0, 200) + "..."
            : r.content,
        created_at: r.created_at,
      }));
    } catch {
      // agent_memory table may not exist yet
    }
  }

  // 4. Latest briefing
  let recentBriefing: string | null = null;
  try {
    const row = rawDb
      .prepare(
        `SELECT content, created_at FROM briefings ORDER BY created_at DESC LIMIT 1`,
      )
      .get() as BriefingRow | undefined;

    if (row) {
      recentBriefing = row.content;
    }
  } catch {
    // briefings table may not exist yet
  }

  // 5. Build summary
  const summaryParts: string[] = [];
  if (activeDecisions.length > 0) {
    summaryParts.push(`${activeDecisions.length} active decisions`);
  }
  if (ecosystemChanges.length > 0) {
    summaryParts.push(`${ecosystemChanges.length} ecosystem changes`);
  }
  if (agentMemories.length > 0) {
    summaryParts.push(`${agentMemories.length} recent memories`);
  }
  if (recentBriefing) {
    summaryParts.push("briefing available");
  }

  const summary =
    summaryParts.length > 0
      ? `Session brief for ${agentType}: ${summaryParts.join(", ")}`
      : `No recent activity found since ${since}`;

  return {
    generated_at: new Date().toISOString(),
    agent_type: agentType,
    active_decisions: activeDecisions,
    ecosystem_changes: ecosystemChanges,
    agent_memories: agentMemories,
    recent_briefing: recentBriefing,
    summary,
  };
}
