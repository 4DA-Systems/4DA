/**
 * autophagy_status tool
 *
 * Intelligence metabolism status — autophagy cycles, calibrations, anti-patterns.
 */
import type { FourDADatabase } from "../db.js";

export interface AutophagyStatusParams {
  limit?: number;
}

interface CycleRow {
  id: number;
  items_analyzed: number;
  calibrations_produced: number;
  anti_patterns_detected: number;
  duration_ms: number;
  created_at: string;
}

interface DigestRow {
  id: number;
  digest_type: string;
  content: string;
  confidence: number;
  created_at: string;
}

export const autophagyStatusTool = {
  name: "autophagy_status",
  description:
    "Get intelligence metabolism status — autophagy cycle history, calibration accuracy, anti-patterns detected, and scoring corrections applied",
  inputSchema: {
    type: "object" as const,
    properties: {
      limit: {
        type: "number",
        description: "Maximum autophagy cycles to return. Default: 5",
        default: 5,
      },
    },
  },
};

function safeAll<T>(rawDb: ReturnType<FourDADatabase["getRawDb"]>, sql: string, ...args: unknown[]): T[] {
  try { return rawDb.prepare(sql).all(...args) as T[]; }
  catch { return []; } // Table may not exist yet
}

export function executeAutophagyStatus(db: FourDADatabase, params: AutophagyStatusParams): object {
  const rawDb = db.getRawDb();
  const limit = params.limit ?? 5;

  const cycles = safeAll<CycleRow>(rawDb,
    `SELECT id, items_analyzed, calibrations_produced,
            anti_patterns_detected, duration_ms, created_at
     FROM autophagy_cycles ORDER BY created_at DESC LIMIT ?`, limit);

  const calibrations = safeAll<DigestRow>(rawDb,
    `SELECT id, digest_type, content, confidence, created_at
     FROM digested_intelligence
     WHERE digest_type = 'calibration' AND superseded_by IS NULL
     ORDER BY created_at DESC`);

  const antiPatterns = safeAll<DigestRow>(rawDb,
    `SELECT id, digest_type, content, confidence, created_at
     FROM digested_intelligence
     WHERE digest_type = 'anti_pattern' AND superseded_by IS NULL
     ORDER BY created_at DESC`);

  let totalCycles = 0;
  if (cycles.length > 0) {
    const row = rawDb.prepare("SELECT COUNT(*) as cnt FROM autophagy_cycles").get() as { cnt: number } | undefined;
    totalCycles = row?.cnt ?? 0;
  }

  return {
    cycles,
    active_calibrations: calibrations,
    active_anti_patterns: antiPatterns,
    totals: {
      cycles_run: totalCycles,
      active_calibrations: calibrations.length,
      active_anti_patterns: antiPatterns.length,
    },
    summary: totalCycles === 0
      ? "No autophagy cycles have run yet. The metabolism system will activate as content accumulates."
      : `${totalCycles} cycles completed — ${calibrations.length} active calibrations, ${antiPatterns.length} active anti-patterns`,
  };
}
