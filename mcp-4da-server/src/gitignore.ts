// SPDX-License-Identifier: Apache-2.0
/**
 * Enough of .gitignore to decide whether a directory or lockfile belongs to
 * the project, for the standalone project-tree walk.
 *
 * A directory the repository ignores is not part of the project: scratch
 * clones, generated output, a sibling tool checked out inside the tree.
 * Measured 2026-10-02: walking 4DA's root without this scanned the ignored
 * `victauri-gauntlet/` and `cli/` and reported their advisories (rustls, h2,
 * scc) as the project's, where osv-scanner, which honours .gitignore, did not.
 *
 * Read from the files, never by running git: `git check-ignore` in a cloned
 * repository honours that repository's config, and a config can name programs
 * for git to run (core.fsmonitor). Reading a scanned folder must not execute
 * anything it names.
 *
 * Supported: comments, blank lines, `dir/` (directory only), leading `/` and
 * inner `/` anchoring, `*`, `?`, `**`, nested .gitignore files and
 * .git/info/exclude. A negation (`!pattern`) is treated conservatively: a
 * path that any negation could re-include is never skipped.
 */

import * as fs from "node:fs";
import * as path from "node:path";

interface Rule {
  /** Directory the rule's file lives in, relative to the walk root ("" for the root). */
  base: string;
  regex: RegExp;
  dirOnly: boolean;
  negated: boolean;
}

const caseless = process.platform === "win32" || process.platform === "darwin";

function globToRegex(glob: string, anchored: boolean): RegExp {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const ch = glob[i];
    if (ch === "*") {
      if (glob[i + 1] === "*") {
        re += ".*";
        i++;
        if (glob[i + 1] === "/") i++;
      } else re += "[^/]*";
    } else if (ch === "?") re += "[^/]";
    else re += ch.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  // Unanchored patterns match at any depth below their file's directory.
  return new RegExp(`^${anchored ? "" : "(?:.*/)?"}${re}(?:/.*)?$`, caseless ? "i" : "");
}

function parseRules(content: string, base: string): Rule[] {
  const rules: Rule[] = [];
  for (const raw of content.split(/\r?\n/)) {
    let line = raw.replace(/\s+$/, "");
    if (!line || line.startsWith("#")) continue;
    const negated = line.startsWith("!");
    if (negated) line = line.slice(1);
    const dirOnly = line.endsWith("/");
    if (dirOnly) line = line.slice(0, -1);
    const anchored = line.startsWith("/") || line.includes("/");
    line = line.replace(/^\//, "");
    if (!line) continue;
    rules.push({ base, regex: globToRegex(line, anchored), dirOnly, negated });
  }
  return rules;
}

/** Ignore rules gathered while walking down from a root. */
export class IgnoreRules {
  private rules: Rule[] = [];
  private loaded = new Set<string>();

  constructor(private readonly root: string) {
    this.load("");
    try {
      this.rules.push(...parseRules(fs.readFileSync(path.join(root, ".git", "info", "exclude"), "utf-8"), ""));
    } catch {
      // Not a git checkout, or no exclude file.
    }
  }

  /** Read the .gitignore of a directory (relative to the root) once. */
  load(relDir: string): void {
    if (this.loaded.has(relDir)) return;
    this.loaded.add(relDir);
    try {
      this.rules.push(...parseRules(fs.readFileSync(path.join(this.root, relDir, ".gitignore"), "utf-8"), relDir));
    } catch {
      // No .gitignore here.
    }
  }

  /** Whether a path (relative to the root, `/`-separated) is ignored. */
  ignores(relPath: string, isDir: boolean): boolean {
    let ignored = false;
    let reincludable = false;
    for (const rule of this.rules) {
      if (rule.base && !relPath.startsWith(`${rule.base}/`)) continue;
      const local = rule.base ? relPath.slice(rule.base.length + 1) : relPath;
      if (rule.dirOnly && !isDir && !local.includes("/")) continue;
      if (!rule.regex.test(local)) continue;
      if (rule.negated) reincludable = true;
      else ignored = true;
    }
    return ignored && !reincludable;
  }
}
