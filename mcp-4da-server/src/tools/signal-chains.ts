/**
 * signal_chains tool
 *
 * Get active signal chains - connected signals that form causal narratives over time.
 */

import type { FourDADatabase } from "../db.js";

export interface SignalChainsParams {
  resolution?: string;
  min_priority?: string;
}

export const signalChainsTool = {
  name: "signal_chains",
  description: `Get active signal chains - connected signals that form causal narratives over time. E.g., "CVE found Monday + your dep uses it Tuesday + patch released today = act now."`,
  inputSchema: {
    type: "object" as const,
    properties: {
      resolution: {
        type: "string",
        enum: ["open", "resolved", "expired", "all"],
        description: "Filter by chain resolution status. Default: open",
        default: "open",
      },
      min_priority: {
        type: "string",
        enum: ["critical", "high", "medium", "low"],
        description: "Minimum chain priority to include. Default: low",
        default: "low",
      },
    },
  },
};

export function executeSignalChains(
  db: FourDADatabase,
  params: SignalChainsParams,
) {
  const rawDb = db.getRawDb();
  const resolution = params.resolution || "open";

  let query = `SELECT id, event_type, subject, data, created_at
    FROM temporal_events
    WHERE event_type = 'signal_chain'`;

  if (resolution !== "all") {
    query += ` AND json_extract(data, '$.resolution') = '${resolution}'`;
  }
  query += " ORDER BY created_at DESC LIMIT 50";

  const rows = rawDb.prepare(query).all() as any[];

  const chains = rows.map((row: any) => {
    const data = JSON.parse(row.data);
    return {
      id: row.id,
      chain_name: data.chain_name || row.subject,
      links: data.links || [],
      overall_priority: data.overall_priority || "medium",
      resolution: data.resolution || "open",
      suggested_action: data.suggested_action || "",
      created_at: row.created_at,
    };
  });

  // Filter by priority
  const priorityOrder: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
  const minLevel = priorityOrder[params.min_priority || "low"] || 1;
  const filtered = chains.filter(
    (c) => (priorityOrder[c.overall_priority] || 0) >= minLevel,
  );

  return {
    chains: filtered,
    total: filtered.length,
    summary: `${filtered.length} signal chain${filtered.length !== 1 ? "s" : ""} (${resolution})`,
  };
}
