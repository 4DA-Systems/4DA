// SPDX-License-Identifier: Apache-2.0
/**
 * Lockfile parsers, one per ecosystem, each reporting WHICH file it resolved
 * from as well as the versions it read.
 *
 * Priority per ecosystem: lock file (exact) > manifest (range/specifier).
 * Uses only Node.js built-ins — no external parsers. The source path is what
 * lets the live layer notice when that file changes and re-resolve (measured
 * 2026-09-10: a server started before a lockfile bump kept reporting hono
 * 4.13.3 after the lockfile said 4.13.5). The pnpm and yarn readers live in
 * js-lockfile-readers.ts; Python and Go in lockfile-parsers-pygo.ts.
 *
 * Every parser reports two things (2026-10-02):
 * - `instances`: EVERY installed copy, name@version, with the lockfile's own
 *   dev flag when it records one. A lockfile routinely holds several versions
 *   of one package (nested npm copies, two semver-incompatible crates). The
 *   reader used to keep one version per name, last one wins: measured against
 *   osv-scanner on 12 projects, nodegoat lost 287 of 1,091 installed copies
 *   and 4DA's src-tauri lost 143 of 934 — including its own `rsa 0.9.10`
 *   (RUSTSEC-2023-0071), overwritten by `rsa 0.10.0-rc.18` — and the direct
 *   `minimist@1.2.0` was reported as its nested `0.0.8`.
 * - `versions`: the ONE version per name a direct dependency resolves to (the
 *   top-level npm copy, the crate version a workspace member depends on),
 *   which dependency health and upgrade planning read.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { readBunLock } from "./bun-lockfile.js";
import { readPnpmLock, readYarnLock } from "./js-lockfile-readers.js";
import { resolveGo, resolvePython } from "./lockfile-parsers-pygo.js";
import { emptySource, found, InstanceSet, type PackageInstance, type VersionSource } from "./lockfile-types.js";
import type { OsvEcosystem } from "./types.js";

export type { PackageInstance, VersionSource } from "./lockfile-types.js";

const LOCKFILES: Partial<Record<OsvEcosystem, string[]>> = {
  npm: ["package-lock.json", "npm-shrinkwrap.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock"],
  "crates.io": ["Cargo.lock"],
  PyPI: ["poetry.lock", "uv.lock", "Pipfile.lock"],
  Go: ["go.mod", "go.sum"],
};

const MANIFESTS: Partial<Record<OsvEcosystem, string>> = {
  npm: "package.json",
  "crates.io": "Cargo.toml",
  PyPI: "requirements.txt",
};

/** Every lock file the resolver may read for this ecosystem, present or not, in priority order. */
export function lockfileCandidates(cwd: string, ecosystem: OsvEcosystem): string[] {
  return (LOCKFILES[ecosystem] ?? []).map((name) => path.join(cwd, name));
}

/** The manifest the resolver falls back to for this ecosystem, or null when it has none. */
export function manifestCandidate(cwd: string, ecosystem: OsvEcosystem): string | null {
  const name = MANIFESTS[ecosystem];
  return name ? path.join(cwd, name) : null;
}

export function resolveVersionSource(cwd: string, ecosystem: OsvEcosystem): VersionSource {
  switch (ecosystem) {
    case "npm":
      return resolveNpm(cwd);
    case "crates.io":
      return resolveRust(cwd);
    case "PyPI":
      return resolvePython(cwd);
    case "Go":
      return resolveGo(cwd);
    default:
      return emptySource();
  }
}

/** Instances for a manifest-only read: one per name, scope unknown. */
function manifestInstances(versions: Map<string, string>): PackageInstance[] {
  return [...versions].map(([name, version]) => ({ name, version }));
}

// =============================================================================
// npm: package-lock.json > npm-shrinkwrap.json > pnpm-lock.yaml > yarn.lock > bun.lock > package.json ranges
// =============================================================================

function resolveNpm(cwd: string): VersionSource {
  for (const name of ["package-lock.json", "npm-shrinkwrap.json"]) {
    const lockPath = path.join(cwd, name);
    if (!fs.existsSync(lockPath)) continue;
    try {
      const read = readPackageLock(JSON.parse(fs.readFileSync(lockPath, "utf-8")));
      if (read.instances.size > 0) return found(read.versions, read.instances, lockPath, "lockfile");
    } catch { /* fall through */ }
  }

  // pnpm-lock.yaml (v5.x, v6 and v9, read by column; see js-lockfile-readers.ts)
  const pnpmLockPath = path.join(cwd, "pnpm-lock.yaml");
  if (fs.existsSync(pnpmLockPath)) {
    try {
      const versions = new Map<string, string>();
      const instances = new InstanceSet();
      readPnpmLock(fs.readFileSync(pnpmLockPath, "utf-8"), versions, instances);
      if (versions.size > 0) return found(versions, instances, pnpmLockPath, "lockfile");
    } catch { /* fall through */ }
  }

  // yarn.lock (v1 and berry)
  const yarnLockPath = path.join(cwd, "yarn.lock");
  if (fs.existsSync(yarnLockPath)) {
    try {
      const versions = new Map<string, string>();
      const instances = new InstanceSet();
      const specs = new Map<string, string>();
      readYarnLock(fs.readFileSync(yarnLockPath, "utf-8"), versions, instances, specs);
      // The copy a direct dependency resolves to is the one its package.json range names.
      for (const [name, range] of manifestRanges(cwd)) {
        const version = specs.get(`${name}@${range}`);
        if (version) versions.set(name, version);
      }
      if (versions.size > 0) return found(versions, instances, yarnLockPath, "lockfile");
    } catch { /* fall through */ }
  }

  // bun.lock (Bun 1.2+ text lockfile; the binary bun.lockb is not readable)
  const bunLockPath = path.join(cwd, "bun.lock");
  if (fs.existsSync(bunLockPath)) {
    try {
      const versions = new Map<string, string>();
      const instances = new InstanceSet();
      readBunLock(fs.readFileSync(bunLockPath, "utf-8"), versions, instances);
      if (versions.size > 0) return found(versions, instances, bunLockPath, "lockfile");
    } catch { /* fall through */ }
  }

  // Fallback: extract version floors from package.json ranges
  const pkgPath = path.join(cwd, "package.json");
  if (fs.existsSync(pkgPath)) {
    const versions = new Map<string, string>();
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf-8"));
      const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
      for (const [name, spec] of Object.entries(allDeps)) {
        const version = extractVersionFromSpec(spec as string);
        if (version) versions.set(name, version);
      }
    } catch { /* skip */ }
    return found(versions, manifestInstances(versions), pkgPath, "manifest");
  }

  return emptySource();
}

/** The package name a package-lock `packages` key installs: the segment after its LAST `node_modules/`. */
export function packageNameFromNodeModulesPath(lockPath: string): string {
  const marker = "node_modules/";
  const index = lockPath.lastIndexOf(marker);
  return index === -1 ? "" : lockPath.slice(index + marker.length);
}

interface LockEntry {
  version?: string;
  dev?: boolean;
  link?: boolean;
  dependencies?: Record<string, LockEntry>;
}

/**
 * package-lock.json / npm-shrinkwrap.json, lockfile v1, v2 and v3.
 *
 * v2/v3 `packages` keys are install paths: `node_modules/a` is the hoisted
 * copy a direct dependency of the root resolves to, `node_modules/a/node_modules/b`
 * a nested one. npm writes `dev: true` on every entry only reachable from
 * devDependencies (`devOptional` = also reachable as an optional runtime dep,
 * so runtime), which makes the dev scope of a TRANSITIVE known. v1 nests the
 * same information under `dependencies`.
 */
export function readPackageLock(lock: {
  packages?: Record<string, LockEntry>;
  dependencies?: Record<string, LockEntry>;
}): { versions: Map<string, string>; instances: InstanceSet } {
  const versions = new Map<string, string>();
  const instances = new InstanceSet();

  if (lock.packages) {
    for (const [key, value] of Object.entries(lock.packages)) {
      const name = packageNameFromNodeModulesPath(key);
      if (!name || !value || value.link || !value.version) continue;
      instances.add(name, value.version, value.dev === true);
      // The hoisted root copy is what `require(name)` from the project resolves to.
      if (key === `node_modules/${name}`) versions.set(name, value.version);
    }
    // A workspace package's own nested copy (packages/x/node_modules/a) when nothing is hoisted.
    for (const instance of instances.toArray()) {
      if (!versions.has(instance.name)) versions.set(instance.name, instance.version);
    }
  }

  if (instances.size === 0 && lock.dependencies) {
    const walk = (deps: Record<string, LockEntry>, topLevel: boolean) => {
      for (const [name, value] of Object.entries(deps)) {
        if (value?.version && /^\d/.test(value.version)) {
          instances.add(name, value.version, value.dev === true);
          if (topLevel) versions.set(name, value.version);
        }
        if (value?.dependencies) walk(value.dependencies, false);
      }
    };
    walk(lock.dependencies, true);
  }

  return { versions, instances };
}

// =============================================================================
// Rust: Cargo.lock
// =============================================================================

interface CargoPackage {
  name: string;
  version: string;
  source: string | null;
  dependencies: string[];
}

/**
 * Every `[[package]]` table in a Cargo.lock, read field by field. The old
 * reader required `name` and `version` on consecutive lines; that holds for
 * cargo's own writer, but the table is TOML and nothing else guarantees it.
 */
export function parseCargoLockPackages(content: string): CargoPackage[] {
  const packages: CargoPackage[] = [];
  for (const block of content.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    const body = block.split(/^\[(?!\[package\]\])/m)[0];
    const field = (key: string) => new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m").exec(body)?.[1] ?? null;
    const name = field("name");
    const version = field("version");
    if (!name || !version) continue;
    const depsMatch = /^dependencies\s*=\s*\[([\s\S]*?)\]/m.exec(body);
    const dependencies = depsMatch ? [...depsMatch[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]) : [];
    packages.push({ name, version, source: field("source"), dependencies });
  }
  return packages;
}

function resolveRust(cwd: string): VersionSource {
  const lockPath = path.join(cwd, "Cargo.lock");
  if (!fs.existsSync(lockPath)) {
    return resolveRustFromManifest(cwd);
  }

  const versions = new Map<string, string>();
  const instances = new InstanceSet();
  try {
    const packages = parseCargoLockPackages(fs.readFileSync(lockPath, "utf-8"));
    const versionsByName = new Map<string, string[]>();
    // Cargo writes `source` on every registry or git package. In a file it
    // wrote, a package with none is a workspace member or path dependency:
    // local code, not a release, so no advisory can be about it. A lockfile
    // with no `source` anywhere was not written by cargo; keep everything.
    const cargoWritten = packages.some((pkg) => pkg.source !== null);
    const isLocal = (pkg: CargoPackage) => cargoWritten && pkg.source === null;
    for (const pkg of packages) {
      if (isLocal(pkg)) continue;
      instances.add(pkg.name, pkg.version);
      versionsByName.set(pkg.name, [...(versionsByName.get(pkg.name) ?? []), pkg.version]);
    }
    // The version a direct dependency resolves to: the one a workspace member
    // names. Cargo writes "name version" when two versions coexist, else "name".
    for (const member of packages.filter(isLocal)) {
      for (const dep of member.dependencies) {
        const [name, version] = dep.split(" ");
        const candidates = versionsByName.get(name);
        if (!candidates || versions.has(name)) continue;
        if (version && candidates.includes(version)) versions.set(name, version);
        else if (!version && candidates.length === 1) versions.set(name, candidates[0]);
      }
    }
    for (const [name, list] of versionsByName) {
      if (!versions.has(name)) versions.set(name, list[0]);
    }
  } catch { /* skip */ }

  return found(versions, instances, lockPath, "lockfile");
}

function resolveRustFromManifest(cwd: string): VersionSource {
  const versions = new Map<string, string>();
  const cargoPath = path.join(cwd, "Cargo.toml");
  if (!fs.existsSync(cargoPath)) return emptySource();

  try {
    const content = fs.readFileSync(cargoPath, "utf-8");
    // name = "1.0" or name = { version = "1.0", ... }
    const depRegex = /^([a-zA-Z_][\w-]*)\s*=\s*(?:"([^"]+)"|.*?version\s*=\s*"([^"]+)")/gm;
    let match;
    while ((match = depRegex.exec(content)) !== null) {
      const name = match[1];
      const version = match[2] || match[3];
      if (version && /^\d/.test(version)) {
        versions.set(name, version);
      }
    }
  } catch { /* skip */ }

  return found(versions, manifestInstances(versions), cargoPath, "manifest");
}

// =============================================================================
// Helpers
// =============================================================================

/** package.json dependency ranges (all dependency fields), or none when unreadable. */
function manifestRanges(cwd: string): Array<[string, string]> {
  try {
    const pkg = JSON.parse(fs.readFileSync(path.join(cwd, "package.json"), "utf-8"));
    const out: Array<[string, string]> = [];
    for (const field of ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"]) {
      for (const [name, range] of Object.entries(pkg[field] ?? {})) {
        if (typeof range === "string") out.push([name, range]);
      }
    }
    return out;
  } catch {
    return [];
  }
}

/**
 * Extract a usable version from an npm version specifier.
 * "^4.18.2" -> "4.18.2", "~1.0.0" -> "1.0.0", ">=2.0.0" -> "2.0.0"
 */
function extractVersionFromSpec(spec: string): string | null {
  if (!spec) return null;
  const match = spec.match(/(\d+\.\d+\.\d+(?:-[\w.]+)?)/);
  return match ? match[1] : null;
}
