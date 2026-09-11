// SPDX-License-Identifier: Apache-2.0
/**
 * Set operations over resolved dependencies, shared by the live layer.
 * (Moved out of live/index.ts; behaviour unchanged except for the install
 * fields, which did not exist before.)
 */

import type { ResolvedDependency, VulnerabilityScanResult } from "./types.js";

/**
 * Deepest common ancestor of a set of directories (segment-wise, both slash
 * styles). Null for an empty set; a single dir is its own root.
 */
export function commonPathRoot(dirs: string[]): string | null {
  if (dirs.length === 0) return null;
  const split = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").split("/");
  let common = split(dirs[0]);
  for (const dir of dirs.slice(1)) {
    const parts = split(dir);
    let i = 0;
    while (i < common.length && i < parts.length && common[i].toLowerCase() === parts[i].toLowerCase()) i++;
    common = common.slice(0, i);
    if (common.length === 0) break;
  }
  return common.length > 0 ? common.join("/") : null;
}

export function dedupeDependencies(deps: ResolvedDependency[]): ResolvedDependency[] {
  const unique = new Map<string, ResolvedDependency>();
  for (const dep of deps) {
    const key = `${dep.ecosystem}\0${dep.name}\0${dep.version ?? ""}`;
    const existing = unique.get(key);
    if (existing) {
      existing.isDirect ||= dep.isDirect;
      existing.devScopeKnown &&= dep.devScopeKnown;
      existing.isDev = existing.devScopeKnown && existing.isDev && dep.isDev;
      // A crate reachable via ANY active path is active; keep a target label if present.
      existing.platformActive ||= dep.platformActive;
      existing.target = existing.target ?? dep.target;
      // Union the provenance rather than discarding it — the same version can
      // legitimately be pinned by several workspaces, and the reader needs all
      // of them to know where to apply the fix.
      for (const dir of dep.sourceDirs ?? []) {
        if (!existing.sourceDirs.includes(dir)) existing.sourceDirs.push(dir);
      }
      mergeInstallContext(existing, dep);
    } else {
      unique.set(key, { ...dep, sourceDirs: [...(dep.sourceDirs ?? [])] });
    }
  }
  return [...unique.values()];
}

/**
 * A drift entry exists only because node_modules holds a version the lockfile
 * does not pin. When a lockfile really does pin that same version somewhere,
 * the merged entry is a lockfile instance, not drift, and must not read as
 * "only in node_modules". Two drift entries keep the first one's context.
 * A drifted install reading wins over a matching one, so a merge never hides
 * a disagreement.
 */
function mergeInstallContext(existing: ResolvedDependency, dep: ResolvedDependency): void {
  if (existing.installDriftOf !== undefined && dep.installDriftOf === undefined) {
    existing.installDriftOf = undefined;
    existing.installFix = undefined;
  }
  if (dep.installedVersion === undefined) return;
  const drifted = (d: ResolvedDependency) =>
    d.installedVersion !== undefined && d.installedVersion !== null && d.installedVersion !== d.version;
  if (existing.installedVersion === undefined || (drifted(dep) && !drifted(existing))) {
    existing.installedVersion = dep.installedVersion;
  }
}

export function emptyVulnResult(projectPath: string, offline: boolean): VulnerabilityScanResult {
  return {
    scannedAt: new Date().toISOString(),
    projectPath,
    ecosystemsScanned: [],
    totalScanned: 0,
    totalVulnerable: 0,
    platformInactiveVulnerable: 0,
    bySeverity: { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 },
    vulnerabilities: [],
    cleanCount: 0,
    scanDurationMs: 0,
    cached: false,
    offline,
  };
}
