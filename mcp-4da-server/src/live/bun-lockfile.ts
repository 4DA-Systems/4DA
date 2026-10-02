// SPDX-License-Identifier: Apache-2.0
/**
 * bun.lock reader (Bun 1.2+ text lockfile).
 *
 * Measured 2026-10-03 on the pre-publish held-out benchmark: the FastAPI
 * full-stack template's frontend is locked by bun.lock only, and the server
 * read nothing from it — 52 advisories osv-scanner reports (axios 1.18.0,
 * next 16.2.6, @babel/core 7.28.6, ...) were never scanned.
 *
 * Format: JSONC (trailing commas). `workspaces` lists each workspace's
 * declared dependencies; `packages` maps a resolution key to
 * `["name@version", registry, { dependencies, optionalDependencies, ... }, integrity]`.
 * The key is the package name for the hoisted copy and a path for a nested
 * one ("next/postcss" is the postcss next resolves to; scoped names keep their
 * slash: "@tanstack/router-plugin/@babel/core"). Workspace, link, file, git and
 * github entries do not name a registry version and are skipped.
 */

import type { InstanceSet } from "./lockfile-types.js";

interface BunWorkspace {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
  optionalDependencies?: Record<string, string>;
}

interface BunLock {
  workspaces?: Record<string, BunWorkspace>;
  packages?: Record<string, unknown>;
}

interface BunMeta {
  dependencies?: Record<string, string>;
  optionalDependencies?: Record<string, string>;
}

/** JSONC to JSON: drops trailing commas outside strings (bun.lock has no comments). */
export function stripTrailingCommas(text: string): string {
  let out = "";
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inString) {
      out += ch;
      if (ch === "\\") out += text[++i] ?? "";
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') inString = true;
    if (ch === ",") {
      let j = i + 1;
      while (j < text.length && /\s/.test(text[j])) j++;
      if (text[j] === "}" || text[j] === "]") continue;
    }
    out += ch;
  }
  return out;
}

/** "next/@babel/core" -> ["next", "@babel/core"]: a scope and its name are one segment. */
function keySegments(key: string): string[] {
  const parts = key.split("/");
  const segments: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    segments.push(parts[i].startsWith("@") && i + 1 < parts.length ? `${parts[i]}/${parts[++i]}` : parts[i]);
  }
  return segments;
}

/**
 * Reads a bun.lock into `versions` (the hoisted copy of each name) and
 * `instances` (every copy). Dev scope is known for every copy: dev when only
 * the workspaces' devDependencies reach it, runtime when a dependency or
 * optionalDependency does.
 */
export function readBunLock(content: string, versions: Map<string, string>, instances?: InstanceSet): void {
  const lock = JSON.parse(stripTrailingCommas(content)) as BunLock;
  const packages = lock.packages ?? {};

  const entry = (key: string): { name: string; version: string; meta: BunMeta } | null => {
    const value = packages[key];
    if (!Array.isArray(value) || typeof value[0] !== "string") return null;
    const at = value[0].lastIndexOf("@");
    if (at <= 0) return null;
    const version = value[0].slice(at + 1);
    if (!/^\d/.test(version)) return null;
    const meta = value[2] && typeof value[2] === "object" ? (value[2] as BunMeta) : {};
    return { name: value[0].slice(0, at), version, meta };
  };

  // Node-style resolution: the nearest nested copy, walking up the key, else the hoisted one.
  const resolve = (fromKey: string, dep: string): string | null => {
    const segments = keySegments(fromKey);
    for (let n = segments.length; n > 0; n--) {
      const candidate = [...segments.slice(0, n), dep].join("/");
      if (candidate in packages) return candidate;
    }
    return dep in packages ? dep : null;
  };

  const reach = (roots: Iterable<string>): Set<string> => {
    const seen = new Set<string>();
    const stack = [...roots].filter((name) => name in packages);
    while (stack.length > 0) {
      const key = stack.pop() as string;
      if (seen.has(key)) continue;
      seen.add(key);
      const e = entry(key);
      if (!e) continue;
      for (const dep of Object.keys({ ...e.meta.dependencies, ...e.meta.optionalDependencies })) {
        const next = resolve(key, dep);
        if (next && !seen.has(next)) stack.push(next);
      }
    }
    return seen;
  };

  const runtimeRoots = new Set<string>();
  const devRoots = new Set<string>();
  for (const ws of Object.values(lock.workspaces ?? {})) {
    for (const name of Object.keys({ ...ws.dependencies, ...ws.optionalDependencies })) runtimeRoots.add(name);
    for (const name of Object.keys(ws.devDependencies ?? {})) devRoots.add(name);
  }
  const runtime = reach(runtimeRoots);
  const dev = reach(devRoots);

  for (const key of Object.keys(packages)) {
    const e = entry(key);
    if (!e) continue;
    if (key === e.name && !versions.has(e.name)) versions.set(e.name, e.version);
    instances?.add(e.name, e.version, runtime.has(key) ? false : dev.has(key) ? true : undefined);
  }
}
