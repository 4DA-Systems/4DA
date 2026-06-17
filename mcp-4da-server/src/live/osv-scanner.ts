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
import { fetchWithTimeout, fetchJson } from "./http-utils.js";
import { cvssBaseScore } from "./cvss.js";
import type {
  ResolvedDependency,
  OsvVulnerability,
  VulnerabilityEntry,
  VulnerabilityScanResult,
} from "./types.js";

const OSV_BATCH_URL = "https://api.osv.dev/v1/querybatch";
const OSV_VULN_URL = "https://api.osv.dev/v1/vulns/";
const OSV_TIMEOUT_MS = 15_000;
const OSV_CACHE_TTL = 3600; // 1 hour
const OSV_DETAIL_TTL = 86_400; // 24 hours — advisory details change rarely
const MAX_BATCH_SIZE = 1000;
// Bounded concurrency for advisory-detail hydration. Only the *vulnerable*
// subset is hydrated (not every scanned dep), so this stays small in practice.
const HYDRATE_CONCURRENCY = 8;

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
    const platformInactiveVulnerable = new Set(
      allVulns.filter((v) => !v.platformActive).map((v) => v.package),
    ).size;

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
      platformInactiveVulnerable,
      bySeverity,
      vulnerabilities: allVulns,
      cleanCount: offline ? 0 : scannable.length - vulnerablePackages.size,
      scanDurationMs: Date.now() - start,
      cached: uncached.length === 0,
      offline,
    };
  }

  private async batchQuery(deps: ResolvedDependency[]): Promise<VulnerabilityEntry[]> {
    // Phase 1 — enumerate advisory IDs per dep via the lightweight batch index.
    // querybatch returns only `{ id, modified }`; the rich fields (severity,
    // fixed version, summary, references) come from per-advisory hydration below.
    const matches: Array<{ dep: ResolvedDependency; ids: string[] }> = [];

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
        const ids = (data.results[i]?.vulns ?? []).map((v) => v.id);
        matches.push({ dep: chunk[i], ids });
      }
    }

    // Phase 2 — hydrate each unique advisory once (cached, bounded concurrency).
    const uniqueIds = [...new Set(matches.flatMap((m) => m.ids))];
    const detailById = await this.hydrateDetails(uniqueIds);

    // Phase 3 — map to entries and cache per-dep (empty results cached too, so
    // clean deps are not re-queried).
    const results: VulnerabilityEntry[] = [];
    for (const { dep, ids } of matches) {
      const depVulns = ids.map((id) => mapVulnerability(detailById.get(id) ?? { id }, dep));
      const cacheKey = `osv:${dep.ecosystem}:${dep.name}:${dep.version}`;
      this.cache.set(cacheKey, depVulns, "osv", OSV_CACHE_TTL);
      results.push(...depVulns);
    }

    return results;
  }

  /**
   * Hydrate advisory detail for the given IDs. Cache-first (24h), then fetch the
   * misses from `/v1/vulns/{id}` with bounded concurrency. A failed fetch is
   * skipped (not fatal) — the caller falls back to the index-only shape so a
   * real advisory is never silently dropped just because its detail 404'd.
   */
  private async hydrateDetails(ids: string[]): Promise<Map<string, OsvVulnerability>> {
    const out = new Map<string, OsvVulnerability>();
    const toFetch: string[] = [];

    for (const id of ids) {
      const cached = this.cache.get<OsvVulnerability>(`osv:detail:${id}`);
      if (cached) out.set(id, cached);
      else toFetch.push(id);
    }

    for (let i = 0; i < toFetch.length; i += HYDRATE_CONCURRENCY) {
      const slice = toFetch.slice(i, i + HYDRATE_CONCURRENCY);
      const settled = await Promise.allSettled(slice.map((id) => this.fetchDetail(id)));
      settled.forEach((res, j) => {
        if (res.status === "fulfilled" && res.value) {
          const id = slice[j];
          out.set(id, res.value);
          this.cache.set(`osv:detail:${id}`, res.value, "osv", OSV_DETAIL_TTL);
        }
      });
    }

    return out;
  }

  private async fetchDetail(id: string): Promise<OsvVulnerability | null> {
    try {
      return await fetchJson<OsvVulnerability>(
        `${OSV_VULN_URL}${encodeURIComponent(id)}`,
        {},
        OSV_TIMEOUT_MS,
      );
    } catch {
      return null;
    }
  }
}

function mapVulnerability(
  vuln: Partial<OsvVulnerability> & { id: string },
  dep: ResolvedDependency,
): VulnerabilityEntry {
  const { severity, cvssScore } = deriveSeverity(vuln);
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
    target: dep.target,
    platformActive: dep.platformActive,
  };
}

/**
 * Derive a severity bucket honestly: a CVSS base score (numeric or computed
 * from a vector) wins; otherwise the GitHub-advisory label; otherwise unknown.
 * Never fabricates a default bucket — an advisory with no severity signal is
 * reported as `unknown`, not silently labeled "medium".
 */
function deriveSeverity(vuln: Partial<OsvVulnerability>): {
  severity: VulnerabilityEntry["severity"];
  cvssScore: number | null;
} {
  const cvssScore = extractCvssScore(vuln.severity);
  if (cvssScore !== null) return { severity: cvssToSeverity(cvssScore), cvssScore };

  const label = labelToSeverity(vuln.database_specific?.severity);
  if (label !== "unknown") return { severity: label, cvssScore: null };

  return { severity: "unknown", cvssScore: null };
}

function labelToSeverity(label: string | undefined): VulnerabilityEntry["severity"] {
  switch ((label || "").toUpperCase()) {
    case "CRITICAL":
      return "critical";
    case "HIGH":
      return "high";
    case "MODERATE":
    case "MEDIUM":
      return "medium";
    case "LOW":
      return "low";
    default:
      return "unknown";
  }
}

export function extractCvssScore(severity: Array<{ type: string; score: string }> | undefined): number | null {
  if (!severity || severity.length === 0) return null;

  // Prefer an explicit numeric base score when present.
  for (const s of severity) {
    const raw = s.score.trim();
    if (/^\d+(?:\.\d+)?$/.test(raw)) {
      const num = Number(raw);
      if (!isNaN(num) && num >= 0 && num <= 10) return num;
    }
  }

  // OSV most often gives a CVSS *vector* string — compute its base score.
  for (const s of severity) {
    if (s.score.includes("AV:")) {
      const score = cvssBaseScore(s.score);
      if (score !== null) return score;
    }
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
