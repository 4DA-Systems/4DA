// SPDX-License-Identifier: Apache-2.0
/**
 * OSV.dev Vulnerability Scanner
 *
 * Batch-queries the Open Source Vulnerability database for known CVEs
 * affecting the user's exact dependency versions. Free, no auth required.
 *
 * Privacy: sends only package names + versions (public manifest data).
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchWithTimeout } from "./http-utils.js";
import type {
  ResolvedDependency,
  OsvVulnerability,
  VulnerabilityEntry,
  VulnerabilityScanResult,
} from "./types.js";

const OSV_BATCH_URL = "https://api.osv.dev/v1/querybatch";
const OSV_TIMEOUT_MS = 15_000;
const OSV_CACHE_TTL = 3600; // 1 hour
const MAX_BATCH_SIZE = 1000;

interface OsvBatchQuery {
  package: { name: string; ecosystem: string };
  version?: string;
}

interface OsvBatchResponse {
  results: Array<{ vulns?: OsvVulnerability[] }>;
}

export class OsvScanner {
  private cache: LiveCache;
  private rateLimiter: RateLimiter;

  constructor(cache: LiveCache, rateLimiter: RateLimiter) {
    this.cache = cache;
    this.rateLimiter = rateLimiter;
  }

  async scan(deps: ResolvedDependency[], projectPath: string): Promise<VulnerabilityScanResult> {
    const start = Date.now();
    const scannable = deps.filter((d) => d.version !== null);
    const ecosystems = [...new Set(scannable.map((d) => d.ecosystem))];

    // Check cache for each dep individually
    const uncached: ResolvedDependency[] = [];
    const cachedVulns: VulnerabilityEntry[] = [];

    for (const dep of scannable) {
      const cacheKey = `osv:${dep.ecosystem}:${dep.name}:${dep.version}`;
      const cached = this.cache.get<VulnerabilityEntry[]>(cacheKey);
      if (cached !== null) {
        cachedVulns.push(...cached);
      } else {
        uncached.push(dep);
      }
    }

    // Fetch uncached from OSV
    let fetchedVulns: VulnerabilityEntry[] = [];
    let offline = false;

    if (uncached.length > 0) {
      if (!this.rateLimiter.canProceed("osv")) {
        offline = true;
      } else {
        try {
          fetchedVulns = await this.batchQuery(uncached);
        } catch {
          offline = true;
          // Try stale cache for uncached deps
          for (const dep of uncached) {
            const cacheKey = `osv:${dep.ecosystem}:${dep.name}:${dep.version}`;
            const stale = this.cache.getStale<VulnerabilityEntry[]>(cacheKey);
            if (stale) cachedVulns.push(...stale.data);
          }
        }
      }
    }

    const allVulns = [...cachedVulns, ...fetchedVulns];
    const vulnerablePackages = new Set(allVulns.map((v) => v.package));

    const bySeverity = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
    for (const v of allVulns) {
      bySeverity[v.severity]++;
    }

    return {
      scannedAt: new Date().toISOString(),
      projectPath,
      ecosystemsScanned: ecosystems,
      totalScanned: scannable.length,
      totalVulnerable: vulnerablePackages.size,
      bySeverity,
      vulnerabilities: allVulns,
      cleanCount: offline ? 0 : scannable.length - vulnerablePackages.size,
      scanDurationMs: Date.now() - start,
      cached: uncached.length === 0,
      offline,
    };
  }

  private async batchQuery(deps: ResolvedDependency[]): Promise<VulnerabilityEntry[]> {
    const results: VulnerabilityEntry[] = [];

    for (let offset = 0; offset < deps.length; offset += MAX_BATCH_SIZE) {
      if (!this.rateLimiter.canProceed("osv")) {
        throw new Error("OSV rate limit reached before all dependency batches completed");
      }
      const chunk = deps.slice(offset, offset + MAX_BATCH_SIZE);
      const queries: OsvBatchQuery[] = chunk.map((d) => ({
        package: { name: d.name, ecosystem: d.ecosystem },
        ...(d.version ? { version: d.version } : {}),
      }));

      const response = await fetchWithTimeout(OSV_BATCH_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ queries }),
      }, OSV_TIMEOUT_MS);

      if (!response.ok) {
        throw new Error(`OSV API error: ${response.status}`);
      }
      this.rateLimiter.consume("osv");

      const data = (await response.json()) as OsvBatchResponse;
      for (let i = 0; i < chunk.length; i++) {
        const dep = chunk[i];
        const osvResult = data.results[i];
        const depVulns: VulnerabilityEntry[] = [];

        if (osvResult?.vulns && osvResult.vulns.length > 0) {
          for (const vuln of osvResult.vulns) {
            depVulns.push(mapVulnerability(vuln, dep));
          }
        }

        // Cache per-dep (even empty results to avoid re-fetching clean deps)
        const cacheKey = `osv:${dep.ecosystem}:${dep.name}:${dep.version}`;
        this.cache.set(cacheKey, depVulns, "osv", OSV_CACHE_TTL);
        results.push(...depVulns);
      }
    }

    return results;
  }
}

function mapVulnerability(vuln: OsvVulnerability, dep: ResolvedDependency): VulnerabilityEntry {
  const cvssScore = extractCvssScore(vuln.severity);
  const severity = cvssScore !== null ? cvssToSeverity(cvssScore) : guessSeverityFromId(vuln.id);
  const fixedVersion = extractFixedVersion(vuln.affected, dep.name, dep.ecosystem);

  return {
    package: dep.name,
    currentVersion: dep.version || "unknown",
    ecosystem: dep.ecosystem,
    isDev: dep.isDev,
    isDirect: dep.isDirect,
    devScopeKnown: dep.devScopeKnown,
    vulnId: vuln.id,
    aliases: vuln.aliases || [],
    severity,
    cvssScore,
    summary: vuln.summary || vuln.details?.slice(0, 200) || vuln.id,
    fixedVersion,
    published: vuln.published || vuln.modified || "",
    references: (vuln.references || [])
      .filter((r) => r.type === "ADVISORY" || r.type === "WEB")
      .map((r) => r.url)
      .slice(0, 3),
  };
}

export function extractCvssScore(severity: Array<{ type: string; score: string }> | undefined): number | null {
  if (!severity || severity.length === 0) return null;

  // OSV commonly returns a CVSS vector rather than a numeric base score.
  // Without a full CVSS calculator, treating the vector's "3.1" version as
  // the score is worse than leaving severity unknown.
  for (const s of severity) {
    const raw = s.score.trim();
    if (!/^\d+(?:\.\d+)?$/.test(raw)) continue;
    const num = Number(raw);
    if (!isNaN(num) && num >= 0 && num <= 10) return num;
  }
  return null;
}

function cvssToSeverity(score: number): "critical" | "high" | "medium" | "low" | "unknown" {
  if (score >= 9.0) return "critical";
  if (score >= 7.0) return "high";
  if (score >= 4.0) return "medium";
  if (score > 0) return "low";
  return "unknown";
}

function guessSeverityFromId(id: string): "critical" | "high" | "medium" | "low" | "unknown" {
  // GHSA severity is sometimes encoded in the advisory
  if (id.startsWith("GHSA-")) return "medium"; // conservative default
  return "unknown";
}

function extractFixedVersion(
  affected: OsvVulnerability["affected"] | undefined,
  packageName: string,
  ecosystem: string,
): string | null {
  if (!affected) return null;
  for (const a of affected) {
    if (a.package.name === packageName && a.package.ecosystem === ecosystem) {
      for (const range of a.ranges || []) {
        for (const event of range.events || []) {
          if (event.fixed) return event.fixed;
        }
      }
    }
  }
  return null;
}
