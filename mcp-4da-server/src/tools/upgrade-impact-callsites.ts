// SPDX-License-Identifier: Apache-2.0
/**
 * Call-site scan for `upgrade_impact`: which files in THIS project use the
 * package, and which of its symbols they name.
 *
 * The symbols are what make a changelog actionable: "6 breaking changes" is
 * noise until one of them says `InitOptions` and the project imports
 * `InitOptions`. The scan is local-only (nothing leaves the machine) and
 * bounded, because it runs inside a tool call: at most 6000 candidate files,
 * none over 1 MB, and the usual generated / vendored directories skipped.
 * Symlinks are not followed, so a link cycle cannot stall the walk.
 *
 * It is a lexical scan, not a type-checker: an import inside a comment
 * counts, and a re-exported symbol used through a local barrel does not.
 * Both errors are tolerable for prioritising a changelog — the entries are
 * all still listed; this only decides which come first.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { IgnoreRules } from "../gitignore.js";
import type { UpgradeEcosystem } from "../live/upgrade-sources.js";

const SKIP_DIRS = new Set([
  "node_modules", "target", ".git", "dist", "build", "out", ".next",
  "coverage", "vendor", ".claude", ".venv",
]);
const NPM_EXTS = new Set([".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts", ".vue", ".svelte"]);
const RUST_EXTS = new Set([".rs"]);
const MAX_FILES = 6000;
const MAX_FILE_BYTES = 1024 * 1024;
const MAX_REPORTED_FILES = 50;
const MAX_SYMBOLS = 100;

export interface CallSiteReport {
  total_files: number;
  files: Array<{ path: string; matches: number }>;
  symbols_used: string[];
  /** Present when the walk stopped at the file cap: counts are then lower bounds. */
  truncated?: string;
}

function escapeRegex(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const IDENT = /^[A-Za-z_$][\w$]*$/;

/** Names from an import clause: `Def`, `* as ns`, `{ a, b as c, type D }`. Imported names, not local aliases. */
function parseImportClause(clause: string, into: Set<string>): void {
  const braces = /\{([^}]*)\}/.exec(clause);
  if (braces) {
    for (const part of braces[1].split(",")) {
      const name = part.trim().replace(/^type\s+/, "").split(/\s+as\s+/)[0].trim();
      if (IDENT.test(name) && name !== "default") into.add(name);
    }
  }
  const outside = clause.replace(/\{[^}]*\}/g, "").replace(/^\s*type\s+/, "");
  const ns = /\*\s+as\s+([A-Za-z_$][\w$]*)/.exec(outside);
  if (ns) into.add(ns[1]);
  const def = /^\s*([A-Za-z_$][\w$]*)\s*(?:,|$)/.exec(outside);
  if (def) into.add(def[1]);
}

/** Count npm usages of `pkg` (or `pkg/sub`) in one source file and collect imported names. */
export function scanNpmSource(source: string, pkg: string, symbols: Set<string>): number {
  const spec = `${escapeRegex(pkg)}(?:/[^'"\`\\s]*)?`;
  let matches = 0;
  const fromRe = new RegExp(String.raw`\b(?:import|export)\s+([^;'"\`]*?)\s*from\s*['"]${spec}['"]`, "g");
  for (const m of source.matchAll(fromRe)) {
    matches++;
    if (!/^\s*\*\s*$/.test(m[1])) parseImportClause(m[1], symbols);
  }
  const bareRe = new RegExp(String.raw`\bimport\s*['"]${spec}['"]`, "g");
  matches += [...source.matchAll(bareRe)].length;
  const dynRe = new RegExp(String.raw`\bimport\s*\(\s*['"]${spec}['"]\s*\)`, "g");
  matches += [...source.matchAll(dynRe)].length;
  const reqRe = new RegExp(String.raw`(?:(?:const|let|var)\s+(\{[^}]*\}|[A-Za-z_$][\w$]*)\s*=\s*)?\brequire\s*\(\s*['"]${spec}['"]\s*\)`, "g");
  for (const m of source.matchAll(reqRe)) {
    matches++;
    const binding = m[1];
    if (!binding) continue;
    if (binding.startsWith("{")) {
      for (const part of binding.slice(1, -1).split(",")) {
        const name = part.split(":")[0].trim();
        if (IDENT.test(name)) symbols.add(name);
      }
    } else symbols.add(binding);
  }
  return matches;
}

/** Leaf names of a Rust `use` tree: `{A, b::{C, D as E}, self, *}` -> A, C, D. */
function useTreeLeaves(tree: string, into: Set<string>): void {
  const flat = tree.replace(/[{}]/g, ",");
  for (const part of flat.split(",")) {
    const pathPart = part.trim().split(/\s+as\s+/)[0].trim();
    if (pathPart.endsWith("::")) continue; // `models::{…}` — the group's members are the leaves
    const leaf = pathPart.split("::").filter(Boolean).pop();
    if (leaf && /^[A-Za-z_]\w*$/.test(leaf) && leaf !== "self" && leaf !== "super" && leaf !== "crate") {
      into.add(leaf);
    }
  }
}

/** Count Rust usages of a crate (`-` read as `_`) in one file and collect the item names it reaches. */
export function scanRustSource(source: string, crateName: string, symbols: Set<string>): number {
  const ident = escapeRegex(crateName.replace(/-/g, "_"));
  let matches = 0;
  const useRe = new RegExp(String.raw`\buse\s+(?:::)?${ident}::([^;]+);`, "g");
  const withoutUses = source.replace(useRe, (_all, tree: string) => {
    matches++;
    useTreeLeaves(tree, symbols);
    return "";
  });
  const externRe = new RegExp(String.raw`\bextern\s+crate\s+${ident}\b`, "g");
  matches += [...withoutUses.matchAll(externRe)].length;
  // Qualified paths outside `use` lines; `crate::x::` / `self::x::` are local modules, hence the lookbehind.
  const pathRe = new RegExp(String.raw`(?<![\w:])${ident}::((?:[A-Za-z_]\w*::)*[A-Za-z_]\w*)`, "g");
  for (const m of withoutUses.matchAll(pathRe)) {
    matches++;
    for (const seg of m[1].split("::")) symbols.add(seg);
  }
  return matches;
}

/** Source files under `root`, skipping installed code, build output, hidden and .gitignored directories. */
function listSources(root: string, exts: Set<string>, budget: { left: number }): string[] {
  const files: string[] = [];
  const ignore = new IgnoreRules(root);
  const stack: Array<[string, string]> = [[root, ""]];
  while (stack.length > 0 && budget.left > 0) {
    const [dir, rel] = stack.pop() as [string, string];
    ignore.load(rel);
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue;
      const childRel = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (SKIP_DIRS.has(entry.name) || entry.name.startsWith(".") || ignore.ignores(childRel, true)) continue;
        stack.push([path.join(dir, entry.name), childRel]);
      } else if (entry.isFile() && exts.has(path.extname(entry.name).toLowerCase())) {
        if (budget.left-- <= 0) break;
        files.push(path.join(dir, entry.name));
      }
    }
  }
  return files;
}

/**
 * Bounded scan of `root` for usages of `pkg`: concurrent reads under a time
 * budget. Measured 2026-10-02 on 4DA's tree (856 TS/JS, 721 Rust files on a
 * nearly full disk): one-at-a-time synchronous reads took 20-26 s cold, so the
 * reads now overlap, a file is regex-scanned only when it contains the name at
 * all, and a scan that runs out of time says its counts are lower bounds.
 */
export async function scanCallSites(
  root: string,
  pkg: string,
  ecosystem: UpgradeEcosystem,
  opts: { timeBudgetMs?: number; concurrency?: number } = {},
): Promise<CallSiteReport> {
  const exts = ecosystem === "npm" ? NPM_EXTS : RUST_EXTS;
  const budget = { left: MAX_FILES };
  const symbols = new Set<string>();
  const hits: Array<{ path: string; matches: number }> = [];
  const needle = ecosystem === "npm" ? pkg : pkg.replace(/-/g, "_");
  const deadline = Date.now() + (opts.timeBudgetMs ?? CALLSITE_TIME_BUDGET_MS);
  const files = listSources(root, exts, budget);

  let next = 0;
  let timedOut = false;
  const worker = async () => {
    while (next < files.length) {
      if (Date.now() > deadline) {
        timedOut = true;
        return;
      }
      const file = files[next++];
      let source: string;
      try {
        const stat = await fs.promises.stat(file);
        if (stat.size > MAX_FILE_BYTES) continue;
        source = await fs.promises.readFile(file, "utf8");
      } catch {
        continue;
      }
      if (!source.includes(needle)) continue;
      const matches =
        ecosystem === "npm" ? scanNpmSource(source, pkg, symbols) : scanRustSource(source, pkg, symbols);
      if (matches > 0) hits.push({ path: path.relative(root, file).split(path.sep).join("/"), matches });
    }
  };
  await Promise.all(Array.from({ length: Math.min(opts.concurrency ?? 32, files.length) }, worker));

  hits.sort((a, b) => b.matches - a.matches || a.path.localeCompare(b.path));
  const report: CallSiteReport = {
    total_files: hits.length,
    files: hits.slice(0, MAX_REPORTED_FILES),
    symbols_used: [...symbols].sort().slice(0, MAX_SYMBOLS),
  };
  if (budget.left <= 0) report.truncated = `stopped after ${MAX_FILES} source files; counts are lower bounds`;
  else if (timedOut) report.truncated = `stopped after ${Math.round((opts.timeBudgetMs ?? CALLSITE_TIME_BUDGET_MS) / 1000)} s with ${files.length - next} of ${files.length} files unread; counts are lower bounds`;
  return report;
}

const CALLSITE_TIME_BUDGET_MS = 10_000;

/**
 * Symbols (≥ 3 chars) that `text` names at a word boundary, case-sensitively.
 * Shorter names (`fs`, `Ok`) would match ordinary prose, so they never flag.
 * A plain lowercase word (`new`, `params`, `default` — Rust paths and
 * CommonJS bindings yield plenty) counts only when the entry marks it as
 * code: in backticks, called `name(`, or reached through `::` / `.`.
 * Measured on the first live run: `TextEmbedding::new` put `new` in the
 * symbol set, which would have flagged every "Add new model" entry.
 */
export function matchSymbols(text: string, symbols: string[]): string[] {
  return symbols.filter((s) => {
    if (s.length < 3) return false;
    const e = escapeRegex(s);
    const pattern = /^[a-z]+$/.test(s)
      ? String.raw`\x60${e}(?:\(\))?\x60|(?<![\w$])${e}\(|(?:::|\.)${e}(?![\w$])|(?<![\w$])${e}::`
      : String.raw`(?<![\w$])${e}(?![\w$])`;
    return new RegExp(pattern).test(text);
  });
}
