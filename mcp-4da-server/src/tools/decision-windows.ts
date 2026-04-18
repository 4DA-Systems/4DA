// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * decision_windows tool
 *
 * View and manage decision windows — time-bounded opportunities
 * detected from content intelligence requiring attention.
 */
import type { FourDADatabase } from "../db.js";

export interface DecisionWindowsParams {
  action?: string;
  window_id?: number;
  outcome?: string;
}

interface WindowRow {
  id: number;
  title: string;
  description: string | null;
  urgency: number;
  source_item_id: number | null;
  status: string;
  opens_at: string;
  closes_at: string | null;
  acted_at: string | null;
  closed_at: string | null;
  outcome: string | null;
  created_at: string;
}

export const decisionWindowsTool = {
  name: "decision_windows",
  description:
    "View and manage decision windows — time-bounded opportunities detected from content intelligence. List open windows, mark as acted on, or close.",
  inputSchema: {
    type: "object" as const,
    properties: {
      action: {
        type: "string",
        enum: ["list", "act", "close"],
        description: "Action to perform. Default: list",
        default: "list",
      },
      window_id: {
        type: "number",
        description: "Window ID (required for 'act' or 'close' actions)",
      },
      outcome: {
        type: "string",
        description: "Outcome description when acting on a window",
      },
    },
  },
};

export function executeDecisionWindows(db: FourDADatabase, params: DecisionWindowsParams): object {
  const rawDb = db.getRawDb();
  const action = params.action ?? "list";

  if (action === "act") {
    if (!params.window_id) return { error: "window_id is required for 'act' action" };
    try {
      rawDb.prepare(
        `UPDATE decision_windows SET status = 'acted', acted_at = datetime('now'), outcome = ? WHERE id = ?`,
      ).run(params.outcome ?? null, params.window_id);
    } catch (err) {
      return { error: `Failed to act on window: ${err instanceof Error ? err.message : String(err)}` };
    }
  } else if (action === "close") {
    if (!params.window_id) return { error: "window_id is required for 'close' action" };
    try {
      rawDb.prepare(
        `UPDATE decision_windows SET status = 'closed', closed_at = datetime('now') WHERE id = ?`,
      ).run(params.window_id);
    } catch (err) {
      return { error: `Failed to close window: ${err instanceof Error ? err.message : String(err)}` };
    }
  }

  // Always return open windows after any action
  let openWindows: WindowRow[] = [];
  try {
    openWindows = rawDb.prepare(
      `SELECT id, title, description, urgency, source_item_id, status,
              opens_at, closes_at, acted_at, closed_at, outcome, created_at
       FROM decision_windows WHERE status = 'open'
       ORDER BY urgency DESC, created_at DESC`,
    ).all() as WindowRow[];
  } catch { /* Table may not exist yet */ }

  const actionMsg = action === "act" ? `Window ${params.window_id} marked as acted`
    : action === "close" ? `Window ${params.window_id} closed` : null;

  return {
    ...(actionMsg ? { action_result: actionMsg } : {}),
    open_windows: openWindows,
    count: openWindows.length,
    summary: openWindows.length === 0
      ? "No open decision windows. The system will surface time-sensitive opportunities as they are detected."
      : `${openWindows.length} open decision window${openWindows.length !== 1 ? "s" : ""} requiring attention`,
  };
}
