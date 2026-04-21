// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Go Module Proxy Registry Fetcher
 *
 * Queries the Go module proxy for package metadata (latest version, version
 * list, semver distance). The proxy does not expose deprecation, license, or
 * download data.
 *
 * Privacy: sends only module paths -- public manifest data.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchJson, fetchWithTimeout } from "./http-utils.js";
import { computeSemverDistance, isPreRelease } from "./semver-utils.js";
import type { RegistryPackageInfo } from "./types.js";

const GO_PROXY_URL = "https://proxy.golang.org";
const GO_TIMEOUT_MS = 8_000;
const GO_CACHE_TTL = 86_400; // 24 hours

interface GoLatestResponse {
  Version: string;
  Time: string;
}

export class GoRegistry {
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
    const cacheKey = `go-reg:${name}`;
    const cached = this.cache.get<RegistryPackageInfo>(cacheKey);
    if (cached !== null) return cached;

    if (!this.rateLimiter.canProceed("go")) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      return errorResult(name, currentVersion, isDev, "Rate limited");
    }

    try {
      this.rateLimiter.consume("go");

      const escaped = escapeModulePath(name);

      // Fetch @latest for latest version and timestamp
      const latest = await fetchJson<GoLatestResponse>(
        `${GO_PROXY_URL}/${escaped}/@latest`,
        {},
        GO_TIMEOUT_MS,
      );

      const latestVersion = latest.Version ?? null;
      const lastPublished = latest.Time ?? null;

      // Fetch @v/list for all versions to find latest stable
      let latestStable: string | null = null;
      try {
        const listResponse = await fetchWithTimeout(
          `${GO_PROXY_URL}/${escaped}/@v/list`,
          {},
          GO_TIMEOUT_MS,
        );
        if (listResponse.ok) {
          const text = await listResponse.text();
          const versions = text.trim().split("\n").filter(Boolean);
          latestStable = findLatestStable(versions);
        }
      } catch {
        // Version list unavailable -- fall back to @latest if it's stable
        if (latestVersion && !isPreRelease(latestVersion.replace(/^v/, ""))) {
          latestStable = latestVersion;
        }
      }

      // Semver distance: strip v prefix for comparison
      let versionsBehind = null;
      const compareTarget = latestStable || latestVersion;
      if (currentVersion && compareTarget) {
        versionsBehind = computeSemverDistance(
          currentVersion.replace(/^v/, ""),
          compareTarget.replace(/^v/, ""),
        );
      }

      const result: RegistryPackageInfo = {
        name,
        ecosystem: "Go",
        currentVersion,
        latestVersion,
        latestStableVersion: latestStable,
        versionsBehind,
        deprecated: false,
        deprecationMessage: null,
        lastPublished,
        license: null, // Go proxy does not expose license info
        weeklyDownloads: null, // Go proxy does not expose download counts
        isDev,
        fetchError: null,
      };

      this.cache.set(cacheKey, result, "go", GO_CACHE_TTL);
      return result;
    } catch (err) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      const message = err instanceof Error ? err.message : String(err);
      return errorResult(name, currentVersion, isDev, message);
    }
  }
}

/**
 * Escape uppercase letters in Go module paths for the proxy URL.
 * Go proxy convention: uppercase 'X' becomes '!x'.
 */
function escapeModulePath(mod: string): string {
  return mod.replace(/[A-Z]/g, (c) => "!" + c.toLowerCase());
}

/**
 * Find the latest stable version from a list of Go module versions.
 * Versions use "v" prefix (v1.2.3) -- strip for prerelease check.
 */
function findLatestStable(versions: string[]): string | null {
  for (let i = versions.length - 1; i >= 0; i--) {
    const v = versions[i];
    if (!isPreRelease(v.replace(/^v/, ""))) {
      return v;
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
    ecosystem: "Go",
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
