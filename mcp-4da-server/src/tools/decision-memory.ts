/**
 * decision_memory tool
 *
 * Manage developer decisions - record, list, check alignment, update, and supersede.
 * Decisions persist and inform signal classification, tech radar, and agent context.
 */

import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir, platform } from "node:os";
import { join } from "node:path";
import type { FourDADatabase } from "../db.js";

// ============================================================================
// Types
// ============================================================================

export interface DecisionMemoryParams {
  action: "record" | "list" | "check_alignment" | "update" | "supersede";
  // For record
  decision_type?:
    | "tech_choice"
    | "architecture"
    | "workflow"
    | "pattern"
    | "dependency";
  subject?: string;
  decision?: string;
  rationale?: string;
  alternatives_rejected?: string[];
  context_tags?: string[];
  confidence?: number;
  // For list
  filter_type?: string;
  filter_status?: string;
  limit?: number;
  // For check_alignment
  technology?: string;
  pattern?: string;
  // For update
  id?: number;
  new_decision?: string;
  new_rationale?: string;
  new_status?: "active" | "superseded" | "reconsidering";
  new_confidence?: number;
  // For supersede
  old_id?: number;
  new_id?: number;
}

interface DecisionRow {
  id: number;
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string;
  context_tags: string;
  confidence: number;
  status: string;
  superseded_by: number | null;
  created_at: string;
  updated_at: string;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const decisionMemoryTool = {
  name: "decision_memory",
  description:
    "Manage developer decisions. Actions: record (create new decision), list (query decisions), check_alignment (check if tech/pattern conflicts with decisions), update (modify a decision), supersede (replace old decision with new one).",
  inputSchema: {
    type: "object" as const,
    properties: {
      action: {
        type: "string",
        enum: ["record", "list", "check_alignment", "update", "supersede"],
        description: "Action to perform",
      },
      decision_type: {
        type: "string",
        enum: [
          "tech_choice",
          "architecture",
          "workflow",
          "pattern",
          "dependency",
        ],
        description: "Type of decision (for record)",
      },
      subject: {
        type: "string",
        description:
          "Subject of the decision (for record, check_alignment)",
      },
      decision: {
        type: "string",
        description: "The decision text (for record)",
      },
      rationale: {
        type: "string",
        description: "Why this decision was made (for record)",
      },
      alternatives_rejected: {
        type: "array",
        items: { type: "string" },
        description:
          "Alternatives that were considered and rejected (for record)",
      },
      context_tags: {
        type: "array",
        items: { type: "string" },
        description: "Tags for categorization (for record)",
      },
      confidence: {
        type: "number",
        description: "Confidence level 0-1 (for record)",
      },
      filter_type: {
        type: "string",
        description: "Filter by decision type (for list)",
      },
      filter_status: {
        type: "string",
        description:
          "Filter by status: active, superseded, reconsidering (for list)",
      },
      limit: {
        type: "number",
        description: "Max results to return (for list)",
      },
      technology: {
        type: "string",
        description: "Technology to check alignment for",
      },
      pattern: {
        type: "string",
        description: "Pattern to check alignment for",
      },
      id: {
        type: "number",
        description: "Decision ID (for update)",
      },
      new_decision: {
        type: "string",
        description: "Updated decision text (for update)",
      },
      new_rationale: {
        type: "string",
        description: "Updated rationale (for update)",
      },
      new_status: {
        type: "string",
        enum: ["active", "superseded", "reconsidering"],
        description: "Updated status (for update)",
      },
      new_confidence: {
        type: "number",
        description: "Updated confidence (for update)",
      },
      old_id: {
        type: "number",
        description: "ID of decision to supersede (for supersede)",
      },
      new_id: {
        type: "number",
        description:
          "ID of new decision that replaces old (for supersede)",
      },
    },
    required: ["action"],
  },
};

// ============================================================================
// Helpers
// ============================================================================

function parseDecisionRow(row: DecisionRow) {
  return {
    ...row,
    alternatives_rejected: JSON.parse(
      row.alternatives_rejected || "[]"
    ) as string[],
    context_tags: JSON.parse(row.context_tags || "[]") as string[],
  };
}

// ============================================================================
// Execute
// ============================================================================

export function executeDecisionMemory(
  db: FourDADatabase,
  params: DecisionMemoryParams
): object {
  const rawDb = db.getRawDb();

  switch (params.action) {
    case "record": {
      if (!params.subject || !params.decision) {
        return {
          error: "subject and decision are required for record action",
        };
      }

      const stmt = rawDb.prepare(
        `INSERT INTO developer_decisions
           (decision_type, subject, decision, rationale,
            alternatives_rejected, context_tags, confidence, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active')`
      );

      const result = stmt.run(
        params.decision_type || "tech_choice",
        params.subject,
        params.decision,
        params.rationale || null,
        JSON.stringify(params.alternatives_rejected || []),
        JSON.stringify(params.context_tags || []),
        params.confidence ?? 0.8
      );

      // Bridge to AWE Wisdom Graph: forward decision for wisdom tracking.
      // Non-blocking — AWE sync is best-effort, doesn't affect 4DA recording.
      try {
        const aweBin = findAweBinary();
        if (aweBin) {
          const query = `${params.subject}: ${params.decision}`;
          const domain = mapDecisionTypeToDomain(params.decision_type || "tech_choice");
          const child = execFile(aweBin, [
            "transmute", query,
            "--domain", domain,
            "--json", "--no-persist",
          ], { timeout: 30_000 }, () => {
            // Callback required — ignore result (fire-and-forget)
          });
          child.unref();
        }
      } catch {
        // AWE bridge is optional
      }

      return {
        success: true,
        id: result.lastInsertRowid,
        message: `Decision recorded: ${params.subject}`,
      };
    }

    case "list": {
      let sql = `SELECT id, decision_type, subject, decision, rationale,
                        alternatives_rejected, context_tags, confidence,
                        status, superseded_by, created_at, updated_at
                 FROM developer_decisions WHERE 1=1`;
      const sqlParams: (string | number)[] = [];

      if (params.filter_type) {
        sql += ` AND decision_type = ?`;
        sqlParams.push(params.filter_type);
      }
      if (params.filter_status) {
        sql += ` AND status = ?`;
        sqlParams.push(params.filter_status);
      }

      sql += ` ORDER BY updated_at DESC LIMIT ?`;
      sqlParams.push(params.limit || 20);

      const rows = rawDb
        .prepare(sql)
        .all(...sqlParams) as DecisionRow[];

      return {
        decisions: rows.map(parseDecisionRow),
        count: rows.length,
      };
    }

    case "check_alignment": {
      if (!params.technology) {
        return {
          error: "technology is required for check_alignment action",
        };
      }

      const search = `%${params.technology.toLowerCase()}%`;
      const rows = rawDb
        .prepare(
          `SELECT id, decision_type, subject, decision, rationale,
                  alternatives_rejected, context_tags, confidence,
                  status, created_at
           FROM developer_decisions
           WHERE status = 'active'
             AND (LOWER(subject) LIKE ?
               OR LOWER(context_tags) LIKE ?
               OR LOWER(alternatives_rejected) LIKE ?)`
        )
        .all(search, search, search) as DecisionRow[];

      const conflicts: {
        decision_id: number;
        subject: string;
        reason: string;
      }[] = [];
      const relevant: ReturnType<typeof parseDecisionRow>[] = [];

      for (const row of rows) {
        const alts: string[] = JSON.parse(
          row.alternatives_rejected || "[]"
        );
        const isRejected = alts.some((alt) =>
          alt.toLowerCase().includes(params.technology!.toLowerCase())
        );

        if (isRejected) {
          conflicts.push({
            decision_id: row.id,
            subject: row.subject,
            reason: `'${params.technology}' was rejected in favor of '${row.decision}' (rationale: ${row.rationale || "none"})`,
          });
        }

        relevant.push(parseDecisionRow(row));
      }

      return {
        aligned: conflicts.length === 0,
        relevant_decisions: relevant,
        conflicts,
        confidence:
          relevant.length > 0
            ? Math.max(...relevant.map((r) => r.confidence))
            : 0.5,
      };
    }

    case "update": {
      if (!params.id) {
        return { error: "id is required for update action" };
      }

      const sets: string[] = [];
      const values: (string | number)[] = [];

      if (params.new_decision) {
        sets.push("decision = ?");
        values.push(params.new_decision);
      }
      if (params.new_rationale) {
        sets.push("rationale = ?");
        values.push(params.new_rationale);
      }
      if (params.new_status) {
        sets.push("status = ?");
        values.push(params.new_status);
      }
      if (params.new_confidence !== undefined) {
        sets.push("confidence = ?");
        values.push(params.new_confidence);
      }

      if (sets.length === 0) {
        return { error: "No fields to update" };
      }

      sets.push("updated_at = datetime('now')");
      values.push(params.id);

      rawDb
        .prepare(
          `UPDATE developer_decisions SET ${sets.join(", ")} WHERE id = ?`
        )
        .run(...values);

      return {
        success: true,
        message: `Decision ${params.id} updated`,
      };
    }

    case "supersede": {
      if (!params.old_id || !params.new_id) {
        return {
          error: "old_id and new_id are required for supersede action",
        };
      }

      rawDb
        .prepare(
          `UPDATE developer_decisions
           SET status = 'superseded',
               superseded_by = ?,
               updated_at = datetime('now')
           WHERE id = ?`
        )
        .run(params.new_id, params.old_id);

      return {
        success: true,
        message: `Decision ${params.old_id} superseded by ${params.new_id}`,
      };
    }

    default:
      return { error: `Unknown action: ${params.action}` };
  }
}

// ============================================================================
// AWE Bridge Helpers
// ============================================================================

/** Map 4DA decision types to AWE domains. */
function mapDecisionTypeToDomain(type: string): string {
  switch (type) {
    case "architecture":
    case "pattern":
      return "software-engineering";
    case "dependency":
      return "software-engineering";
    case "workflow":
      return "workflow";
    default:
      return "software-engineering";
  }
}

/** Find AWE binary — checks env var, platform defaults, then PATH fallback. */
function findAweBinary(): string | null {
  // 1. Explicit env var takes priority
  const envPath = process.env.FOURDA_AWE_PATH || process.env.AWE_BIN;
  if (envPath && existsSync(envPath)) return envPath;

  // 2. Platform-specific default install locations
  const home = homedir();
  const os = platform();
  const candidates: string[] = [];

  if (os === "win32") {
    const appData = process.env.APPDATA || join(home, "AppData", "Roaming");
    candidates.push(join(appData, "awe", "awe.exe"));
  } else if (os === "darwin") {
    candidates.push(join(home, "Library", "Application Support", "awe", "awe"));
  } else {
    // Linux and other Unix
    candidates.push(join(home, ".local", "share", "awe", "awe"));
  }

  for (const p of candidates) {
    if (existsSync(p)) return p;
  }

  // 3. Fall back to bare command name (relies on PATH)
  return "awe";
}
