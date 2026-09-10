// SPDX-License-Identifier: Apache-2.0
/**
 * Marker files the Rust engine leaves beside `4da.db` when it cannot record
 * an event in the database itself. `data_freshness` reads them so a
 * DB-backed answer can say what happened to the data it was read from.
 *
 * Read-only by design: the app owns each marker's lifecycle. The engine
 * clears `.engine-blocked` the moment a cycle opens the database again; the
 * desktop app shows `.db-recovered` once and deletes it. Nothing here deletes
 * either one, so every MCP answer keeps saying it until the app has.
 */

import * as fs from "node:fs";
import * as path from "node:path";

function markerPath(dbFile: string, name: string): string | null {
  // An in-memory database has no directory; do not read a marker out of the cwd.
  if (!dbFile || dbFile === ":memory:") return null;
  return path.join(path.dirname(dbFile), name);
}

function readJsonMarker(dbFile: string, name: string): Record<string, unknown> | null {
  const file = markerPath(dbFile, name);
  if (!file) return null;
  try {
    const parsed: unknown = JSON.parse(fs.readFileSync(file, "utf-8"));
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

export interface EngineBlockMarker {
  at: string;
  error: string;
}

/**
 * `data/.engine-blocked`: written by the Rust engine (`engine_block.rs`) when
 * a scheduled refresh is refused by a newer database schema.
 */
export function readEngineBlockMarker(dbFile: string): EngineBlockMarker | null {
  const v = readJsonMarker(dbFile, ".engine-blocked");
  if (v && typeof v.at === "string" && typeof v.error === "string") {
    return { at: v.at, error: v.error };
  }
  return null;
}

/** The kinds the engine writes today. A newer engine's unknown kind is still surfaced. */
export type DbRecoveryKind = "restored_from_backup" | "quarantined_no_backup" | "recovery_failed";

export interface DbRecoveredMarker {
  at: string;
  kind: DbRecoveryKind | string;
  detail: string;
}

/**
 * `data/.db-recovered`: written by the headless refresh engine when it
 * restored the database from a backup or quarantined it.
 * `{"at": "<ISO UTC>", "kind": "...", "detail": "<path or reason>"}`.
 */
export function readDbRecoveredMarker(dbFile: string): DbRecoveredMarker | null {
  const v = readJsonMarker(dbFile, ".db-recovered");
  if (!v) return null;
  const { at, kind, detail } = v;
  if (typeof at !== "string" || typeof kind !== "string" || typeof detail !== "string") return null;
  if (!at.trim() || !kind.trim()) return null;
  return { at, kind, detail };
}

/** The sentence `data_freshness.note` carries for a recovery marker. */
export function dbRecoveryNote(marker: DbRecoveredMarker): string {
  const { at, kind, detail } = marker;
  switch (kind) {
    case "restored_from_backup":
      return `The database was restored from a backup by the background refresh at ${at} — results may be incomplete or empty; the preserved file is ${detail}.`;
    case "quarantined_no_backup":
      return `The database was replaced with a fresh empty database by the background refresh at ${at} — results may be incomplete or empty; the preserved file is ${detail}.`;
    case "recovery_failed":
      return `The background refresh failed to recover the database at ${at} — results may be incomplete or empty; ${detail}.`;
    default:
      return `The background refresh recorded a database recovery (${kind}) at ${at} — results may be incomplete or empty; ${detail}.`;
  }
}
