// SPDX-License-Identifier: Apache-2.0
/**
 * crates.io release reader for dependency_check.
 *
 * Three name-only reads:
 * - `crates.io/api/v1/crates/<name>/versions` (paged, newest semver first):
 *   publish time, publisher, `trustpub_data`, yanked.
 * - the sparse index (`index.crates.io`, the CDN file cargo itself reads):
 *   each version's dependency list, which the versions endpoint lacks.
 * - `crates.io/api/v1/crates/<name>`: when a newly introduced dependency
 *   was first published.
 *
 * crates.io asks API clients for a descriptive User-Agent and at most one
 * request per second; every API request here waits for its slot. Paging
 * stops as soon as the versions the caller asked about (and the release just
 * below them) have been seen, so a check costs one or two pages, not the
 * crate's whole history.
 *
 * Privacy: requests carry the crate NAME only, never the installed version.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchWithTimeout } from "./http-utils.js";
import { compareVersionPrecedence, parseSemverPrecedence } from "./semver-precedence.js";
import type {
  PackageReleaseIndex,
  RegistryLookup,
  ReleaseMetadataSource,
  ReleaseRecord,
} from "./release-metadata.js";

const CRATES_API = "https://crates.io/api/v1/crates";
export const CRATES_USER_AGENT = "4DA-MCP-Server/1.0 (https://4da.ai)";
const TIMEOUT_MS = 15_000;
const MAX_PAGES = 25;
const VERSIONS_TTL = 3600;
const CREATED_TTL = 30 * 86_400; // a crate's creation date never changes

interface CratesVersion {
  num: string;
  created_at?: string;
  yanked?: boolean;
  published_by?: { login?: string } | null;
  trustpub_data?: { provider?: string; repository?: string } | null;
}

interface CratesVersionsPage {
  versions?: CratesVersion[];
  meta?: { next_page?: string | null };
}

interface SparseLine {
  vers: string;
  deps?: Array<{ name: string; kind?: string | null; package?: string }>;
}

class HttpStatusError extends Error {
  constructor(readonly status: number) {
    super(`HTTP ${status}`);
  }
}

export class CratesVersionsReader implements ReleaseMetadataSource {
  /** Earliest time the next crates.io API request may start. Reserved synchronously so concurrent callers queue. */
  private nextSlotMs = 0;

  constructor(
    private cache: LiveCache,
    private rateLimiter: RateLimiter,
    /** crates.io's crawler policy: one request per second. Tests pass 0. */
    private minIntervalMs = 1000,
  ) {}

  async getReleases(name: string, wanted: string[]): Promise<RegistryLookup<PackageReleaseIndex>> {
    const key = `crates-versions:${name}`;
    const cached = this.cache.get<{ index: PackageReleaseIndex; complete: boolean }>(key);
    if (cached && (cached.complete || pagesSatisfy(Object.keys(cached.index.releases), wanted))) {
      return { status: "ok", data: cached.index };
    }

    const releases: Record<string, ReleaseRecord> = {};
    let query: string | null = "?per_page=100&sort=semver";
    let pages = 0;
    try {
      while (query && pages < MAX_PAGES) {
        const page: CratesVersionsPage = await this.apiGet<CratesVersionsPage>(
          `${CRATES_API}/${encodeURIComponent(name)}/versions${query}`,
        );
        pages++;
        for (const v of page.versions ?? []) releases[v.num] = toRecord(v);
        query = page.meta?.next_page ?? null;
        if (query && pagesSatisfy(Object.keys(releases), wanted)) break;
      }
    } catch (err) {
      if (err instanceof HttpStatusError && err.status === 404) {
        return { status: "not_found", detail: `"${name}" is not on crates.io` };
      }
      return { status: "unreachable", detail: `crates.io: ${errorText(err)}` };
    }

    // Dependency lists come from the sparse index; without them the
    // new-dependency signal reports "unknown" rather than "none".
    const deps = await this.sparseDependencies(name);
    if (deps) {
      for (const [version, names] of Object.entries(deps)) {
        if (releases[version]) releases[version].dependencies = names;
      }
    }

    const index: PackageReleaseIndex = { ecosystem: "crates.io", name, createdAt: null, releases };
    this.cache.set(key, { index, complete: query === null }, "crates", VERSIONS_TTL);
    return { status: "ok", data: index };
  }

  async getCreatedAt(name: string): Promise<RegistryLookup<string | null>> {
    const key = `crates-created:${name}`;
    const cached = this.cache.get<{ createdAt: string | null }>(key);
    if (cached) return { status: "ok", data: cached.createdAt };
    try {
      const body = await this.apiGet<{ crate?: { created_at?: string } }>(
        `${CRATES_API}/${encodeURIComponent(name)}`,
      );
      const createdAt = body.crate?.created_at ?? null;
      this.cache.set(key, { createdAt }, "crates", CREATED_TTL);
      return { status: "ok", data: createdAt };
    } catch (err) {
      if (err instanceof HttpStatusError && err.status === 404) {
        return { status: "not_found", detail: `"${name}" is not on crates.io` };
      }
      return { status: "unreachable", detail: `crates.io: ${errorText(err)}` };
    }
  }

  private async apiGet<T>(url: string): Promise<T> {
    if (!this.rateLimiter.consume("crates")) throw new Error("crates.io rate limit reached for this minute");
    await this.waitForSlot();
    const response = await fetchWithTimeout(
      url,
      { headers: { Accept: "application/json", "User-Agent": CRATES_USER_AGENT } },
      TIMEOUT_MS,
    );
    if (!response.ok) throw new HttpStatusError(response.status);
    return (await response.json()) as T;
  }

  private async waitForSlot(): Promise<void> {
    const now = Date.now();
    const start = Math.max(now, this.nextSlotMs);
    this.nextSlotMs = start + this.minIntervalMs;
    if (start > now) await new Promise((resolve) => setTimeout(resolve, start - now));
  }

  /** version -> runtime dependency names (normal + build; renamed deps by their real crate name). */
  private async sparseDependencies(name: string): Promise<Record<string, string[]> | null> {
    const key = `crates-sparse-deps:${name}`;
    const cached = this.cache.get<Record<string, string[]>>(key);
    if (cached) return cached;
    try {
      const response = await fetchWithTimeout(
        sparseIndexUrl(name),
        { headers: { "User-Agent": CRATES_USER_AGENT } },
        TIMEOUT_MS,
      );
      if (!response.ok) return null;
      const out: Record<string, string[]> = {};
      for (const line of (await response.text()).split("\n")) {
        if (!line.trim()) continue;
        try {
          const entry = JSON.parse(line) as SparseLine;
          const names = (entry.deps ?? [])
            .filter((d) => d.kind !== "dev")
            .map((d) => d.package ?? d.name);
          out[entry.vers] = [...new Set(names)];
        } catch {
          // skip a malformed line
        }
      }
      this.cache.set(key, out, "crates", VERSIONS_TTL);
      return out;
    } catch {
      return null;
    }
  }
}

function toRecord(v: CratesVersion): ReleaseRecord {
  const tp = v.trustpub_data;
  const who = v.published_by?.login ? ` (published by crates.io user "${v.published_by.login}")` : "";
  return {
    version: v.num,
    publishedAt: v.created_at ?? null,
    trust: tp ? 2 : 0,
    trustEvidence: tp
      ? `trusted publisher (${[tp.provider, tp.repository].filter(Boolean).join(" ") || "OIDC"})`
      : `no trusted publisher${who}`,
    installScripts: null,
    deprecated: null,
    withdrawn: v.yanked ? "yanked" : null,
    dependencies: null,
  };
}

/**
 * True once every wanted version is present AND a release (not a prerelease)
 * below all of them has been seen. Pages arrive in descending semver order,
 * so at that point the release just below the target on its line — the
 * new-dependency trust baseline — has been seen too, if it exists.
 */
export function pagesSatisfy(seen: string[], wanted: string[]): boolean {
  if (wanted.length === 0) return false;
  const have = new Set(seen);
  if (!wanted.every((w) => have.has(w))) return false;
  return seen.some((v) => {
    const p = parseSemverPrecedence(v);
    if (!p || p.prerelease.length > 0) return false;
    return wanted.every((w) => (compareVersionPrecedence(v, w) ?? 0) < 0);
  });
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

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
