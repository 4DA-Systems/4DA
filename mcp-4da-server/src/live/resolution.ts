// SPDX-License-Identifier: Apache-2.0
/**
 * Dependency resolution that knows when it has gone stale.
 *
 * Measured 2026-09-10: three `node ./mcp-4da-server/dist/index.js` processes
 * were live and two had started before the pull that brought hono 4.13.5.
 * `vulnerability_scan` from one of them reported hono 4.13.3 (the lockfile
 * value when it started) with `_meta.cached: false`. The live layer resolved
 * versions once at init and never again, and `cached` described only the OSV
 * lookup, so nothing in the answer could reveal that it was stale.
 *
 * A group records every file its resolution depended on as a stat signature:
 * each lockfile candidate (present or absent, so one appearing or vanishing
 * counts), the manifest when the resolver fell back to it, and for npm the
 * node_modules install state. Signatures are taken BEFORE reading, so a write
 * that lands mid-read still registers as a change. `resolveGroup` depends
 * only on its inputs and the filesystem, so the SAME resolution can be re-run
 * when a signature moves. Stat calls only: no timers, no watchers.
 */

import { anySignatureChanged, signatureMap, signatureMtime, type FileSignature } from "./file-signature.js";
import { checkInstallState } from "./install-state.js";
import { lockfileCandidates, manifestCandidate, resolveVersionSource } from "./lockfile-parsers.js";
import { mapEcosystem, resolveAuditVersionsFrom, resolveVersionsFrom } from "./version-resolver.js";
import type { InstallDriftRecord, ResolutionSourceRecord, ResolvedDependency } from "./types.js";

/** The inputs of one resolution, kept so the same resolution can be re-run. */
export interface ResolutionGroup {
  dir: string;
  language: string;
  deps: string[];
  devDeps: string[];
  targets: Record<string, string>;
}

export interface GroupResolution {
  group: ResolutionGroup;
  /** Direct dependencies (dependency health, upgrade planning). */
  resolved: ResolvedDependency[];
  /** The full lockfile set plus drift entries (vulnerability scanning). */
  audit: ResolvedDependency[];
  drift: InstallDriftRecord[];
  /** The file the versions were read from, or null when the group resolved from nothing. */
  source: ResolutionSourceRecord | null;
  signatures: Map<string, FileSignature>;
  resolvedAt: string;
}

export function resolveGroup(group: ResolutionGroup): GroupResolution {
  const resolvedAt = new Date().toISOString();
  const ecosystem = mapEcosystem(group.language);
  const manifest = manifestCandidate(group.dir, ecosystem);
  const signatures = signatureMap([
    ...lockfileCandidates(group.dir, ecosystem),
    ...(manifest ? [manifest] : []),
  ]);

  const read = resolveVersionSource(group.dir, ecosystem);
  // The manifest only matters when the resolver fell back to it; editing a
  // script in package.json must not throw away a scan of an unchanged lockfile.
  if (manifest && read.kind === "lockfile") signatures.delete(manifest);

  let resolved = resolveVersionsFrom(group.dir, group.deps, group.devDeps, ecosystem, read.versions, group.targets);
  let drift: InstallDriftRecord[] = [];
  let driftAudit: ResolvedDependency[] = [];
  // Only a lockfile pins exact versions; comparing node_modules against a
  // manifest's specifier floor would report drift that is not there.
  if (ecosystem === "npm" && read.kind === "lockfile" && read.source) {
    const install = checkInstallState(group.dir, resolved, read.source);
    resolved = install.resolved;
    drift = install.drift;
    driftAudit = install.driftAuditDeps;
    for (const [file, signature] of install.signatures) signatures.set(file, signature);
  }

  const audit = [
    ...resolveAuditVersionsFrom(
      group.dir,
      group.deps,
      group.devDeps,
      ecosystem,
      read.versions,
      group.targets,
      resolved,
    ),
    ...driftAudit,
  ];

  const source: ResolutionSourceRecord | null =
    read.source && read.kind
      ? { path: read.source, kind: read.kind, mtimeMs: signatureMtime(signatures.get(read.source)) }
      : null;

  return { group, resolved, audit, drift, source, signatures, resolvedAt };
}

/** True when any file the resolution depended on changed, appeared, or vanished. */
export function groupIsStale(result: GroupResolution): boolean {
  return anySignatureChanged(result.signatures);
}
