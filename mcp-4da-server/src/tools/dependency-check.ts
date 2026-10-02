// SPDX-License-Identifier: Apache-2.0
/**
 * dependency_check tool
 *
 * A pre-flight verdict for ONE proposed change per item: adding a dependency
 * at a version, or bumping an installed one. An agent calls this before it
 * edits a manifest and gets `proceed | wait | review | avoid | unknown`, a
 * one-line reason, and the signals behind it, each with evidence.
 *
 * Signals (see dependency-check-signals.ts): known advisories on the target
 * (OSV), release age (hold releases < 3 days unless they fix an advisory on
 * the installed version), publish-trust drop, newly added install scripts
 * (npm), newly introduced runtime dependencies that are themselves < 30 days
 * old, yanked/deprecated/unpublished targets, and the upgrade type.
 *
 * Honesty contract: a registry or OSV that cannot be reached makes the item
 * `unknown`, never `proceed`. A package missing from the public registry is
 * `unknown` with that said (private package, or a misspelled/hallucinated
 * name), never treated as clean.
 *
 * Privacy: registry requests carry the package name only — the installed and
 * target versions are picked out of the full release list locally. Only OSV
 * receives name + version, exactly as vulnerability_scan already does.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import { isMaintenanceNotice } from "../live/maintenance.js";
import { parseSemverPrecedence } from "../live/semver-precedence.js";
import type { PackageReleaseIndex, RegistryLookup, ReleaseMetadataSource } from "../live/release-metadata.js";
import type { ResolvedDependency, VulnerabilityEntry } from "../live/types.js";
import {
  advisoriesSignal,
  decide,
  installScriptSignal,
  introducedDependencies,
  newDependenciesSignal,
  publishTrustSignal,
  releaseAgeSignal,
  selectBaseline,
  upgradeTypeSignal,
  withdrawnSignal,
  type AdvisoryFacts,
  type AdvisoryRef,
  type CheckItem,
  type CreatedAtFact,
  type Signal,
  type Verdict,
} from "./dependency-check-signals.js";

export interface DependencyCheckParams {
  items?: unknown;
}

const MAX_ITEMS = 25;
/** First-publish lookups for newly introduced dependencies, per call (each is a registry read). */
const MAX_NEW_DEP_LOOKUPS = 40;
const NPM_CONCURRENCY = 4;

const PRIVACY_NOTE =
  "Registry requests carried package names only (npm full packument, crates.io versions API and sparse index); " +
  "installed and target versions were never sent to a registry. OSV.dev received package names and versions, as vulnerability_scan does.";

const VERDICT_RULE =
  "avoid > review > unknown > wait > proceed. unknown = a registry or OSV.dev lookup failed (never read as proceed); " +
  "a concrete avoid/review finding still wins over a missing source.";

export const dependencyCheckTool = {
  name: "dependency_check",
  description: [
    "Pre-flight verdict for adding a dependency or changing one to a specific version. Call BEFORE you add a package to a manifest or apply a version bump (including bumps proposed by upgrade_planner or a bot), and act on the verdict.",
    "",
    "Per item it returns `verdict` (proceed | wait | review | avoid | unknown), a one-line `reason`, and `signals`, each with `id`, `value`, `evidence` and the verdict that signal implies alone (`effect`):",
    "- advisories: known OSV advisories affecting `to`, and those this change fixes on `from`. High/critical on `to` -> avoid; other -> review.",
    "- release_age: days since `to` was published. Under 3 days -> wait, unless `to` fixes an advisory affecting `from` (a security fix is not held).",
    "- publish_trust: trust level of `to` vs `from` (or, for a new dependency, the previous release on the same line). 2 = trusted publisher (npm `_npmUser.trustedPublisher`, crates.io `trustpub_data`), 1 = npm provenance attestation, 0 = neither. A drop -> review.",
    "- install_script_added (npm): `to` runs preinstall/install/postinstall and `from` did not -> review. For a new dependency, information only.",
    "- new_dependencies: runtime dependencies added since the baseline, each with its first-publish age. Any under 30 days old, or no longer on the registry -> review.",
    "- yanked_or_deprecated: `to` yanked, unpublished or deprecated -> avoid.",
    "- upgrade_type: patch | minor | major (a 0.x minor counts as major) | downgrade | new_dependency. Information only.",
    "",
    "Verdict precedence: avoid > review > unknown > wait > proceed. `unknown` means a registry or OSV.dev lookup failed, or the package is not on the public registry (private package, or a misspelled/hallucinated name); it is never a silent proceed. Apply only `proceed` items; report the others with their evidence. For WHAT the bump changes in your code, call upgrade_impact.",
    "",
    "Privacy: registry requests carry the package NAME only (npm full packument, crates.io versions API and sparse index); the installed and target versions are never sent to a registry. OSV.dev receives package names and versions, as vulnerability_scan does. Set FOURDA_OFFLINE=true to disable all lookups (every item then returns unknown).",
  ].join("\n"),
  inputSchema: {
    type: "object" as const,
    properties: {
      items: {
        type: "array",
        minItems: 1,
        maxItems: 25,
        description: "The proposed changes to check, 1 to 25.",
        items: {
          type: "object",
          properties: {
            ecosystem: { type: "string", enum: ["npm", "crates.io"], description: "Package registry." },
            package: {
              type: "string",
              description: 'Package name exactly as published (e.g. "axios", "@scope/pkg", "serde_json").',
            },
            to: { type: "string", description: 'Exact version you intend to install (e.g. "1.14.1"), not a range.' },
            from: { type: "string", description: "Exact version currently installed. Omit when adding a new dependency." },
          },
          required: ["ecosystem", "package", "to"],
        },
      },
    },
    required: ["items"],
  },
};

interface ItemResult {
  ecosystem: string;
  package: string;
  from: string | null;
  to: string;
  verdict: Verdict;
  reason: string;
  signals: Signal[];
}

export async function executeDependencyCheck(
  _db: FourDADatabase,
  params: DependencyCheckParams,
  liveIntel: LiveIntelligence | null,
  opts: { now?: Date } = {},
) {
  const items = validateItems(params.items);
  const now = opts.now ?? new Date();
  const checkedAt = now.toISOString();

  if (!liveIntel || !liveIntel.isEnabled()) {
    const reason = "Live lookups are disabled (FOURDA_OFFLINE=true); nothing about these versions could be checked.";
    return finish(
      checkedAt,
      items.map((item) => ({ ...head(item), verdict: "unknown" as const, reason, signals: [] })),
      { offline: true, osvCached: false },
    );
  }

  const sources = liveIntel.getReleaseSources();
  const indexes = await fetchIndexes(items, sources);

  const osvDeps: ResolvedDependency[] = items.flatMap((item) =>
    [item.to, item.from].filter((v): v is string => Boolean(v)).map((version) => osvDep(item, version)),
  );
  const scan = await liveIntel.queryAdvisories(osvDeps);
  const byInstance = new Map<string, VulnerabilityEntry[]>();
  for (const v of scan.vulnerabilities) {
    const key = instanceKey(v.ecosystem, v.package, v.currentVersion);
    byInstance.set(key, [...(byInstance.get(key) ?? []), v]);
  }
  const advisoriesFor = (item: CheckItem): AdvisoryFacts =>
    scan.offline
      ? { available: false, detail: "lookup failed or was rate limited" }
      : {
          available: true,
          to: refs(byInstance.get(instanceKey(item.ecosystem, item.package, item.to))),
          from: item.from ? refs(byInstance.get(instanceKey(item.ecosystem, item.package, item.from))) : [],
        };

  // Phase 2: first-publish dates of newly introduced dependencies.
  const plans = items.map((item) => planItem(item, indexes.get(lookupKey(item))!));
  const wantedCreated = new Map<string, { ecosystem: CheckItem["ecosystem"]; name: string }>();
  for (const p of plans) {
    for (const name of p.introduced ?? []) {
      if (wantedCreated.size >= MAX_NEW_DEP_LOOKUPS) break;
      wantedCreated.set(`${p.item.ecosystem}\0${name}`, { ecosystem: p.item.ecosystem, name });
    }
  }
  const created = new Map<string, CreatedAtFact>();
  await mapLimit([...wantedCreated.entries()], NPM_CONCURRENCY, async ([key, { ecosystem, name }]) => {
    const lookup = await sources[ecosystem].getCreatedAt(name);
    created.set(key, lookup.status === "ok" ? { status: "ok", createdAt: lookup.data } : lookup);
  });

  const results: ItemResult[] = plans.map((p) => {
    if (p.blocked) {
      // No release metadata: only the advisory lookup can still speak, and
      // only a negative finding is worth more than "unknown".
      const advisories = advisoriesSignal(p.item, advisoriesFor(p.item));
      const negative = advisories.effect === "avoid" || advisories.effect === "review";
      return {
        ...head(p.item),
        verdict: negative ? (advisories.effect as Verdict) : "unknown",
        reason: negative ? `advisories: ${advisories.evidence} | ${p.blocked}` : p.blocked,
        signals: [advisories],
      };
    }
    const { item, index } = p;
    const to = index!.releases[item.to];
    const base = selectBaseline(index!, item);
    const advisories = advisoriesSignal(item, advisoriesFor(item));
    const createdForItem = new Map<string, CreatedAtFact>();
    for (const name of p.introduced ?? []) {
      const fact = created.get(`${item.ecosystem}\0${name}`);
      if (fact) createdForItem.set(name, fact);
    }
    const signals: Signal[] = [
      advisories,
      releaseAgeSignal(item, to, advisories, now),
      publishTrustSignal(item, to, base),
      installScriptSignal(item, to, item.from ? index!.releases[item.from] ?? null : null),
      newDependenciesSignal(to, base, p.introduced, createdForItem),
      withdrawnSignal(item, to),
      upgradeTypeSignal(item),
    ];
    return { ...head(item), ...decide(signals), signals };
  });

  return finish(checkedAt, results, { offline: scan.offline, osvCached: scan.cached });
}

function planItem(item: CheckItem, lookup: RegistryLookup<PackageReleaseIndex>) {
  if (lookup.status === "unreachable") {
    return { item, blocked: `Registry could not be reached (${lookup.detail}); nothing about ${item.to} was verified.`, index: null, introduced: null };
  }
  if (lookup.status === "not_found") {
    return {
      item,
      blocked: `${lookup.detail}. If it is a private package this tool cannot judge it; otherwise check the name for a typo or a hallucinated package before adding it.`,
      index: null,
      introduced: null,
    };
  }
  const index = lookup.data;
  const to = index.releases[item.to];
  if (!to) {
    return { item, blocked: `${item.package}@${item.to} is not a published version on ${item.ecosystem}.`, index, introduced: null };
  }
  return { item, blocked: null, index, introduced: introducedDependencies(to, selectBaseline(index, item).record) };
}

async function fetchIndexes(
  items: CheckItem[],
  sources: Record<CheckItem["ecosystem"], ReleaseMetadataSource>,
): Promise<Map<string, RegistryLookup<PackageReleaseIndex>>> {
  const wanted = new Map<string, { item: CheckItem; versions: Set<string> }>();
  for (const item of items) {
    const key = lookupKey(item);
    const entry = wanted.get(key) ?? { item, versions: new Set<string>() };
    entry.versions.add(item.to);
    if (item.from) entry.versions.add(item.from);
    wanted.set(key, entry);
  }
  const out = new Map<string, RegistryLookup<PackageReleaseIndex>>();
  await mapLimit([...wanted.entries()], NPM_CONCURRENCY, async ([key, { item, versions }]) => {
    out.set(key, await sources[item.ecosystem].getReleases(item.package, [...versions]));
  });
  return out;
}

function validateItems(raw: unknown): CheckItem[] {
  if (!Array.isArray(raw) || raw.length < 1 || raw.length > MAX_ITEMS) {
    throw new Error(`items must be an array of 1 to ${MAX_ITEMS} { ecosystem, package, to, from? } objects`);
  }
  return raw.map((r, i) => {
    const o = (r ?? {}) as Record<string, unknown>;
    const where = `items[${i}]`;
    if (o.ecosystem !== "npm" && o.ecosystem !== "crates.io") throw new Error(`${where}.ecosystem must be "npm" or "crates.io"`);
    if (typeof o.package !== "string" || !o.package.trim()) throw new Error(`${where}.package must be a non-empty string`);
    const to = exactVersion(o.to, `${where}.to`);
    const from = o.from === undefined || o.from === null || o.from === "" ? undefined : exactVersion(o.from, `${where}.from`);
    return { ecosystem: o.ecosystem, package: o.package.trim(), to, ...(from ? { from } : {}) };
  });
}

function exactVersion(v: unknown, where: string): string {
  const s = typeof v === "string" ? v.trim().replace(/^v(?=\d)/, "") : "";
  if (!parseSemverPrecedence(s)) throw new Error(`${where} must be an exact version like 1.2.3 (got ${JSON.stringify(v)}), not a range`);
  return s;
}

function osvDep(item: CheckItem, version: string): ResolvedDependency {
  return {
    name: item.package,
    version,
    ecosystem: item.ecosystem,
    isDev: false,
    isDirect: true,
    devScopeKnown: true,
    target: null,
    platformActive: true,
    sourceDirs: [],
  };
}

function refs(entries: VulnerabilityEntry[] | undefined): AdvisoryRef[] {
  const seen = new Set<string>();
  return (entries ?? []).flatMap((v) => {
    if (seen.has(v.vulnId)) return [];
    seen.add(v.vulnId);
    return [{ id: v.vulnId, severity: v.severity, summary: v.summary, maintenance: isMaintenanceNotice(v) }];
  });
}

const instanceKey = (eco: string, name: string, version: string) => `${eco}\0${name}\0${version}`;
const lookupKey = (item: CheckItem) => `${item.ecosystem}\0${item.package}`;
const head = (item: CheckItem) => ({ ecosystem: item.ecosystem, package: item.package, from: item.from ?? null, to: item.to });

function finish(checkedAt: string, results: ItemResult[], meta: { offline: boolean; osvCached: boolean }) {
  const counts = new Map<Verdict, number>();
  for (const r of results) counts.set(r.verdict, (counts.get(r.verdict) ?? 0) + 1);
  const order: Verdict[] = ["avoid", "review", "unknown", "wait", "proceed"];
  const parts = order.filter((v) => counts.has(v)).map((v) => `${counts.get(v)} ${v}`);
  return {
    checked_at: checkedAt,
    summary: `${results.length} checked: ${parts.join(", ")}. Apply only "proceed" items; report the rest with their evidence.`,
    results,
    _meta: {
      sources: ["registry.npmjs.org", "crates.io", "index.crates.io", "osv.dev"],
      verdict_rule: VERDICT_RULE,
      privacy: PRIVACY_NOTE,
      offline: meta.offline,
      osv_cached: meta.osvCached,
      provenance: {
        mode: "live_registry_check",
        note: "Point-in-time facts from the public registries and OSV.dev at checked_at. Says nothing about code behaviour; a proceed is the absence of known warning signs, not a review of the release.",
      },
    },
  };
}

async function mapLimit<T>(items: T[], limit: number, fn: (item: T) => Promise<void>): Promise<void> {
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) await fn(items[next++]);
  });
  await Promise.all(workers);
}
