// SPDX-License-Identifier: Apache-2.0
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
import { computeSemverDistance, maxStableSemver } from "./semver-utils.js";

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
    // The cache holds facts about the PACKAGE; the answer is computed for the
    // installed version on every call. It used to cache the finished answer by
    // name, so the second project to ask about a package got the first one's
    // `currentVersion` and distance.
    const cacheKey = `npm-facts:${name}`;
    let facts = this.cache.get<NpmPackageFacts>(cacheKey);
    if (facts === null) {
      if (!this.rateLimiter.canProceed("npm")) {
        facts = this.cache.getStale<NpmPackageFacts>(cacheKey)?.data ?? null;
        if (!facts) return errorResult(name, currentVersion, isDev, "Rate limited");
      } else {
        try {
          this.rateLimiter.consume("npm");
          const meta = await fetchJson<NpmAbbreviatedMeta>(
            `${NPM_REGISTRY_URL}/${encodeURIComponent(name)}`,
            { headers: { Accept: "application/vnd.npm.install-v1+json" } },
            NPM_TIMEOUT_MS,
          );
          facts = packageFacts(meta);
          this.cache.set(cacheKey, facts, "npm", NPM_CACHE_TTL);
        } catch (err) {
          facts = this.cache.getStale<NpmPackageFacts>(cacheKey)?.data ?? null;
          if (!facts) return errorResult(name, currentVersion, isDev, err instanceof Error ? err.message : String(err));
        }
      }
    }
    return infoFor(name, currentVersion, isDev, facts);
  }

  async getBulkDownloads(names: string[]): Promise<Map<string, number>> {
    const result = new Map<string, number>();
    if (names.length === 0) return result;

    // npm's bulk endpoint rejects scoped packages outright — a single "@scope/pkg"
    // in a comma-joined request fails the WHOLE batch ("scoped packages are not
    // currently supported in bulk lookups"), which previously zeroed downloads for
    // every package in the group. Split them: unscoped go through the bulk endpoint,
    // scoped are fetched individually (the single-package endpoint accepts them).
    const scoped = names.filter((n) => n.startsWith("@"));
    const unscoped = names.filter((n) => !n.startsWith("@"));

    for (let i = 0; i < unscoped.length; i += DOWNLOADS_BATCH_SIZE) {
      if (!this.rateLimiter.canProceed("npm")) break;
      const batch = unscoped.slice(i, i + DOWNLOADS_BATCH_SIZE);
      try {
        this.rateLimiter.consume("npm");
        const url = `${NPM_DOWNLOADS_URL}/${batch.map((n) => encodeURIComponent(n)).join(",")}`;
        const response = await fetchWithTimeout(url, {}, NPM_TIMEOUT_MS);
        if (!response.ok) continue;
        const data = (await response.json()) as NpmDownloadsResponse;
        for (const [pkg, info] of Object.entries(data)) {
          if (info && typeof info.downloads === "number") {
            result.set(pkg, info.downloads);
          }
        }
      } catch {
        // Batch failure — continue with remaining batches.
      }
    }

    // Scoped packages, one request each. The "@scope/pkg" path is sent literally
    // (encoding the "/" breaks it); the single endpoint returns { downloads, ... }.
    for (const name of scoped) {
      if (!this.rateLimiter.canProceed("npm")) break;
      try {
        this.rateLimiter.consume("npm");
        const response = await fetchWithTimeout(`${NPM_DOWNLOADS_URL}/${name}`, {}, NPM_TIMEOUT_MS);
        if (!response.ok) continue;
        const data = (await response.json()) as { downloads?: number };
        if (typeof data.downloads === "number") result.set(name, data.downloads);
      } catch {
        // Skip this package's downloads — best-effort signal.
      }
    }

    return result;
  }
}

/** What the registry says about a package, independent of which version is installed. */
interface NpmPackageFacts {
  latestTag: string | null;
  latestStable: string | null;
  modified: string | null;
  /** Deprecation message per deprecated version (only those). */
  deprecations: Record<string, string>;
}

function packageFacts(meta: NpmAbbreviatedMeta): NpmPackageFacts {
  const deprecations: Record<string, string> = {};
  for (const [version, entry] of Object.entries(meta.versions || {})) {
    if (entry?.deprecated) deprecations[version] = entry.deprecated;
  }
  return {
    latestTag: meta["dist-tags"]?.latest ?? null,
    // Semver-max, not last-published: maintenance releases of older lines are
    // published after newer lines (see maxStableSemver).
    latestStable: maxStableSemver(Object.keys(meta.versions || {})),
    modified: meta.modified ?? null,
    deprecations,
  };
}

/**
 * The answer for one installed version. Deprecation is the INSTALLED
 * version's first (that is what the project runs): `mkdirp@0.5.1` is
 * deprecated on npm while its latest is not, and checking only the latest
 * entry reported it healthy. A deprecated latest still marks the whole
 * package deprecated.
 */
function infoFor(name: string, currentVersion: string | null, isDev: boolean, facts: NpmPackageFacts): RegistryPackageInfo {
  const installedDeprecation = currentVersion ? facts.deprecations[currentVersion] : undefined;
  const latestDeprecation = facts.latestTag ? facts.deprecations[facts.latestTag] : undefined;
  const target = facts.latestStable || facts.latestTag;
  const versionsBehind: SemverDistance | null =
    currentVersion && target ? computeSemverDistance(currentVersion, target) : null;
  return {
    name,
    ecosystem: "npm",
    currentVersion,
    latestVersion: facts.latestTag,
    latestStableVersion: facts.latestStable,
    versionsBehind,
    deprecated: Boolean(installedDeprecation || latestDeprecation),
    deprecationMessage: installedDeprecation ?? latestDeprecation ?? null,
    lastPublished: facts.modified,
    license: null, // abbreviated metadata does not include license
    weeklyDownloads: null, // fetched separately via getBulkDownloads
    isDev,
    fetchError: null,
  };
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
