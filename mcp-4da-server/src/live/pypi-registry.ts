// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * PyPI Registry Fetcher
 *
 * Queries the PyPI JSON API for package metadata (latest version, deprecation
 * status, semver distance, license, last published date).
 *
 * Privacy: sends only package names -- public manifest data.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchJson } from "./http-utils.js";
import { computeSemverDistance, isPreRelease } from "./semver-utils.js";
import type { RegistryPackageInfo } from "./types.js";

const PYPI_BASE_URL = "https://pypi.org/pypi";
const PYPI_TIMEOUT_MS = 8_000;
const PYPI_CACHE_TTL = 86_400; // 24 hours

interface PyPIReleaseFile {
  upload_time_iso_8601: string;
  yanked: boolean;
}

interface PyPIResponse {
  info: {
    version: string;
    license: string;
    classifiers: string[];
    name: string;
    summary: string;
    yanked: boolean;
    yanked_reason: string | null;
  };
  releases: Record<string, PyPIReleaseFile[]>;
}

export class PyPIRegistry {
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
    const normalizedName = name.toLowerCase().replace(/_/g, "-");
    const cacheKey = `pypi-reg:${name.toLowerCase()}`;
    const cached = this.cache.get<RegistryPackageInfo>(cacheKey);
    if (cached !== null) return cached;

    if (!this.rateLimiter.canProceed("pypi")) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      return errorResult(name, currentVersion, isDev, "Rate limited");
    }

    try {
      this.rateLimiter.consume("pypi");

      const data = await fetchJson<PyPIResponse>(
        `${PYPI_BASE_URL}/${encodeURIComponent(normalizedName)}/json`,
        {},
        PYPI_TIMEOUT_MS,
      );

      const latestVersion = data.info.version ?? null;

      // Find latest stable: iterate release keys in reverse, skip yanked and prerelease
      const releaseVersions = Object.keys(data.releases);
      const latestStable = findLatestStable(releaseVersions, data.releases);

      // License: prefer classifiers over free-text info.license
      const license = extractLicenseFromClassifiers(data.info.classifiers)
        ?? truncateLicense(data.info.license);

      // Last published: max upload_time_iso_8601 across files of the latest version
      const lastPublished = latestVersion
        ? getLastPublished(data.releases[latestVersion])
        : null;

      // Deprecation: yanked at the package info level
      const deprecated = !!data.info.yanked;
      const deprecationMessage = deprecated
        ? (data.info.yanked_reason || "Package has been yanked")
        : null;

      // Semver distance
      let versionsBehind = null;
      if (currentVersion && (latestStable || latestVersion)) {
        versionsBehind = computeSemverDistance(
          currentVersion,
          latestStable || latestVersion!,
        );
      }

      const result: RegistryPackageInfo = {
        name,
        ecosystem: "PyPI",
        currentVersion,
        latestVersion,
        latestStableVersion: latestStable,
        versionsBehind,
        deprecated,
        deprecationMessage,
        lastPublished,
        license,
        weeklyDownloads: null, // PyPI JSON API does not include download counts
        isDev,
        fetchError: null,
      };

      this.cache.set(cacheKey, result, "pypi", PYPI_CACHE_TTL);
      return result;
    } catch (err) {
      const stale = this.cache.getStale<RegistryPackageInfo>(cacheKey);
      if (stale) return stale.data;
      const message = err instanceof Error ? err.message : String(err);
      return errorResult(name, currentVersion, isDev, message);
    }
  }
}

function findLatestStable(
  versions: string[],
  releases: Record<string, PyPIReleaseFile[]>,
): string | null {
  for (let i = versions.length - 1; i >= 0; i--) {
    const v = versions[i];
    if (isPreRelease(v)) continue;

    // Skip if all files for this version are yanked
    const files = releases[v];
    if (files && files.length > 0 && files.every((f) => f.yanked)) continue;

    return v;
  }
  return null;
}

function getLastPublished(files: PyPIReleaseFile[] | undefined): string | null {
  if (!files || files.length === 0) return null;
  let latest = "";
  for (const f of files) {
    if (f.upload_time_iso_8601 > latest) {
      latest = f.upload_time_iso_8601;
    }
  }
  return latest || null;
}

function extractLicenseFromClassifiers(classifiers: string[]): string | null {
  for (const c of classifiers) {
    if (c.startsWith("License :: OSI Approved :: ")) {
      return c.replace("License :: OSI Approved :: ", "").replace(" License", "");
    }
  }
  return null;
}

function truncateLicense(license: string | null | undefined): string | null {
  if (!license || license.trim().length === 0) return null;
  const trimmed = license.trim();
  if (trimmed.length <= 50) return trimmed;
  return trimmed.slice(0, 47) + "...";
}

function errorResult(
  name: string,
  currentVersion: string | null,
  isDev: boolean,
  fetchError: string,
): RegistryPackageInfo {
  return {
    name,
    ecosystem: "PyPI",
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
