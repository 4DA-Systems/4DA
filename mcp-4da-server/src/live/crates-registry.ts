// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * crates.io Registry Fetcher
 *
 * Queries the crates.io sparse index (CDN, no rate limit) for package version
 * data. Avoids the main crates.io API which has a strict 1 req/s rate limit.
 *
 * Privacy: sends only crate names — public manifest data.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchWithTimeout } from "./http-utils.js";
import type { RegistryPackageInfo, SemverDistance } from "./types.js";
import { computeSemverDistance, isPreRelease } from "./semver-utils.js";

const CRATES_TIMEOUT_MS = 8_000;
const CRATES_CACHE_TTL = 86_400; // 24 hours

interface SparseIndexEntry {
  name: string;
  vers: string;
  yanked: boolean;
}

export class CratesRegistry {
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
    const cacheKey = `crates-reg:${name}`;
    const cached = this.cache.get<RegistryPackageInfo>(cacheKey);
    if (cached !== null) return cached;

    if (!this.rateLimiter.canProceed("crates")) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      return errorResult(name, currentVersion, isDev, "Rate limited");
    }

    try {
      this.rateLimiter.consume("crates");

      const url = sparseIndexUrl(name);
      const response = await fetchWithTimeout(
        url,
        { headers: { Accept: "application/json" } },
        CRATES_TIMEOUT_MS,
      );

      if (!response.ok) {
        throw new Error(`Sparse index HTTP ${response.status}: ${url}`);
      }

      const text = await response.text();
      const entries = parseIndexLines(text);

      if (entries.length === 0) {
        throw new Error(`No versions found for crate: ${name}`);
      }

      // Latest = last entry overall (may be yanked or prerelease)
      const latestEntry = entries[entries.length - 1];
      const latestVersion = latestEntry.vers;

      // Latest stable = last non-yanked, non-prerelease entry
      const latestStable = findLatestStable(entries);

      // Semver distance between current and latest stable (or latest)
      let versionsBehind: SemverDistance | null = null;
      if (currentVersion && (latestStable || latestVersion)) {
        versionsBehind = computeSemverDistance(currentVersion, latestStable || latestVersion);
      }

      const result: RegistryPackageInfo = {
        name,
        ecosystem: "crates.io",
        currentVersion,
        latestVersion,
        latestStableVersion: latestStable,
        versionsBehind,
        deprecated: false, // sparse index does not carry deprecation info
        deprecationMessage: null,
        lastPublished: null, // sparse index does not carry dates
        license: null, // sparse index does not carry license
        weeklyDownloads: null, // sparse index does not carry downloads
        isDev,
        fetchError: null,
      };

      this.cache.set(cacheKey, result, "crates", CRATES_CACHE_TTL);
      return result;
    } catch (err) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      const message = err instanceof Error ? err.message : String(err);
      return errorResult(name, currentVersion, isDev, message);
    }
  }
}

function sparseIndexUrl(name: string): string {
  const n = name.toLowerCase();
  switch (n.length) {
    case 1: return `https://index.crates.io/1/${n}`;
    case 2: return `https://index.crates.io/2/${n}`;
    case 3: return `https://index.crates.io/3/${n[0]}/${n}`;
    default: return `https://index.crates.io/${n.slice(0, 2)}/${n.slice(2, 4)}/${n}`;
  }
}

function parseIndexLines(text: string): SparseIndexEntry[] {
  const entries: SparseIndexEntry[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length === 0) continue;
    try {
      const entry = JSON.parse(trimmed) as SparseIndexEntry;
      if (entry.name && entry.vers) {
        entries.push(entry);
      }
    } catch {
      // Skip malformed lines
    }
  }
  return entries;
}

function findLatestStable(entries: SparseIndexEntry[]): string | null {
  for (let i = entries.length - 1; i >= 0; i--) {
    if (!entries[i].yanked && !isPreRelease(entries[i].vers)) {
      return entries[i].vers;
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
    ecosystem: "crates.io",
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
