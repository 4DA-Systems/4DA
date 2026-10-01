// SPDX-License-Identifier: Apache-2.0
/**
 * npm full-packument reader for dependency_check.
 *
 * The abbreviated metadata the freshness reader uses carries no publish times,
 * publishers, attestations, scripts or per-version dependencies, so this reads
 * the FULL packument (`GET registry.npmjs.org/<name>`) and keeps only what the
 * signals need. Some packuments are ~15 MB, so the projection is cached on
 * disk (the live_cache table) together with the registry's ETag, and later
 * reads revalidate with `If-None-Match` — an unchanged package costs a 304.
 *
 * Privacy: the request names the package and nothing else. The installed
 * version is never sent; versions are picked out of the full list locally.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import { fetchWithTimeout } from "./http-utils.js";
import type {
  PackageReleaseIndex,
  RegistryLookup,
  ReleaseMetadataSource,
  ReleaseRecord,
  TrustLevel,
} from "./release-metadata.js";

const NPM_REGISTRY_URL = "https://registry.npmjs.org";
// Full packuments can be large; 8 s (the abbreviated reader's budget) is not enough.
const PACKUMENT_TIMEOUT_MS = 30_000;
/** Serve the cached projection without revalidating for this long. */
const FRESH_MS = 15 * 60 * 1000;
/** Keep the row (and its ETag) this long so revalidation survives restarts. */
const ROW_TTL_SECONDS = 7 * 86_400;
const INSTALL_HOOKS = ["preinstall", "install", "postinstall"] as const;

interface CachedPackument {
  etag: string | null;
  fetchedAtMs: number;
  index: PackageReleaseIndex;
}

interface NpmVersionDoc {
  _npmUser?: { name?: string; trustedPublisher?: { id?: string } | null };
  dist?: { attestations?: unknown };
  scripts?: Record<string, string>;
  gypfile?: boolean;
  hasInstallScript?: boolean;
  dependencies?: Record<string, string>;
  optionalDependencies?: Record<string, string>;
  deprecated?: string;
}

export interface NpmPackument {
  versions?: Record<string, NpmVersionDoc>;
  time?: Record<string, unknown>;
}

export class NpmPackumentReader implements ReleaseMetadataSource {
  constructor(
    private cache: LiveCache,
    private rateLimiter: RateLimiter,
  ) {}

  async getReleases(name: string): Promise<RegistryLookup<PackageReleaseIndex>> {
    const key = `npm-packument:${name}`;
    const cached = this.cache.getStale<CachedPackument>(key)?.data ?? null;
    if (cached && Date.now() - cached.fetchedAtMs < FRESH_MS) {
      return { status: "ok", data: cached.index };
    }
    if (!this.rateLimiter.consume("npm")) {
      return { status: "unreachable", detail: "npm registry rate limit reached for this minute" };
    }

    const headers: Record<string, string> = { Accept: "application/json" };
    if (cached?.etag) headers["If-None-Match"] = cached.etag;

    let response: Response;
    try {
      response = await fetchWithTimeout(
        `${NPM_REGISTRY_URL}/${encodeURIComponent(name)}`,
        { headers },
        PACKUMENT_TIMEOUT_MS,
      );
    } catch (err) {
      return { status: "unreachable", detail: `npm registry: ${errorText(err)}` };
    }

    if (response.status === 304 && cached) {
      this.cache.set(key, { ...cached, fetchedAtMs: Date.now() }, "npm", ROW_TTL_SECONDS);
      return { status: "ok", data: cached.index };
    }
    if (response.status === 404) {
      return { status: "not_found", detail: `"${name}" is not on the public npm registry` };
    }
    if (!response.ok) {
      return { status: "unreachable", detail: `npm registry answered HTTP ${response.status}` };
    }

    let doc: NpmPackument;
    try {
      doc = (await response.json()) as NpmPackument;
    } catch (err) {
      return { status: "unreachable", detail: `npm registry: unreadable packument (${errorText(err)})` };
    }
    const index = projectPackument(name, doc);
    this.cache.set(
      key,
      { etag: response.headers.get("etag"), fetchedAtMs: Date.now(), index } satisfies CachedPackument,
      "npm",
      ROW_TTL_SECONDS,
    );
    return { status: "ok", data: index };
  }

  async getCreatedAt(name: string): Promise<RegistryLookup<string | null>> {
    const lookup = await this.getReleases(name);
    return lookup.status === "ok" ? { status: "ok", data: lookup.data.createdAt } : lookup;
  }
}

/** Keep only what dependency_check reads. Exported for tests. */
export function projectPackument(name: string, doc: NpmPackument): PackageReleaseIndex {
  const time = doc.time ?? {};
  const releases: Record<string, ReleaseRecord> = {};

  for (const [version, v] of Object.entries(doc.versions ?? {})) {
    const { trust, trustEvidence } = npmTrust(v);
    releases[version] = {
      version,
      publishedAt: typeof time[version] === "string" ? (time[version] as string) : null,
      trust,
      trustEvidence,
      installScripts: installScriptsOf(v),
      deprecated: typeof v.deprecated === "string" && v.deprecated.length > 0 ? v.deprecated : null,
      withdrawn: null,
      dependencies: [
        ...new Set([...Object.keys(v.dependencies ?? {}), ...Object.keys(v.optionalDependencies ?? {})]),
      ],
    };
  }

  // A version listed in `time` but absent from `versions` was unpublished —
  // npm keeps the timestamp so the number can never be reused. The 2026-03-31
  // axios 1.14.1 / 0.30.4 releases look exactly like this today.
  const unpublished = new Set<string>();
  for (const k of Object.keys(time)) {
    if (k !== "created" && k !== "modified" && k !== "unpublished" && !releases[k]) unpublished.add(k);
  }
  const whole = time.unpublished as { versions?: unknown } | undefined;
  if (whole && Array.isArray(whole.versions)) {
    for (const v of whole.versions) if (typeof v === "string" && !releases[v]) unpublished.add(v);
  }
  for (const version of unpublished) {
    releases[version] = {
      version,
      publishedAt: typeof time[version] === "string" ? (time[version] as string) : null,
      trust: 0,
      trustEvidence: "unknown (release unpublished; its metadata is gone)",
      installScripts: null,
      deprecated: null,
      withdrawn: "unpublished",
      dependencies: null,
    };
  }

  return {
    ecosystem: "npm",
    name,
    createdAt: typeof time.created === "string" ? time.created : null,
    releases,
  };
}

function npmTrust(v: NpmVersionDoc): { trust: TrustLevel; trustEvidence: string } {
  const tp = v._npmUser?.trustedPublisher;
  if (tp) return { trust: 2, trustEvidence: `trusted publisher (${tp.id ?? "OIDC"})` };
  if (v.dist?.attestations) return { trust: 1, trustEvidence: "provenance attestation, no trusted publisher" };
  const who = v._npmUser?.name ? ` (published by npm user "${v._npmUser.name}")` : "";
  return { trust: 0, trustEvidence: `no trusted publisher or provenance attestation${who}` };
}

function installScriptsOf(v: NpmVersionDoc): string[] {
  const found: string[] = INSTALL_HOOKS.filter((hook) => typeof v.scripts?.[hook] === "string");
  // npm runs `node-gyp rebuild` as an implicit install script when the package
  // ships a binding.gyp and declares no install/preinstall of its own.
  if (v.gypfile && !found.includes("install") && !found.includes("preinstall")) {
    found.push("install (implicit node-gyp rebuild)");
  }
  if (found.length === 0 && v.hasInstallScript) found.push("install script (hasInstallScript)");
  return found;
}

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
