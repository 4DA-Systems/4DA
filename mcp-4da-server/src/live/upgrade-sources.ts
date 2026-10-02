// SPDX-License-Identifier: Apache-2.0
/**
 * Network sources for `upgrade_impact`: registry version index, the target
 * version's changelog (read from its registry archive) and OSV advisories.
 *
 * Privacy rule (Decision 1, option A): the only hosts contacted are the
 * package's own registry (registry.npmjs.org; crates.io + static.crates.io)
 * and api.osv.dev. A changelog is never fetched from GitHub or a docs site,
 * even when the archive ships none — the report then hands the agent a
 * `release_notes_url` built from registry metadata, and the agent decides
 * whether to fetch it. Every request goes through `UpgradeNet`, which records
 * the host so `_meta.sources` is a fact, not a claim, and which tests replace
 * with a fake fetch.
 *
 * Request budget per call: index (1) + archive (1) + OSV for from/to (2) = 4,
 * each cached (index 6 h, parsed changelog 7 d, OSV 1 h). The npm packument
 * is large (vite's is ~39 MB, measured 2026-10-02), so only a reduced index
 * is cached — never the raw document.
 */

import { compareVersionPrecedence, parseSemverPrecedence } from "./semver-precedence.js";
import { ArchiveError, defaultFetch, fetchArchiveRootFiles, type FetchFn } from "./package-archive.js";
import { findChangelogFile, isChangelogName, parseChangelog, type ChangelogSection } from "./changelog.js";

export type UpgradeEcosystem = "npm" | "crates.io";

/** crates.io refuses API requests without a descriptive User-Agent. */
export const CRATES_USER_AGENT = "4da-mcp-server (https://4da.ai; upgrade_impact)";

const INDEX_TTL = 6 * 3600;
const CHANGELOG_TTL = 7 * 86_400;
const OSV_TTL = 3600;
const CACHE_SOURCE = "upgrade-impact";
const INDEX_TIMEOUT_MS = 30_000;
const OSV_TIMEOUT_MS = 10_000;

export interface PublishedVersion {
  version: string;
  published: string | null;
  deprecated?: string;
  yanked?: boolean;
  /** Archive URL (npm `dist.tarball`); crates archives are derived from name + version. */
  tarball?: string;
}

export interface RegistryIndex {
  /** The registry's canonical spelling (crates.io folds `-`/`_`). */
  name: string;
  ecosystem: UpgradeEcosystem;
  versions: PublishedVersion[];
  repository: string | null;
}

export interface ChangelogResult {
  found: boolean;
  file?: string;
  sections?: ChangelogSection[];
  reason?: string;
}

/** The cache surface used here; `LiveCache` satisfies it. */
export interface UpgradeCache {
  get<T>(key: string): T | null;
  set(key: string, data: unknown, source: string, ttlSeconds: number): void;
}

/** Network + cache seam. `contacted` collects every host actually requested. */
export interface UpgradeNet {
  fetch: FetchFn;
  cache: UpgradeCache | null;
  contacted: Set<string>;
}

export function createUpgradeNet(cache: UpgradeCache | null, fetchFn: FetchFn = defaultFetch): UpgradeNet {
  const contacted = new Set<string>();
  return {
    contacted,
    cache,
    fetch: (url, init, timeoutMs) => {
      contacted.add(new URL(url).host);
      return fetchFn(url, init, timeoutMs);
    },
  };
}

const NPM_NAME = /^(?:@[a-z0-9-~][a-z0-9-._~]*\/)?[a-z0-9-~][a-z0-9-._~]*$/i;
const CRATE_NAME = /^[A-Za-z][A-Za-z0-9_-]{0,63}$/;

export function isValidPackageName(name: string, ecosystem: UpgradeEcosystem): boolean {
  return ecosystem === "npm" ? NPM_NAME.test(name) && name.length <= 214 : CRATE_NAME.test(name);
}

async function cached<T>(net: UpgradeNet, key: string, ttl: number, load: () => Promise<T>): Promise<T> {
  const hit = net.cache?.get<T>(key);
  if (hit !== null && hit !== undefined) return hit;
  const value = await load();
  net.cache?.set(key, value, CACHE_SOURCE, ttl);
  return value;
}

interface NpmPackument {
  name?: string;
  versions?: Record<string, { deprecated?: unknown; dist?: { tarball?: string } }>;
  time?: Record<string, string>;
  repository?: unknown;
}

interface CratesCrateResponse {
  crate?: { name?: string; repository?: string | null };
  versions?: Array<{ num?: string; created_at?: string; yanked?: boolean }>;
}

function repositoryUrl(repo: unknown): string | null {
  if (typeof repo === "string") return repo;
  if (repo && typeof repo === "object" && typeof (repo as { url?: unknown }).url === "string") {
    return (repo as { url: string }).url;
  }
  return null;
}

async function loadNpmIndex(net: UpgradeNet, name: string): Promise<RegistryIndex | null> {
  const url = `https://registry.npmjs.org/${name.replaceAll("/", "%2F")}`;
  const res = await net.fetch(url, { headers: { Accept: "application/json" } }, INDEX_TIMEOUT_MS);
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`npm registry HTTP ${res.status} for ${name}`);
  const doc = (await res.json()) as NpmPackument;
  const versions = Object.entries(doc.versions ?? {}).map(([version, meta]) => {
    const entry: PublishedVersion = { version, published: doc.time?.[version] ?? null };
    if (typeof meta.deprecated === "string" && meta.deprecated) entry.deprecated = meta.deprecated;
    if (meta.dist?.tarball) entry.tarball = meta.dist.tarball;
    return entry;
  });
  return { name: doc.name ?? name, ecosystem: "npm", versions, repository: repositoryUrl(doc.repository) };
}

async function loadCratesIndex(net: UpgradeNet, name: string): Promise<RegistryIndex | null> {
  const url = `https://crates.io/api/v1/crates/${encodeURIComponent(name)}`;
  const headers = { Accept: "application/json", "User-Agent": CRATES_USER_AGENT };
  const res = await net.fetch(url, { headers }, INDEX_TIMEOUT_MS);
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`crates.io HTTP ${res.status} for ${name}`);
  const doc = (await res.json()) as CratesCrateResponse;
  const versions = (doc.versions ?? [])
    .filter((v): v is { num: string; created_at?: string; yanked?: boolean } => typeof v.num === "string")
    .map((v) => {
      const entry: PublishedVersion = { version: v.num, published: v.created_at ?? null };
      if (v.yanked) entry.yanked = true;
      return entry;
    });
  return { name: doc.crate?.name ?? name, ecosystem: "crates.io", versions, repository: doc.crate?.repository ?? null };
}

/** Registry index with versions in ascending precedence order (unreadable versions dropped). Null = unknown package. */
export async function getRegistryIndex(
  net: UpgradeNet,
  ecosystem: UpgradeEcosystem,
  name: string,
): Promise<RegistryIndex | null> {
  const key = `upgrade-impact:index:${ecosystem}:${ecosystem === "crates.io" ? name.replace(/-/g, "_") : name}`;
  const index = await cached(net, key, INDEX_TTL, () =>
    ecosystem === "npm" ? loadNpmIndex(net, name) : loadCratesIndex(net, name),
  );
  if (!index) return null;
  const versions = index.versions
    .filter((v) => parseSemverPrecedence(v.version) !== null)
    .sort((a, b) => compareVersionPrecedence(a.version, b.version) ?? 0);
  return { ...index, versions };
}

const ARCHIVE_HOSTS = new Set(["registry.npmjs.org", "static.crates.io"]);

/** The registry archive URL for one version, or null when it is not on the package's own registry. */
export function archiveUrl(index: RegistryIndex, version: PublishedVersion): string | null {
  const url =
    index.ecosystem === "crates.io"
      ? `https://static.crates.io/crates/${index.name}/${index.name}-${version.version}.crate`
      : version.tarball;
  if (!url) return null;
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && ARCHIVE_HOSTS.has(parsed.host) ? url : null;
  } catch {
    return null;
  }
}

/** Parsed changelog shipped inside one version's archive (cached 7 days per version). */
export async function getChangelog(
  net: UpgradeNet,
  index: RegistryIndex,
  version: PublishedVersion,
): Promise<ChangelogResult> {
  const url = archiveUrl(index, version);
  if (!url) {
    return { found: false, reason: "the version's archive is not hosted on the package's own registry" };
  }
  const key = `upgrade-impact:changelog:${index.ecosystem}:${index.name}:${version.version}`;
  const hit = net.cache?.get<ChangelogResult>(key);
  if (hit) return hit;

  const headers: Record<string, string> = index.ecosystem === "crates.io" ? { "User-Agent": CRATES_USER_AGENT } : {};
  let files: Map<string, string>;
  try {
    files = await fetchArchiveRootFiles(url, isChangelogName, net.fetch, headers);
  } catch (err) {
    const result = { found: false, reason: `could not read the ${version.version} archive: ${(err as Error).message}` };
    // A published archive never changes, so a cap or format refusal is permanent;
    // a network failure is not, and must not be remembered for a week.
    if (err instanceof ArchiveError) net.cache?.set(key, result, CACHE_SOURCE, CHANGELOG_TTL);
    return result;
  }
  const file = findChangelogFile(files.keys());
  const sections = file ? parseChangelog(files.get(file) ?? "") : [];
  const result: ChangelogResult = !file
    ? { found: false, reason: `the ${version.version} archive ships no changelog file` }
    : sections.length === 0
      ? { found: false, file, reason: `${file} has no version headings this parser recognises` }
      : { found: true, file, sections };
  net.cache?.set(key, result, CACHE_SOURCE, CHANGELOG_TTL);
  return result;
}

/** OSV advisory ids affecting one version, or null when OSV could not be reached. */
export async function getOsvAdvisories(
  net: UpgradeNet,
  ecosystem: UpgradeEcosystem,
  name: string,
  version: string,
): Promise<string[] | null> {
  const key = `upgrade-impact:osv:${ecosystem}:${name}:${version}`;
  const hit = net.cache?.get<string[]>(key);
  if (hit) return hit;
  try {
    const res = await net.fetch(
      "https://api.osv.dev/v1/query",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ package: { name, ecosystem }, version }),
      },
      OSV_TIMEOUT_MS,
    );
    if (!res.ok) return null;
    const body = (await res.json()) as { vulns?: Array<{ id?: string }> };
    const ids = (body.vulns ?? []).map((v) => v.id).filter((id): id is string => typeof id === "string");
    net.cache?.set(key, ids, CACHE_SOURCE, OSV_TTL);
    return ids;
  } catch {
    return null;
  }
}

/**
 * Release-notes page derived from the registry's own `repository` field: a
 * GitHub repo becomes `https://github.com/<owner>/<repo>/releases`; any other
 * https repository URL is returned as-is; anything else is null.
 */
export function releaseNotesUrl(repository: string | null): string | null {
  if (!repository) return null;
  const raw = repository.trim();
  const shorthand = /^(?:github:)?([\w.-]+)\/([\w.-]+)$/.exec(raw);
  const gh = shorthand ?? /github\.com[/:]([\w.-]+)\/([\w.-]+?)(?:\.git)?(?:[/#?].*)?$/i.exec(raw);
  if (gh) return `https://github.com/${gh[1]}/${gh[2].replace(/\.git$/, "")}/releases`;
  const https = raw.replace(/^git\+/, "");
  return /^https:\/\//i.test(https) ? https.replace(/\.git$/, "") : null;
}
