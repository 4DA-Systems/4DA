// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * npm Registry Fetcher
 *
 * Queries the npm registry for package metadata (latest version, deprecation
 * status, semver distance) and the npm downloads API for weekly download counts.
 *
 * Privacy: sends only package names — public manifest data.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchJson, fetchWithTimeout } from "./http-utils.js";
import type { RegistryPackageInfo, SemverDistance } from "./types.js";
import { computeSemverDistance, isPreRelease } from "./semver-utils.js";

const NPM_REGISTRY_URL = "https://registry.npmjs.org";
const NPM_DOWNLOADS_URL = "https://api.npmjs.org/downloads/point/last-week";
const NPM_TIMEOUT_MS = 8_000;
const NPM_CACHE_TTL = 86_400; // 24 hours
const DOWNLOADS_BATCH_SIZE = 128;

interface NpmAbbreviatedMeta {
  "dist-tags": Record<string, string>;
  modified?: string;
  versions: Record<string, { deprecated?: string }>;
}

interface NpmDownloadsResponse {
  [packageName: string]: { downloads: number } | null;
}

export class NpmRegistry {
  private cache: LiveCache;
  private rateLimiter: RateLimiter;

  constructor(cache: LiveCache, rateLimiter: RateLimiter) {
    this.cache = cache;
    this.rateLimiter = rateLimiter;
  }

  async getPackageInfo(
    name: string,
    currentVersion: string | null,
    isDev: boolean,
  ): Promise<RegistryPackageInfo> {
    const cacheKey = `npm-reg:${name}`;
    const cached = this.cache.get<RegistryPackageInfo>(cacheKey);
    if (cached !== null) return cached;

    if (!this.rateLimiter.canProceed("npm")) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      return errorResult(name, currentVersion, isDev, "Rate limited");
    }

    try {
      this.rateLimiter.consume("npm");

      const meta = await fetchJson<NpmAbbreviatedMeta>(
        `${NPM_REGISTRY_URL}/${encodeURIComponent(name)}`,
        { headers: { Accept: "application/vnd.npm.install-v1+json" } },
        NPM_TIMEOUT_MS,
      );

      const latestTag = meta["dist-tags"]?.latest ?? null;
      const versions = Object.keys(meta.versions || {});

      // Latest stable = last version that is not a prerelease
      const latestStable = findLatestStable(versions);

      // Check deprecation on the latest version entry
      const latestVersionEntry = latestTag ? meta.versions[latestTag] : null;
      const deprecated = !!latestVersionEntry?.deprecated;
      const deprecationMessage = latestVersionEntry?.deprecated ?? null;

      // Semver distance between current and latest stable (or latest tag)
      let versionsBehind: SemverDistance | null = null;
      if (currentVersion && (latestStable || latestTag)) {
        versionsBehind = computeSemverDistance(currentVersion, latestStable || latestTag!);
      }

      const result: RegistryPackageInfo = {
        name,
        ecosystem: "npm",
        currentVersion,
        latestVersion: latestTag,
        latestStableVersion: latestStable,
        versionsBehind,
        deprecated,
        deprecationMessage,
        lastPublished: meta.modified ?? null,
        license: null, // abbreviated metadata does not include license
        weeklyDownloads: null, // fetched separately via getBulkDownloads
        isDev,
        fetchError: null,
      };

      this.cache.set(cacheKey, result, "npm", NPM_CACHE_TTL);
      return result;
    } catch (err) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      const message = err instanceof Error ? err.message : String(err);
      return errorResult(name, currentVersion, isDev, message);
    }
  }

  async getBulkDownloads(names: string[]): Promise<Map<string, number>> {
    const result = new Map<string, number>();
    if (names.length === 0) return result;

    // Batch into groups of DOWNLOADS_BATCH_SIZE
    for (let i = 0; i < names.length; i += DOWNLOADS_BATCH_SIZE) {
      const batch = names.slice(i, i + DOWNLOADS_BATCH_SIZE);

      if (!this.rateLimiter.canProceed("npm")) break;

      try {
        this.rateLimiter.consume("npm");

        const scopedNames = batch.map((n) => encodeURIComponent(n));
        const url = `${NPM_DOWNLOADS_URL}/${scopedNames.join(",")}`;

        const response = await fetchWithTimeout(url, {}, NPM_TIMEOUT_MS);
        if (!response.ok) continue;

        const data = (await response.json()) as NpmDownloadsResponse;

        for (const [pkg, info] of Object.entries(data)) {
          if (info && typeof info.downloads === "number") {
            result.set(pkg, info.downloads);
          }
        }
      } catch {
        // Batch failure — continue with remaining batches
      }
    }

    return result;
  }
}

function findLatestStable(versions: string[]): string | null {
  for (let i = versions.length - 1; i >= 0; i--) {
    if (!isPreRelease(versions[i])) {
      return versions[i];
    }
  }
  return null;
}

function errorResult(
  name: string,
  currentVersion: string | null,
  isDev: boolean,
  fetchError: string,
): RegistryPackageInfo {
  return {
    name,
    ecosystem: "npm",
    currentVersion,
    latestVersion: null,
    latestStableVersion: null,
    versionsBehind: null,
    deprecated: false,
    deprecationMessage: null,
    lastPublished: null,
    license: null,
    weeklyDownloads: null,
    isDev,
    fetchError,
  };
}
