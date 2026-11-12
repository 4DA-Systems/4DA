// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Pure helpers for the engine end-to-end gate (scripts/ci/engine-e2e.mjs):
// the frozen advisory archives, the conformance key normalisation, and the
// comparisons against the conformance truth and the goldens. No process, no
// network: everything here is unit-tested in engine-e2e-lib.test.mjs.

import { createHash } from 'node:crypto';
import { crc32, deflateRawSync } from 'node:zlib';

// --- frozen OSV archives --------------------------------------------------------

/** The ecosystems the engine can sync (osv/cache.rs KNOWN_ECOSYSTEMS). */
export const MIRROR_ECOSYSTEMS = ['crates.io', 'npm', 'PyPI', 'Go', 'Maven', 'NuGet', 'RubyGems', 'Packagist', 'Pub'];

/**
 * Group OSV records by the ecosystems they affect: {ecosystem: [{name, json}]}.
 * Every known ecosystem gets an entry (an empty archive is still an archive), so
 * the mirror never answers 404 for an ecosystem the engine asks about.
 */
export function groupByEcosystem(records) {
  const out = Object.fromEntries(MIRROR_ECOSYSTEMS.map((e) => [e, []]));
  for (const { name, json } of records) {
    const ecos = new Set((json.affected || []).map((a) => a?.package?.ecosystem).filter(Boolean));
    for (const e of ecos) if (out[e]) out[e].push({ name, json });
  }
  return out;
}

/** A ZIP archive (deflate) holding `entries` [{name, data: Buffer}], deterministic bytes. */
export function buildZip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  // 1980-01-01 00:00: fixed, so the archive bytes (and its ETag) depend only on content.
  const dosTime = 0;
  const dosDate = (0 << 9) | (1 << 5) | 1;
  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, 'utf8');
    const compressed = deflateRawSync(data);
    const crc = crc32(data) >>> 0;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6); // UTF-8 names
    local.writeUInt16LE(8, 8); // deflate
    local.writeUInt16LE(dosTime, 10);
    local.writeUInt16LE(dosDate, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(compressed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28);
    locals.push(local, nameBuf, compressed);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt16LE(dosTime, 12);
    central.writeUInt16LE(dosDate, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(compressed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBuf);
    offset += local.length + nameBuf.length + compressed.length;
  }
  const centralBuf = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralBuf.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, centralBuf, end]);
}

export const etagOf = (buf) => `"${createHash('sha256').update(buf).digest('hex').slice(0, 32)}"`;

// --- conformance keys (tests/conformance/README.md "Key normalisation") ---------

const OSV_ECOSYSTEM = {
  npm: 'npm',
  javascript: 'npm',
  typescript: 'npm',
  rust: 'crates.io',
  cargo: 'crates.io',
  'crates.io': 'crates.io',
  python: 'PyPI',
  pypi: 'PyPI',
  pip: 'PyPI',
  go: 'Go',
  golang: 'Go',
  ruby: 'RubyGems',
  rubygems: 'RubyGems',
  php: 'Packagist',
  packagist: 'Packagist',
};

export const osvEcosystem = (eco) => OSV_ECOSYSTEM[String(eco).toLowerCase()] || eco;

export function normName(eco, name) {
  if (eco === 'PyPI') return name.toLowerCase().replace(/[-_.]+/g, '-');
  if (eco === 'Go') return name;
  return name.toLowerCase();
}

export function normVersion(eco, version) {
  let v = String(version);
  if (eco === 'Go') return v.replace(/^v/, '');
  if (eco === 'PyPI') {
    v = v.toLowerCase().replace(/^v/, '');
    // Drop trailing `.0` release segments: 2.0.0 -> 2, 1.0rc1 -> 1rc1.
    const m = /^(\d+(?:\.\d+)*)(.*)$/.exec(v);
    if (m) {
      const release = m[1].split('.');
      while (release.length > 1 && release.at(-1) === '0') release.pop();
      v = release.join('.') + m[2];
    }
  }
  return v;
}

/** The case and the project directory (relative to the case root) a stored project path belongs to. */
export function locate(projectPath, ctxRoot) {
  const p = String(projectPath).replace(/\\/g, '/');
  const root = String(ctxRoot).replace(/\\/g, '/').replace(/\/$/, '');
  const cmp = (s) => (process.platform === 'win32' ? s.toLowerCase() : s);
  if (!cmp(p).startsWith(`${cmp(root)}/`)) return null;
  const [caseId, ...rest] = p.slice(root.length + 1).split('/');
  return { caseId, dir: rest.length ? rest.join('/') : '.' };
}

/**
 * dependency_instances rows -> {caseId: Set("eco|dir|name|version")}, with every
 * row outside the context root collected under `outside`.
 */
export function inventoryByCase(rows, ctxRoot, caseIdsByLower = null) {
  const out = {};
  const outside = [];
  for (const r of rows) {
    const at = locate(r.project_path, ctxRoot);
    if (!at) {
      outside.push(r.project_path);
      continue;
    }
    // Windows stores project paths lowercased; map back to the case id.
    const caseId = caseIdsByLower?.[at.caseId.toLowerCase()] || at.caseId;
    const eco = osvEcosystem(r.ecosystem);
    const key = `${eco}|${at.dir}|${normName(eco, r.package_name)}|${normVersion(eco, r.version)}`;
    (out[caseId] ||= new Set()).add(key);
  }
  return { byCase: out, outside };
}

/** Set difference both ways, sorted. */
export function diffSets(actual, expected) {
  const a = new Set(actual);
  const e = new Set(expected);
  return {
    missing: [...e].filter((k) => !a.has(k)).sort(),
    extra: [...a].filter((k) => !e.has(k)).sort(),
  };
}

/**
 * The inventory the engine is expected to store for a case: the truth, minus the
 * lockfiles the coverage record says were skipped (a skipped directory is named,
 * never silently read as empty) and the lockfiles the truth marks unsupported.
 * `skippedDirs` are case-relative directories ('.'-free, forward slashes).
 */
export function expectedInventory(expected, skippedDirs = []) {
  const skipped = skippedDirs.map((d) => d.replace(/\/$/, ''));
  const inSkipped = (dir) => skipped.some((s) => dir === s || dir.startsWith(`${s}/`));
  return expected.inventory.packages.filter((k) => !inSkipped(k.split('|')[1]));
}

/** Expected findings as "eco|dir|name|version" -> Set(ids) (one entry per alias group). */
export function expectedFindingKeys(expected, skippedDirs = []) {
  const keep = new Set(expectedInventory(expected, skippedDirs));
  const out = new Map();
  for (const f of expected.findings) {
    const eco = f.ecosystem;
    const key = `${eco}|${f.dir}|${normName(eco, f.package)}|${normVersion(eco, f.version)}`;
    if (!keep.has(key)) continue;
    if (!out.has(key)) out.set(key, []);
    out.get(key).push(new Set(f.ids));
  }
  return out;
}

/** Apply a lockfile mutation spec to an inventory key list. */
export function applyMutation(keys, mutation) {
  const removed = new Set(mutation.removed || []);
  return [...keys.filter((k) => !removed.has(k)), ...(mutation.added || [])].sort();
}

// --- goldens ----------------------------------------------------------------------

/**
 * What a plan snapshot tells the user, without what varies by run or machine:
 * per item its id, kind, urgency, title and the dependencies and projects it
 * names; per step the advisories it clears and, per installed version, the target,
 * the kind of upgrade, whether it clears every known advisory, the held-target
 * check status and each site (project relative to the context root, dev, direct).
 * Left out on purpose: timestamps, run ids, the inventory hash (absolute paths),
 * `freshness_days` (moves with the calendar) and prose that embeds absolute paths
 * cut at a fixed length (its cut point moves with the root's length).
 */
export function normalizePlan(plan, ctxRoot) {
  const root = String(ctxRoot).replace(/\\/g, '/').replace(/\/$/, '');
  const ci = process.platform === 'win32';
  const rel = (p) => {
    const s = String(p).replace(/\\/g, '/');
    return (ci ? s.toLowerCase() : s).startsWith(ci ? `${root.toLowerCase()}/` : `${root}/`) ? `<ctx>/${s.slice(root.length + 1)}` : s;
  };
  const sorted = (xs) => [...(xs || [])].sort();
  const items = (plan.items || [])
    .map((i) => ({
      id: i.id,
      kind: i.kind,
      urgency: i.urgency,
      title: i.title,
      affected_deps: sorted(i.affected_deps),
      affected_projects: sorted((i.affected_projects || []).map(rel)),
    }))
    .sort((a, b) => a.id.localeCompare(b.id));
  const steps = (plan.steps || [])
    .map((s) => ({
      item_id: s.item_id,
      package: s.package,
      ecosystem: s.ecosystem,
      mechanism: s.mechanism,
      advisory_ids: sorted(s.advisory_ids),
      lines: (s.lines || [])
        .map((l) => ({
          installed: l.installed,
          target: l.target ?? null,
          upgrade_type: l.upgrade_type ?? null,
          clears_all_known: l.clears_all_known ?? null,
          held: l.held?.status ?? null,
          sites: sorted((l.sites || []).map((x) => `${rel(x.project)}${x.dev ? ' dev' : ''}${x.direct ? ' direct' : ''}`)),
        }))
        .sort((a, b) => String(a.installed).localeCompare(String(b.installed))),
    }))
    .sort((a, b) => a.item_id.localeCompare(b.item_id) || String(a.package).localeCompare(String(b.package)));
  return {
    schema_version: plan.schema_version,
    item_count: plan.item_count,
    multi_version_coverage: plan.multi_version_coverage,
    validation_drop_count: plan.validation_drop_count,
    items,
    steps,
  };
}

/** First differing path between two JSON values, or null when equal. */
export function firstDifference(a, b, path = '$') {
  if (a === b) return null;
  if (typeof a !== typeof b || a === null || b === null || typeof a !== 'object') {
    return { path, actual: a, expected: b };
  }
  if (Array.isArray(a) !== Array.isArray(b)) return { path, actual: a, expected: b };
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const k of keys) {
    const d = firstDifference(a[k], b[k], `${path}${Array.isArray(a) ? `[${k}]` : `.${k}`}`);
    if (d) return d;
  }
  return null;
}
