// SPDX-License-Identifier: Apache-2.0
/**
 * Install drift: what node_modules holds versus what the lockfile pins.
 *
 * Measured 2026-09-10: mcp-4da-server/pnpm-lock.yaml pinned hono 4.13.5 (PR
 * #632, the fix for CVE-2026-84363/-84364/-84365) while
 * mcp-4da-server/node_modules/hono held 4.13.1 for 25 days, because
 * activation ran `pnpm install` only at the repo root. Every 4DA surface
 * reported hono fixed, because every surface read the lockfile. A lockfile
 * says what SHOULD be installed; only node_modules says what IS, and the
 * installed copy is the one that runs.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { fileSignature, type FileSignature } from "./file-signature.js";
import type { InstallDriftRecord, InstallFixCommand, ResolvedDependency } from "./types.js";

/** Parent directories the lookup may climb for hoisted workspaces. */
const MAX_PARENT_LEVELS = 6;

/**
 * Files an install rewrites (pnpm, npm >= 7, yarn 1, yarn berry's
 * node-modules linker). A change to one is how a reinstall that fixes drift,
 * without touching the lockfile, gets noticed.
 */
const INSTALL_MARKERS = [".modules.yaml", ".package-lock.json", ".yarn-integrity", ".yarn-state.yml"];

/** The reinstall command for the lockfile a directory resolved from, or null for a non-npm file. */
export function installFixFor(lockfilePath: string): InstallFixCommand | null {
  switch (path.basename(lockfilePath)) {
    case "pnpm-lock.yaml":
      return "pnpm install";
    case "package-lock.json":
      return "npm ci";
    case "yarn.lock":
      return "yarn install";
    default:
      return null;
  }
}

function isDirectory(p: string): boolean {
  try {
    return fs.statSync(p).isDirectory();
  } catch {
    return false;
  }
}

/**
 * The directories a lookup from `dir` visits, nearest first: `dir`, then its
 * parents, stopping at the directory that holds `.git` (the repository root)
 * or after MAX_PARENT_LEVELS parents.
 */
function lookupChain(dir: string): string[] {
  const chain: string[] = [];
  let current = path.resolve(dir);
  for (let level = 0; level <= MAX_PARENT_LEVELS; level++) {
    chain.push(current);
    if (fs.existsSync(path.join(current, ".git"))) break;
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return chain;
}

/**
 * `<d>/node_modules/<name>/package.json` for the nearest `d` in the lookup
 * chain that has it (scoped names included), or null. This is the copy
 * Node's resolver would load from `dir`.
 */
export function findInstalledManifest(dir: string, name: string, chain: string[] = lookupChain(dir)): string | null {
  for (const d of chain) {
    const candidate = path.join(d, "node_modules", ...name.split("/"), "package.json");
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

function readManifestVersion(manifest: string): string | null {
  try {
    const parsed = JSON.parse(fs.readFileSync(manifest, "utf-8")) as { version?: unknown };
    const version = typeof parsed.version === "string" ? parsed.version.trim() : "";
    return version || null;
  } catch {
    return null;
  }
}

/** The installed version of `name` as seen from `dir`, or null when not installed or unreadable. Never throws. */
export function readInstalledVersion(dir: string, name: string): string | null {
  const manifest = findInstalledManifest(dir, name);
  return manifest ? readManifestVersion(manifest) : null;
}

export interface InstallCheck {
  /** The direct deps with `installedVersion` filled in; a new array, inputs untouched. */
  resolved: ResolvedDependency[];
  /** One record per direct dep whose installed copy differs from its lockfile version. */
  drift: InstallDriftRecord[];
  /** Audit entries for installed versions the lockfile does not pin, flagged `installDriftOf`. */
  driftAuditDeps: ResolvedDependency[];
  /** Signatures, taken BEFORE reading, of every file whose change means the install state moved. */
  signatures: Map<string, FileSignature>;
}

/**
 * Compare each npm direct dependency's lockfile version with node_modules.
 *
 * Skipped entirely when `dir` has no node_modules of its own: a checkout that
 * was never installed has no installed copy to disagree with the lockfile,
 * and borrowing a parent project's node_modules would compare against a
 * different project's install. The node_modules path is still tracked, so an
 * install that creates it is noticed.
 */
export function checkInstallState(
  dir: string,
  resolved: ResolvedDependency[],
  lockfilePath: string,
): InstallCheck {
  const nodeModules = path.join(dir, "node_modules");
  const signatures = new Map<string, FileSignature>([[nodeModules, fileSignature(nodeModules)]]);
  const fix = installFixFor(lockfilePath);
  if (!fix || !isDirectory(nodeModules)) {
    return { resolved, drift: [], driftAuditDeps: [], signatures };
  }

  const chain = lookupChain(dir);
  for (const d of chain) {
    const nm = path.join(d, "node_modules");
    if (!isDirectory(nm)) continue;
    signatures.set(nm, fileSignature(nm));
    for (const marker of INSTALL_MARKERS) {
      const markerPath = path.join(nm, marker);
      signatures.set(markerPath, fileSignature(markerPath));
    }
  }

  const drift: InstallDriftRecord[] = [];
  const driftAuditDeps: ResolvedDependency[] = [];
  const checked = resolved.map((dep) => {
    if (!dep.isDirect || dep.ecosystem !== "npm" || !dep.version) return dep;
    const manifest = findInstalledManifest(dir, dep.name, chain);
    if (manifest) signatures.set(manifest, fileSignature(manifest));
    const installedVersion = manifest ? readManifestVersion(manifest) : null;
    if (installedVersion && installedVersion !== dep.version) {
      drift.push({
        package: dep.name,
        dir,
        lockfileVersion: dep.version,
        installedVersion,
        fix,
        isDev: dep.isDev,
      });
      driftAuditDeps.push({
        ...dep,
        version: installedVersion,
        installedVersion,
        installDriftOf: dep.version,
        installFix: fix,
      });
    }
    return { ...dep, installedVersion };
  });

  return { resolved: checked, drift, driftAuditDeps, signatures };
}
