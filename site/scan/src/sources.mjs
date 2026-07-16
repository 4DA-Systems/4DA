// Data layer — everything runs in the visitor's browser. The only thing that leaves the
// page is public package names + versions sent to public registries and OSV.dev (the same
// data anyone can read out of the manifest). No server of ours is involved, ever.
//
// Each resolver returns per-dep facts: latest version, releases inside the window,
// deprecation, yanked status. Failures degrade per-package (skipped count), never page-wide.

const WINDOW_DAYS = 30;
const DEP_CAP = 150; // direct deps beyond this are skipped with a notice
const VULN_DETAIL_CAP = 80;

const now = () => Date.now();
const daysAgo = (iso) => (now() - new Date(iso).getTime()) / 86400000;

// crates.io requires a User-Agent (403 without one). Browsers always send their own and
// silently drop this forbidden header; Node (undici) sends none unless told — this keeps
// the same code working in both runtimes.
const UA = { "user-agent": "4da-stackscan (https://4da.ai)" };

// Prerelease versions (1.2.3-beta.1, 0.0.0-insiders.*, 1.0a1) are workflow noise for this
// report: a "what shipped around your stack" list flooded with canary builds buries the
// actual releases. Stable-only is the premium default.
const isPrerelease = (v) => /-|(?:^|\.)(?:a|b|rc|alpha|beta|dev|insiders|next|canary|nightly)\d*(?:\.|$)/i.test(String(v));

/** Tiny concurrency pool — registries are polite-rate, crates.io especially. */
async function pool(items, limit, fn, onProgress) {
  const results = new Array(items.length);
  let i = 0;
  async function worker() {
    while (i < items.length) {
      const idx = i++;
      try {
        results[idx] = await fn(items[idx], idx);
      } catch (e) {
        results[idx] = { error: e.message };
      }
      onProgress?.(items[idx], results[idx]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

function majorOf(v) {
  const m = String(v || "").match(/^(\d+)/);
  return m ? Number(m[1]) : null;
}

function cmpSemver(a, b) {
  const pa = String(a).split(/[.+-]/).map((x) => (isNaN(x) ? x : Number(x)));
  const pb = String(b).split(/[.+-]/).map((x) => (isNaN(x) ? x : Number(x)));
  for (let i = 0; i < 3; i++) {
    const x = pa[i] ?? 0, y = pb[i] ?? 0;
    if (x !== y) return x > y ? 1 : -1;
  }
  return 0;
}

// ---------------------------------------------------------------------------
// Per-ecosystem release/metadata resolvers
// ---------------------------------------------------------------------------

async function resolveNpm(dep) {
  const res = await fetch(`https://registry.npmjs.org/${encodeURIComponent(dep.name)}`, { headers: UA });
  if (!res.ok) return { missing: true };
  const doc = await res.json();
  const latest = doc["dist-tags"]?.latest ?? null;
  const time = doc.time || {};
  const releases = Object.entries(time)
    .filter(([v, t]) => v !== "created" && v !== "modified" && !isPrerelease(v) && daysAgo(t) <= WINDOW_DAYS)
    .map(([v, t]) => ({ version: v, at: t }))
    .sort((a, b) => new Date(b.at) - new Date(a.at));
  const latestMeta = latest ? doc.versions?.[latest] : null;
  return {
    latest,
    latestAt: latest ? time[latest] ?? null : null,
    releases,
    deprecated: typeof latestMeta?.deprecated === "string" ? latestMeta.deprecated : null,
    url: `https://www.npmjs.com/package/${dep.name}`,
  };
}

async function resolveCrate(dep) {
  const res = await fetch(`https://crates.io/api/v1/crates/${encodeURIComponent(dep.name)}`, { headers: UA });
  if (!res.ok) return { missing: true };
  const doc = await res.json();
  const latest = doc.crate?.max_stable_version || doc.crate?.max_version || null;
  const releases = (doc.versions || [])
    .filter((v) => !v.yanked && daysAgo(v.created_at) <= WINDOW_DAYS)
    .map((v) => ({ version: v.num, at: v.created_at }));
  const latestRow = (doc.versions || []).find((v) => v.num === latest);
  return {
    latest,
    latestAt: latestRow?.created_at ?? null,
    releases,
    deprecated: null,
    url: `https://crates.io/crates/${dep.name}`,
  };
}

async function resolvePypi(dep) {
  const res = await fetch(`https://pypi.org/pypi/${encodeURIComponent(dep.name)}/json`, { headers: UA });
  if (!res.ok) return { missing: true };
  const doc = await res.json();
  const latest = doc.info?.version ?? null;
  const releases = [];
  for (const [v, files] of Object.entries(doc.releases || {})) {
    const t = files?.[0]?.upload_time_iso_8601;
    if (t && !isPrerelease(v) && daysAgo(t) <= WINDOW_DAYS) releases.push({ version: v, at: t });
  }
  releases.sort((a, b) => new Date(b.at) - new Date(a.at));
  const latestAt = doc.releases?.[latest]?.[0]?.upload_time_iso_8601 ?? null;
  return {
    latest,
    latestAt,
    releases,
    deprecated: doc.info?.yanked ? doc.info?.yanked_reason || "yanked" : null,
    url: `https://pypi.org/project/${dep.name}/`,
  };
}

// Go module paths are case-encoded for the proxy: uppercase -> !lowercase.
const goEscape = (p) => p.replace(/[A-Z]/g, (c) => "!" + c.toLowerCase());

async function resolveGo(dep) {
  const res = await fetch(`https://proxy.golang.org/${goEscape(dep.name)}/@latest`, { headers: UA });
  if (!res.ok) return { missing: true };
  const doc = await res.json();
  const latest = (doc.Version || "").replace(/^v/, "");
  const latestAt = doc.Time ?? null;
  // The proxy's version list carries no dates; we report the latest release when it falls
  // inside the window rather than fanning out one .info call per historical version.
  const releases =
    latestAt && daysAgo(latestAt) <= WINDOW_DAYS ? [{ version: latest, at: latestAt }] : [];
  return {
    latest,
    latestAt,
    releases,
    deprecated: null,
    url: `https://pkg.go.dev/${dep.name}`,
  };
}

const RESOLVERS = { npm: resolveNpm, "crates.io": resolveCrate, PyPI: resolvePypi, Go: resolveGo };

// ---------------------------------------------------------------------------
// OSV — batch match, then detail fetch
// ---------------------------------------------------------------------------

async function osvBatch(ecosystem, deps) {
  const queries = deps.map((d) =>
    d.version
      ? { package: { name: d.name, ecosystem }, version: d.version }
      : { package: { name: d.name, ecosystem } },
  );
  const res = await fetch("https://api.osv.dev/v1/querybatch", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ queries }),
  });
  if (!res.ok) throw new Error(`OSV querybatch ${res.status}`);
  const { results } = await res.json();
  return results.map((r) => (r.vulns || []).map((v) => v.id));
}

function cvssScoreToSeverity(score) {
  if (score >= 9) return "critical";
  if (score >= 7) return "high";
  if (score >= 4) return "medium";
  return "low";
}

function extractSeverity(vuln) {
  // Prefer database_specific.severity (GHSA), fall back to CVSS score parsing.
  const ds = vuln.database_specific?.severity;
  if (typeof ds === "string") return ds.toLowerCase();
  for (const s of vuln.severity || []) {
    const m = String(s.score).match(/(\d+(\.\d+)?)$/); // numeric score
    if (s.type?.startsWith("CVSS") && m) return cvssScoreToSeverity(Number(m[1]));
  }
  for (const a of vuln.affected || []) {
    const v = a.database_specific?.severity;
    if (typeof v === "string") return v.toLowerCase();
  }
  return "unknown";
}

function extractFixedVersion(vuln, pkgName, ecosystem) {
  for (const a of vuln.affected || []) {
    if (a.package?.name !== pkgName || a.package?.ecosystem !== ecosystem) continue;
    for (const r of a.ranges || []) {
      for (const e of r.events || []) {
        if (e.fixed) return e.fixed;
      }
    }
  }
  return null;
}

async function osvDetail(id) {
  const res = await fetch(`https://api.osv.dev/v1/vulns/${encodeURIComponent(id)}`);
  if (!res.ok) throw new Error(`OSV detail ${res.status}`);
  return res.json();
}

// ---------------------------------------------------------------------------
// The scan
// ---------------------------------------------------------------------------

/**
 * Run the full scan for one parsed manifest.
 * progress(line) receives terminal-feed strings as work happens.
 * Returns { stats, advisories, releases, behind, deprecated, notes }.
 */
export async function scan(manifest, progress = () => {}) {
  const notes = [];
  let deps = manifest.deps;
  if (deps.length > DEP_CAP) {
    notes.push(`${deps.length - DEP_CAP} dependencies beyond the first ${DEP_CAP} were skipped.`);
    deps = deps.slice(0, DEP_CAP);
  }
  if (deps.length === 0) throw new Error("No dependencies found in this manifest.");

  progress(`${manifest.ecosystem} · ${deps.length} direct dependencies parsed`);

  // 1. Registry facts (releases, latest, deprecation), throttled politely.
  const limit = manifest.ecosystem === "crates.io" ? 2 : 6;
  const facts = await pool(deps, limit, (d) => RESOLVERS[manifest.ecosystem](d), (dep, fact) => {
    if (fact?.missing) progress(`${dep.name} — not found in registry, skipped`);
    else if (fact?.error) progress(`${dep.name} — registry error, skipped`);
    else if (fact?.releases?.length)
      progress(`${dep.name} ${dep.version ?? ""} -> ${fact.latest} · ${fact.releases.length} release(s) in window`);
    else progress(`${dep.name} — quiet (no releases in ${WINDOW_DAYS}d)`);
  });

  // 2. OSV match.
  progress(`matching ${deps.length} packages against OSV.dev...`);
  let vulnIdsPerDep = [];
  try {
    vulnIdsPerDep = await osvBatch(manifest.ecosystem, deps);
  } catch (e) {
    notes.push(`Vulnerability matching unavailable (${e.message}).`);
    vulnIdsPerDep = deps.map(() => []);
  }
  const idToDeps = new Map();
  vulnIdsPerDep.forEach((ids, i) => {
    for (const id of ids) {
      if (!idToDeps.has(id)) idToDeps.set(id, []);
      idToDeps.get(id).push(deps[i]);
    }
  });
  let vulnIds = [...idToDeps.keys()];
  progress(`osv: ${vulnIds.length} distinct advisories matched`);
  if (vulnIds.length > VULN_DETAIL_CAP) {
    notes.push(`${vulnIds.length - VULN_DETAIL_CAP} advisories beyond the first ${VULN_DETAIL_CAP} were not detailed.`);
    vulnIds = vulnIds.slice(0, VULN_DETAIL_CAP);
  }

  // 3. OSV details.
  const details = await pool(vulnIds, 6, osvDetail, (id, d) => {
    if (d && !d.error) progress(`advisory ${id} — ${extractSeverity(d)}`);
  });

  const advisories = [];
  details.forEach((vuln, i) => {
    if (!vuln || vuln.error) return;
    const id = vulnIds[i];
    for (const dep of idToDeps.get(id)) {
      advisories.push({
        id,
        cve: (vuln.aliases || []).find((a) => a.startsWith("CVE-")) || null,
        package: dep.name,
        version: dep.version,
        dev: dep.dev,
        severity: extractSeverity(vuln),
        summary: vuln.summary || (vuln.details || "").slice(0, 160) || id,
        fixed: extractFixedVersion(vuln, dep.name, manifest.ecosystem),
        url: `https://osv.dev/vulnerability/${id}`,
        published: vuln.published || null,
      });
    }
  });
  const sevRank = { critical: 4, high: 3, medium: 2, moderate: 2, low: 1, unknown: 0 };
  advisories.sort((a, b) => (sevRank[b.severity] || 0) - (sevRank[a.severity] || 0));

  // 4. Releases / behind / deprecated from registry facts.
  const releases = [];
  const behind = [];
  const deprecated = [];
  let resolved = 0;
  deps.forEach((dep, i) => {
    const f = facts[i];
    if (!f || f.missing || f.error) return;
    resolved++;
    for (const r of f.releases || []) {
      releases.push({ package: dep.name, pinned: dep.version, version: r.version, at: r.at, dev: dep.dev, url: f.url });
    }
    const pinnedMajor = majorOf(dep.version);
    const latestMajor = majorOf(f.latest);
    if (pinnedMajor != null && latestMajor != null && latestMajor > pinnedMajor) {
      behind.push({ package: dep.name, pinned: dep.version, latest: f.latest, majors: latestMajor - pinnedMajor, dev: dep.dev, url: f.url });
    }
    if (f.deprecated) deprecated.push({ package: dep.name, message: String(f.deprecated).slice(0, 200), url: f.url });
  });
  releases.sort((a, b) => new Date(b.at) - new Date(a.at));
  behind.sort((a, b) => b.majors - a.majors || cmpSemver(b.latest, a.latest));

  const skipped = deps.length - resolved;
  if (skipped > 0) notes.push(`${skipped} package(s) could not be resolved in the registry.`);

  return {
    stats: {
      deps: deps.length,
      advisories: advisories.length,
      releases: releases.length,
      behind: behind.length,
      deprecated: deprecated.length,
    },
    windowDays: WINDOW_DAYS,
    advisories,
    releases,
    behind,
    deprecated,
    notes,
  };
}
