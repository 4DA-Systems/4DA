// SPDX-License-Identifier: Apache-2.0
/**
 * Version Resolver
 *
 * Extracts exact dependency versions from lock files.
 * Priority: lock file (exact) > manifest (range/specifier).
 * Uses only Node.js built-ins — no external parsers. The per-ecosystem
 * parsers live in lockfile-parsers.ts, which also reports which file each
 * resolution read from.
 */

import type { OsvEcosystem, ResolvedDependency } from "./types.js";
import { targetActiveOnHost } from "./platform.js";
import { activeCratesForHost, hostTriple } from "./cargo-platform.js";
import { resolveVersionSource } from "./lockfile-parsers.js";

// Alias -> OSV ecosystem. Mirrors the desktop app's canonical
// `Ecosystem::parse` (src-tauri/src/ecosystem.rs) so both sides recognize the
// SAME aliases — they previously drifted (the Rust side knew csharp/dart/php
// while this map keyed C# as "dotnet" and omitted Dart entirely, mislabeling
// those deps). Keep this in sync with that enum.
const ECOSYSTEM_MAP: Record<string, OsvEcosystem> = {
  npm: "npm",
  javascript: "npm",
  typescript: "npm",
  node: "npm",
  js: "npm",
  ts: "npm",
  rust: "crates.io",
  cargo: "crates.io",
  "crates.io": "crates.io",
  crates: "crates.io",
  python: "PyPI",
  pypi: "PyPI",
  pip: "PyPI",
  py: "PyPI",
  go: "Go",
  golang: "Go",
  java: "Maven",
  maven: "Maven",
  kotlin: "Maven",
  gradle: "Maven",
  csharp: "NuGet",
  "c#": "NuGet",
  dotnet: "NuGet",
  nuget: "NuGet",
  php: "Packagist",
  composer: "Packagist",
  packagist: "Packagist",
  ruby: "RubyGems",
  rubygems: "RubyGems",
  gem: "RubyGems",
  dart: "Pub",
  flutter: "Pub",
  pub: "Pub",
};

/**
 * Map a language/ecosystem alias to its OSV ecosystem id (case-insensitive).
 *
 * Falls back to "npm" ONLY for genuinely unrecognized input — a last resort, not
 * a real default. OSV-unindexed languages (Swift, C/C++) and unknowns produce no
 * OSV matches anyway (their package names don't collide with the npm namespace),
 * so this never fabricates a vulnerability. Ecosystems that lack a version
 * resolver here (NuGet/Maven/Packagist/RubyGems/Pub) still get the CORRECT label,
 * so the moment a resolver is added the scan is right by construction.
 */
export function mapEcosystem(language: string): OsvEcosystem {
  return ECOSYSTEM_MAP[language.trim().toLowerCase()] || "npm";
}

/**
 * crates.io treats `-` and `_` as one namespace (you cannot publish both
 * `http-body-util` and `http_body_util`), but the two spellings reach this
 * resolver from different sources: Cargo.lock records the publisher's canonical
 * name while ACE's import scraper reports the IMPORT identifier, which is
 * always underscored. An exact-match lookup then fails for the import spelling
 * and the same crate surfaces twice — once versioned, once `version: null`
 * (observed live 2026-08-30: http_body_util, async_trait, tower_http,
 * ed25519_dalek, ts_rs, proc_macro2, victauri_test, all null beside their
 * hyphenated versioned twins). Resolve via the spelling the lock file actually
 * uses and RETURN that spelling, so downstream dedupe merges the variants.
 */
function canonicalCrateName(name: string, versionMap: Map<string, string>): string {
  if (versionMap.has(name)) return name;
  const swapped = name.includes("_") ? name.replace(/_/g, "-") : name.replace(/-/g, "_");
  return versionMap.has(swapped) ? swapped : name;
}

export function resolveVersions(
  cwd: string,
  deps: string[],
  devDeps: string[],
  language: string,
  targets: Record<string, string> = {},
): ResolvedDependency[] {
  const ecosystem = mapEcosystem(language);
  const versionMap = resolveVersionSource(cwd, ecosystem).versions;
  return resolveVersionsFrom(cwd, deps, devDeps, ecosystem, versionMap, targets);
}

/** `resolveVersions` over a version map the caller already read from `cwd`. */
export function resolveVersionsFrom(
  cwd: string,
  deps: string[],
  devDeps: string[],
  ecosystem: OsvEcosystem,
  versionMap: Map<string, string>,
  targets: Record<string, string> = {},
): ResolvedDependency[] {
  const results: ResolvedDependency[] = [];

  const build = (rawName: string, isDev: boolean): ResolvedDependency => {
    const name =
      ecosystem === "crates.io" ? canonicalCrateName(rawName, versionMap) : rawName;
    const target = targets[rawName] ?? targets[name] ?? null;
    return {
      name,
      version: versionMap.get(normalizePackageName(name, ecosystem)) || null,
      ecosystem,
      isDev,
      isDirect: true,
      devScopeKnown: true,
      target,
      platformActive: targetActiveOnHost(target),
      // Provenance recorded at the point of resolution: this version came from
      // THIS manifest directory's lock file, not from "the project" generally.
      sourceDirs: [cwd],
    };
  };

  for (const name of deps) results.push(build(name, false));
  for (const name of devDeps) results.push(build(name, true));

  return results;
}

/**
 * Resolve the complete lockfile package set for vulnerability scanning.
 * Direct dependency health and upgrade planning continue to use resolveVersions.
 */
export function resolveAuditVersions(
  cwd: string,
  deps: string[],
  devDeps: string[],
  language: string,
  targets: Record<string, string> = {},
): ResolvedDependency[] {
  const ecosystem = mapEcosystem(language);
  const versionMap = resolveVersionSource(cwd, ecosystem).versions;
  return resolveAuditVersionsFrom(cwd, deps, devDeps, ecosystem, versionMap, targets);
}

/**
 * `resolveAuditVersions` over a version map the caller already read. `direct`
 * lets the caller pass direct entries it has already enriched (install state)
 * so the audit set carries the same objects' context.
 */
export function resolveAuditVersionsFrom(
  cwd: string,
  deps: string[],
  devDeps: string[],
  ecosystem: OsvEcosystem,
  versionMap: Map<string, string>,
  targets: Record<string, string> = {},
  direct: ResolvedDependency[] = resolveVersionsFrom(cwd, deps, devDeps, ecosystem, versionMap, targets),
): ResolvedDependency[] {
  // Canonicalize the same way resolveVersions does, so a dep declared under its
  // import spelling still marks the lock file's canonical entry as direct.
  const canonical = (name: string) =>
    normalizePackageName(
      ecosystem === "crates.io" ? canonicalCrateName(name, versionMap) : name,
      ecosystem,
    );
  const directNames = new Set(deps.map(canonical));
  const devNames = new Set(devDeps.map(canonical));
  const seen = new Set(direct.map((dep) => dependencyKey(dep)));
  const results = [...direct];

  // Cargo.lock is target-agnostic, so a Windows lockfile still lists the whole
  // Linux GTK3 stack Tauri pulls in. `[target.'cfg(...)']` parsing only ever
  // covered DIRECT gated deps, and that cluster is entirely transitive — which
  // is why nine unreachable crates were reported while the scan claimed to be
  // platform-filtered. Ask cargo to resolve the graph for this host instead.
  // `null` means cargo could not answer; every crate then stays active, because
  // "unknown" must never silently hide a real advisory.
  //
  // THE SHARED PREDICATE — keep in step with the app's
  // `src-tauri/src/platform_filter.rs`, which documents it in full:
  //
  //   a crate that is not RESOLVED FOR THE HOST is platform-inactive
  //
  // Two independent facts can make that true, and both are recorded because
  // the copy differs: (1) the manifest gates it behind a `cfg(...)` for a
  // target this machine is not — `targetActiveOnHost(declaredTarget)`, the
  // app's `project_dependencies.target_cfg`; (2) cargo builds it for no
  // target/feature combination here — `builtOnHost`, the app's
  // `user_dependencies.target_cfg = 'lockfile-only'` (schema 122). Neither
  // fact alone is the whole picture: (1) covers only DIRECT deps, and (2) is
  // the only one that can see a transitive.
  const hostCrates = ecosystem === "crates.io" ? activeCratesForHost(cwd) : null;
  const triple = hostCrates ? hostTriple() : null;

  for (const [name, version] of versionMap) {
    const normalized = normalizePackageName(name, ecosystem);
    const isDirect = directNames.has(normalized) || devNames.has(normalized);
    const declaredTarget = targets[name] ?? null;
    const builtOnHost = hostCrates ? hostCrates.has(name) : true;
    // Prefer the manifest's own cfg() spec when it has one — it is the more
    // precise, human-readable explanation. Fall back to naming the triple the
    // crate is absent from.
    const target =
      declaredTarget ?? (builtOnHost || !triple ? null : `not built for ${triple}`);
    const candidate: ResolvedDependency = {
      name,
      version,
      ecosystem,
      isDev: devNames.has(normalized),
      isDirect,
      devScopeKnown: isDirect,
      target,
      platformActive: targetActiveOnHost(declaredTarget) && builtOnHost,
      sourceDirs: [cwd],
    };
    const key = dependencyKey(candidate);
    if (!seen.has(key)) {
      seen.add(key);
      results.push(candidate);
    }
  }

  return results;
}

function normalizePackageName(name: string, ecosystem: OsvEcosystem): string {
  return ecosystem === "PyPI" ? name.toLowerCase() : name;
}

function dependencyKey(dep: ResolvedDependency): string {
  return `${dep.ecosystem}\0${normalizePackageName(dep.name, dep.ecosystem)}\0${dep.version ?? ""}`;
}
