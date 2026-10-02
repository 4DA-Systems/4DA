// SPDX-License-Identifier: Apache-2.0
/**
 * Direct-dependency names from Python, Go and Cargo-workspace manifests, for
 * the project scanner (project-scanner.ts keeps npm and single-crate Cargo).
 *
 * Measured 2026-10-02, each gap below hid a whole project from the scan:
 * - Poetry projects declare dependencies under `[tool.poetry.dependencies]`;
 *   the scanner only looked for `dependencies = [`, found nothing, and never
 *   created a Python group — poetry 1.0's own lockfile (131 advisories per
 *   osv-scanner) scanned as zero dependencies.
 * - A Cargo virtual workspace root (`[workspace]`, no `[package]`) declares
 *   its dependencies in `[workspace.dependencies]` and in member manifests;
 *   victauri's whole Cargo.lock (10 advisories per cargo audit) was ignored.
 * - go.mod `// indirect` requirements are transitive, not direct.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { normalizePythonName, parseGoMod } from "./live/lockfile-parsers-pygo.js";

/** The body of a TOML table (`[name]`) up to the next table header, or null. */
export function tomlTable(content: string, name: string): string | null {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const header = new RegExp(`^\\[${escaped}\\]\\s*$`, "m").exec(content);
  if (!header) return null;
  const rest = content.slice(header.index + header[0].length);
  const next = rest.search(/^\[/m);
  return next === -1 ? rest : rest.slice(0, next);
}

/** Keys of a TOML table body: `name = ...` lines, skipping comments. */
function tomlKeys(body: string | null): string[] {
  if (!body) return [];
  const keys: string[] = [];
  for (const line of body.split(/\r?\n/)) {
    const m = /^\s*"?([A-Za-z0-9_.-]+)"?\s*=/.exec(line);
    if (m && !line.trim().startsWith("#")) keys.push(m[1]);
  }
  return keys;
}

/**
 * PEP 508 requirement strings inside a TOML array literal:
 * `"requests[socks]>=2; python_version>'3'"` -> requests. String literals are
 * read whole, so a quote INSIDE one (an environment marker) is not a new entry.
 */
function requirementNames(arrayBody: string): string[] {
  const names: string[] = [];
  for (const m of arrayBody.matchAll(/"((?:[^"\\]|\\.)*)"|'([^']*)'/g)) {
    const name = /^\s*([A-Za-z0-9][A-Za-z0-9._-]*)/.exec(m[1] ?? m[2] ?? "");
    if (name) names.push(name[1]);
  }
  return names;
}

/**
 * The array literal assigned to `key` inside `body` (multi-line), or "".
 * Brackets inside quoted strings do not close it: `"requests[socks]>=2"` is
 * an ordinary PEP 508 entry, and a non-greedy `\[...\]` stopped at its `]`
 * and dropped every dependency after it.
 */
function tomlArray(body: string | null, key: string): string {
  if (!body) return "";
  const start = new RegExp(`^\\s*${key}\\s*=\\s*\\[`, "m").exec(body);
  if (!start) return "";
  let depth = 1;
  let quote: string | null = null;
  const from = start.index + start[0].length;
  for (let i = from; i < body.length; i++) {
    const ch = body[i];
    if (quote) {
      if (ch === "\\") i++;
      else if (ch === quote) quote = null;
    } else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === "[") depth++;
    else if (ch === "]" && --depth === 0) return body.slice(from, i);
  }
  return body.slice(from);
}

/**
 * Direct Python dependencies from pyproject.toml: PEP 621 `[project]`
 * `dependencies`, `[project.optional-dependencies]` and PEP 735
 * `[dependency-groups]` (dev), Poetry's `[tool.poetry.dependencies]` and its
 * dev groups, and uv's `[tool.uv] dev-dependencies`. Names are PEP 503 normalised.
 */
export function parsePyprojectDependencies(content: string): { deps: string[]; devDeps: string[] } {
  const deps = new Set<string>();
  const devDeps = new Set<string>();
  const add = (set: Set<string>, names: string[]) => {
    for (const name of names) {
      const normalized = normalizePythonName(name);
      if (normalized !== "python") set.add(normalized);
    }
  };

  add(deps, requirementNames(tomlArray(tomlTable(content, "project"), "dependencies")));
  const optional = tomlTable(content, "project.optional-dependencies");
  if (optional) add(deps, requirementNames(optional.replace(/^\s*[A-Za-z0-9_.-]+\s*=/gm, "")));

  add(deps, tomlKeys(tomlTable(content, "tool.poetry.dependencies")));
  add(devDeps, tomlKeys(tomlTable(content, "tool.poetry.dev-dependencies")));
  for (const m of content.matchAll(/^\[tool\.poetry\.group\.([A-Za-z0-9_-]+)\.dependencies\]\s*$/gm)) {
    add(devDeps, tomlKeys(tomlTable(content, `tool.poetry.group.${m[1]}.dependencies`)));
  }

  add(devDeps, requirementNames(tomlArray(tomlTable(content, "tool.uv"), "dev-dependencies")));
  const groups = tomlTable(content, "dependency-groups");
  if (groups) add(devDeps, requirementNames(groups.replace(/^\s*[A-Za-z0-9_.-]+\s*=/gm, "")));

  for (const name of deps) devDeps.delete(name);
  return { deps: [...deps], devDeps: [...devDeps] };
}

/** Requirement names (pins and ranges alike) from a requirements file, PEP 503 normalised. */
export function parseRequirementNames(content: string): string[] {
  const names: string[] = [];
  for (const raw of content.replace(/\\\r?\n/g, " ").split(/\r?\n/)) {
    const line = raw.replace(/(^|\s)#.*$/, "").trim();
    if (!line || line.startsWith("-")) continue;
    const m = /^([A-Za-z0-9][A-Za-z0-9._-]*)/.exec(line);
    if (m) names.push(normalizePythonName(m[1]));
  }
  return [...new Set(names)];
}

/** Direct Go dependencies: go.mod requirements not marked `// indirect`. */
export function parseGoModDirectDeps(content: string): string[] {
  return parseGoMod(content)
    .requires.filter((r) => !r.indirect)
    .map((r) => r.module);
}

/**
 * Dependencies of a Cargo workspace root: `[workspace.dependencies]` plus the
 * `[dependencies]`/`[dev-dependencies]`/`[build-dependencies]` of every member
 * (`members = ["crates/*", "app"]`, one-level `*` globs). Member crates
 * themselves are excluded: they are this project's own code.
 */
export function cargoWorkspaceDeps(root: string, content: string): { deps: string[]; devDeps: string[] } {
  const workspace = tomlTable(content, "workspace");
  if (workspace === null) return { deps: [], devDeps: [] };
  const deps = new Set(tomlKeys(tomlTable(content, "workspace.dependencies")));
  const devDeps = new Set<string>();
  const memberNames = new Set<string>();

  const members = [...tomlArray(workspace, "members").matchAll(/["']([^"']+)["']/g)].map((m) => m[1]);
  for (const pattern of members) {
    for (const dir of expandMemberGlob(root, pattern)) {
      let manifest: string;
      try {
        manifest = fs.readFileSync(path.join(dir, "Cargo.toml"), "utf-8");
      } catch {
        continue;
      }
      const name = /^name\s*=\s*"([^"]+)"/m.exec(tomlTable(manifest, "package") ?? "")?.[1];
      if (name) memberNames.add(name);
      for (const dep of tomlKeys(tomlTable(manifest, "dependencies"))) deps.add(dep);
      for (const section of ["dev-dependencies", "build-dependencies"]) {
        for (const dep of tomlKeys(tomlTable(manifest, section))) devDeps.add(dep);
      }
    }
  }

  const strip = (set: Set<string>) => [...set].filter((d) => !memberNames.has(d));
  const runtime = strip(deps);
  return { deps: runtime, devDeps: strip(devDeps).filter((d) => !runtime.includes(d)) };
}

/** Member directories for one `members` entry: a path, or a path whose last segment is `*`. */
function expandMemberGlob(root: string, pattern: string): string[] {
  if (!pattern.includes("*")) return [path.join(root, pattern)];
  const star = pattern.indexOf("*");
  const parent = path.join(root, pattern.slice(0, star));
  try {
    return fs
      .readdirSync(parent, { withFileTypes: true })
      .filter((e) => e.isDirectory())
      .map((e) => path.join(parent, e.name))
      .filter((dir) => fs.existsSync(path.join(dir, "Cargo.toml")));
  } catch {
    return [];
  }
}
