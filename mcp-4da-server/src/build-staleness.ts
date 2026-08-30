// SPDX-License-Identifier: Apache-2.0
/**
 * Build staleness — is the running `dist/` older than `src/`?
 *
 * `.mcp.json` runs `node ./mcp-4da-server/dist/index.js`, and nothing rebuilds
 * `dist/` when a merge lands. 2026-08-30: the live server was a dist built two
 * days before its src — three already-fixed defects (platform filtering,
 * knowledge-gap grading, feed hygiene) presented as live bugs and were nearly
 * re-fixed. The same merged-but-not-running disease as a stale binary, with
 * zero symptoms.
 *
 * This check only means something in a repo checkout: the published npm
 * package ships no `src/`, so it returns null there (unknown-and-irrelevant,
 * never a false alarm). In a checkout it compares the newest source mtime
 * against the newest dist mtime — a `git pull` touches exactly the changed
 * files, which is exactly the signal needed.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

export interface BuildStaleness {
  stale: boolean;
  /** Newest mtime under src/ (ms). */
  src_newest_ms: number;
  /** Newest mtime under dist/ (ms). */
  dist_newest_ms: number;
  /** Human-readable remedy, set when stale. */
  note: string | null;
}

/** Editable-source extensions that produce build output. */
const SRC_EXTENSIONS = new Set([".ts", ".json"]);

/** Skip clock-skew false alarms: src must be newer by more than this. */
const SKEW_TOLERANCE_MS = 2_000;

function newestMtimeMs(dir: string, filter: (name: string) => boolean): number {
  let newest = 0;
  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return newest;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name.startsWith(".")) continue;
      const sub = newestMtimeMs(full, filter);
      if (sub > newest) newest = sub;
    } else if (filter(entry.name)) {
      try {
        const m = fs.statSync(full).mtimeMs;
        if (m > newest) newest = m;
      } catch {
        // A file deleted mid-scan is not a staleness signal.
      }
    }
  }
  return newest;
}

let memo: BuildStaleness | null | undefined;

/** Reset the memo. Tests only. */
export function _resetBuildStalenessCache(): void {
  memo = undefined;
}

/**
 * Compare `src/` against `dist/` for the package this module runs from.
 *
 * Returns null when there is no `src/` beside the running `dist/` (the
 * published package) or no dist output at all — unknown, never "fresh".
 * Pass explicit directories in tests.
 */
export function checkBuildStaleness(
  distDir?: string,
  srcDir?: string,
): BuildStaleness | null {
  if (distDir === undefined && memo !== undefined) return memo;

  const here = distDir ?? path.dirname(fileURLToPath(import.meta.url));
  const src = srcDir ?? path.join(here, "..", "src");
  const result = compute(here, src);
  if (distDir === undefined) memo = result;
  return result;
}

function compute(distDir: string, srcDir: string): BuildStaleness | null {
  if (!fs.existsSync(srcDir)) return null;

  const srcNewest = newestMtimeMs(srcDir, (n) =>
    SRC_EXTENSIONS.has(path.extname(n)),
  );
  const distNewest = newestMtimeMs(distDir, (n) => n.endsWith(".js"));
  if (srcNewest === 0 || distNewest === 0) return null;

  const stale = srcNewest > distNewest + SKEW_TOLERANCE_MS;
  const behindMinutes = Math.round((srcNewest - distNewest) / 60_000);
  return {
    stale,
    src_newest_ms: srcNewest,
    dist_newest_ms: distNewest,
    note: stale
      ? `This server's dist/ build is ~${behindMinutes} min older than src/ — its behavior does not include the latest merged fixes. Rebuild with: pnpm --dir mcp-4da-server run build`
      : null,
  };
}
