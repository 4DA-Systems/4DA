// SPDX-License-Identifier: Apache-2.0
/**
 * Registry health lookups for resolved dependencies (moved out of
 * live/index.ts unchanged, so the coordinator stays within its size limit).
 */

import { computeSemverDistance } from "./semver-utils.js";
import type { NpmRegistry } from "./npm-registry.js";
import type { CratesRegistry } from "./crates-registry.js";
import type { PyPIRegistry } from "./pypi-registry.js";
import type { GoRegistry } from "./go-registry.js";
import type { RegistryPackageInfo, ResolvedDependency } from "./types.js";

export interface Registries {
  npm: NpmRegistry;
  crates: CratesRegistry;
  pypi: PyPIRegistry;
  go: GoRegistry;
}

function unavailable(dep: ResolvedDependency, fetchError: string): RegistryPackageInfo {
  return {
    name: dep.name, ecosystem: dep.ecosystem, currentVersion: dep.version,
    latestVersion: null, latestStableVersion: null, versionsBehind: null,
    deprecated: false, deprecationMessage: null, lastPublished: null,
    license: null, weeklyDownloads: null, isDev: dep.isDev, fetchError,
  };
}

export async function fetchRegistryHealthFor(
  deps: ResolvedDependency[],
  registries: Registries,
  enabled: boolean,
): Promise<RegistryPackageInfo[]> {
  if (!enabled) return deps.map((d) => unavailable(d, "Offline mode"));

  const registryForEcosystem = (eco: string) => {
    switch (eco) {
      case "npm": return registries.npm;
      case "crates.io": return registries.crates;
      case "PyPI": return registries.pypi;
      case "Go": return registries.go;
      default: return null;
    }
  };

  const results = await Promise.all(
    deps.map(async (dep) => {
      const registry = registryForEcosystem(dep.ecosystem);
      if (!registry) return unavailable(dep, `No registry fetcher for ${dep.ecosystem}`);
      try {
        const info = await registry.getPackageInfo(dep.name, dep.version, dep.isDev);
        return restampRegistryContext(info, dep);
      } catch {
        return unavailable(dep, "Registry fetch failed");
      }
    }),
  );

  // Bulk fetch npm downloads for npm deps
  const npmDeps = deps.filter((d) => d.ecosystem === "npm");
  if (npmDeps.length > 0) {
    try {
      const downloads = await registries.npm.getBulkDownloads(npmDeps.map((d) => d.name));
      for (const result of results) {
        if (result.ecosystem === "npm" && downloads.has(result.name)) {
          result.weeklyDownloads = downloads.get(result.name) || null;
        }
      }
    } catch {
      // Downloads are nice-to-have, not critical
    }
  }

  return results;
}

/**
 * Registry caches key by package NAME, but the cached record embeds the
 * QUERYING dependency's `currentVersion`, `versionsBehind`, and `isDev`. Two
 * instances of one package at different versions (better-sqlite3 11.10.0 and
 * 12.11.1 across workspaces) therefore both came back wearing the first
 * instance's version — the upgrade planner then showed two identical rows and
 * lost the older instance entirely. Registry facts (latest version,
 * deprecation, downloads) are per-package and cacheable; the per-instance
 * fields are re-stamped here from the dep actually being asked about.
 */
function restampRegistryContext(
  info: RegistryPackageInfo,
  dep: ResolvedDependency,
): RegistryPackageInfo {
  const latest = info.latestStableVersion || info.latestVersion;
  return {
    ...info,
    currentVersion: dep.version,
    isDev: dep.isDev,
    versionsBehind: dep.version && latest ? computeSemverDistance(dep.version, latest) : null,
  };
}
