// SPDX-License-Identifier: Apache-2.0
/**
 * Every independently-locked project under a root, for standalone scanning.
 *
 * A repository root is often not where its lockfiles live: 4DA keeps
 * `src-tauri/Cargo.lock`, `relay/Cargo.lock`, `site/pnpm-lock.yaml` beside a
 * root `pnpm-lock.yaml`. Standalone mode used to read the root directory only,
 * so `npx @4da/mcp-server` started at the repo root never scanned the Rust
 * code at all (measured 2026-10-02: 0 of src-tauri's 791 crates). The desktop
 * app does not have this problem — it records each dependency's own manifest
 * directory — so this is the standalone counterpart of that inventory.
 *
 * A directory joins when it holds a lockfile (or go.mod / requirements.txt,
 * which carry exact versions themselves). Workspace members that share their
 * root's lockfile are covered by the root. Bounded: depth 2 below the root,
 * at most 64 projects, and directories that hold installed code, build
 * output, test fixtures or tool worktrees are skipped.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { IgnoreRules } from "./gitignore.js";
import { scanCurrentProject, type ProjectScanResult } from "./project-scanner.js";

const LOCK_MARKERS = [
  "package-lock.json",
  "npm-shrinkwrap.json",
  "pnpm-lock.yaml",
  "yarn.lock",
  "bun.lock",
  "Cargo.lock",
  "poetry.lock",
  "uv.lock",
  "Pipfile.lock",
  "go.mod",
  "requirements.txt",
];

const SKIP_DIRS = new Set([
  "node_modules", "target", ".git", "dist", "build", "out", "vendor", ".venv", "venv",
  "__pycache__", ".next", ".nuxt", "coverage", ".claude", ".codex", ".cursor", "fixtures",
  "__fixtures__", "testdata", "test-fixtures", "__tests__", ".turbo", ".cache", ".pnpm-store",
]);

const MAX_DEPTH = 2;
const MAX_PROJECTS = 64;

export interface ProjectTreeEntry {
  dir: string;
  scan: ProjectScanResult;
}

/**
 * The root (always) and every locked project below it, each with its own
 * scan. Directories and lockfiles the repository's .gitignore excludes are not
 * part of the project and are skipped (gitignore.ts).
 */
export function scanProjectTree(root: string): ProjectTreeEntry[] {
  const entries: ProjectTreeEntry[] = [{ dir: root, scan: scanCurrentProject(root) }];
  const ignore = new IgnoreRules(root);
  const walk = (dir: string, rel: string, depth: number) => {
    if (depth > MAX_DEPTH || entries.length >= MAX_PROJECTS) return;
    ignore.load(rel);
    let children: fs.Dirent[];
    try {
      children = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const child of children) {
      if (entries.length >= MAX_PROJECTS) return;
      if (!child.isDirectory() || SKIP_DIRS.has(child.name) || child.name.startsWith(".")) continue;
      const childRel = rel ? `${rel}/${child.name}` : child.name;
      if (ignore.ignores(childRel, true)) continue;
      const sub = path.join(dir, child.name);
      ignore.load(childRel);
      const locked = LOCK_MARKERS.some(
        (marker) => fs.existsSync(path.join(sub, marker)) && !ignore.ignores(`${childRel}/${marker}`, false),
      );
      if (locked) entries.push({ dir: sub, scan: scanCurrentProject(sub) });
      walk(sub, childRel, depth + 1);
    }
  };
  walk(root, "", 1);
  return entries;
}

/** Lockfiles that make an ecosystem scannable in a directory even when no direct dependency was parsed. */
const LOCKS_BY_LANGUAGE: Record<string, string[]> = {
  npm: ["package-lock.json", "npm-shrinkwrap.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock"],
  rust: ["Cargo.lock"],
  python: ["poetry.lock", "uv.lock", "Pipfile.lock", "requirements.txt"],
  go: ["go.mod"],
};

/**
 * Resolution groups for the live layer: one per (project directory, ecosystem).
 * A lockfile whose manifest declared nothing the scanner could read still gets
 * a group, so its installed packages are scanned (as transitives) rather than
 * silently dropped.
 */
export function treeResolutionGroups(entries: ProjectTreeEntry[]): Array<{
  dir: string;
  language: string;
  deps: string[];
  devDeps: string[];
  targets: Record<string, string>;
}> {
  return entries.flatMap(({ dir, scan }) => {
    const byLanguage = { ...scan.depsByEcosystem };
    for (const [language, locks] of Object.entries(LOCKS_BY_LANGUAGE)) {
      if (!byLanguage[language] && locks.some((lock) => fs.existsSync(path.join(dir, lock)))) {
        byLanguage[language] = { deps: [], devDeps: [] };
      }
    }
    return Object.entries(byLanguage).map(([language, { deps, devDeps }]) => ({
      dir,
      language,
      deps,
      devDeps,
      targets: scan.depTargets,
    }));
  });
}
