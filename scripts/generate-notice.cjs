#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Generates NOTICE mechanically from the real dependency graphs so third-party
// attribution cannot drift from what actually ships.
//
// Why this exists: NOTICE was hand-maintained at ~92 entries against a graph of
// ~900 Rust crates and ~40 production npm packages. It drifted (ts-rs pinned at
// "10" while the lockfile resolved 12, scraper 0.23 vs 0.27, chacha20poly1305
// 0.10 vs 0.11), omitted `ammonia` entirely, and attributed none of the shipping
// MPL-2.0 crates even though MPL-2.0 s3.2 requires notice. The two OFL-1.1 fonts
// were likewise unattributed, and OFL-1.1 requires the licence text to travel
// with the font software.
//
// NOTICE is raw-imported by src/components/ThirdPartyLicensesModal.tsx, so this
// file is the single source of truth for both the repo and the in-app modal.
//
// Usage:
//   node scripts/generate-notice.cjs                     # write NOTICE
//   node scripts/generate-notice.cjs --check             # fail (exit 1) if stale
//   node scripts/generate-notice.cjs --check --require   # ...and fail if the
//                                                        # toolchain is missing
//
// Requirements: cargo (for `cargo metadata`) and an installed node_modules
// (for `pnpm licenses list`). In plain --check mode a missing toolchain is
// reported as a skip rather than a failure, so a partial checkout does not block
// a local commit. CI passes --require so the gate can never pass by skipping.

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = path.resolve(__dirname, '..');
const NOTICE_PATH = path.join(REPO_ROOT, 'NOTICE');
const CARGO_MANIFEST = path.join(REPO_ROOT, 'src-tauri', 'Cargo.toml');

const COPYRIGHT_YEARS = '2025-2026';

function run(cmd, args, opts = {}) {
  return execFileSync(cmd, args, {
    cwd: REPO_ROOT,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    ...opts,
  });
}

// ---------------------------------------------------------------------------
// Rust: resolve the graph reachable from the root crate over normal + build
// dependency edges. Dev-only crates are excluded because they never ship.
// ---------------------------------------------------------------------------

function collectRustCrates() {
  const raw = run('cargo', [
    'metadata',
    '--format-version',
    '1',
    '--manifest-path',
    CARGO_MANIFEST,
    '--locked',
  ]);
  const meta = JSON.parse(raw);

  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const rootId = meta.resolve.root;

  // BFS over non-dev edges. `dep_kinds[].kind` is null for normal deps,
  // "build" for build-dependencies, "dev" for dev-dependencies.
  const seen = new Set();
  const queue = [rootId];
  while (queue.length > 0) {
    const id = queue.shift();
    if (seen.has(id)) continue;
    seen.add(id);
    const node = nodes.get(id);
    if (!node) continue;
    for (const dep of node.deps) {
      const kinds = dep.dep_kinds ?? [];
      const ships = kinds.length === 0 || kinds.some((k) => k.kind === null || k.kind === 'build');
      if (ships && !seen.has(dep.pkg)) queue.push(dep.pkg);
    }
  }
  seen.delete(rootId);

  return [...seen]
    .map((id) => byId.get(id))
    .filter(Boolean)
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: normalizeLicense(p.license, p.license_file),
      url: p.repository || p.homepage || `https://crates.io/crates/${p.name}`,
    }))
    .sort(cmpEntry);
}

// ---------------------------------------------------------------------------
// npm: production dependency closure, via pnpm's own licence resolver.
// ---------------------------------------------------------------------------

function collectNpmPackages() {
  const raw = run('pnpm', ['licenses', 'list', '--json', '--prod'], { shell: process.platform === 'win32' });
  const grouped = JSON.parse(raw);
  const out = [];
  for (const [license, entries] of Object.entries(grouped)) {
    for (const e of entries) {
      for (const version of e.versions ?? []) {
        out.push({
          name: e.name,
          version,
          license: normalizeLicense(license),
          url: e.homepage || `https://www.npmjs.com/package/${e.name}`,
          paths: e.paths ?? [],
        });
      }
    }
  }
  return out.sort(cmpEntry);
}

function normalizeLicense(license, licenseFile) {
  if (!license || license === 'null') {
    return licenseFile ? 'See bundled licence file' : 'UNKNOWN';
  }
  return license;
}

function cmpEntry(a, b) {
  const n = a.name.localeCompare(b.name, 'en');
  return n !== 0 ? n : compareVersions(a.version, b.version);
}

function compareVersions(a, b) {
  const pa = String(a).split(/[.+-]/);
  const pb = String(b).split(/[.+-]/);
  for (let i = 0; i < Math.max(pa.length, pb.length); i += 1) {
    const na = Number(pa[i]);
    const nb = Number(pb[i]);
    if (Number.isNaN(na) || Number.isNaN(nb)) {
      const s = String(pa[i] ?? '').localeCompare(String(pb[i] ?? ''), 'en');
      if (s !== 0) return s;
    } else if (na !== nb) {
      return na - nb;
    }
  }
  return 0;
}

// ---------------------------------------------------------------------------
// Reciprocal / attribution-sensitive licences that need more than a one-liner.
// ---------------------------------------------------------------------------

const RECIPROCAL = /\b(MPL-2\.0|EPL-2\.0|LGPL|CDDL|CPL-1\.0)\b/;
const COPYLEFT_BLOCKERS = /\b(GPL-2\.0|GPL-3\.0|AGPL|SSPL)\b/;

function isReciprocal(license) {
  // "MIT OR MPL-2.0" style dual licences let us take the permissive half, so
  // only flag a licence that is reciprocal on every branch of the expression.
  if (!RECIPROCAL.test(license)) return false;
  return license
    .split(/\s+OR\s+/i)
    .every((branch) => RECIPROCAL.test(branch));
}

function findCopyleftBlockers(entries) {
  return entries.filter((e) => {
    if (!COPYLEFT_BLOCKERS.test(e.license)) return false;
    // LGPL is matched separately; a "GPL-3.0 WITH exception" or a dual licence
    // with a permissive branch is not a blocker.
    if (/\bWITH\b/i.test(e.license)) return false;
    return e.license.split(/\s+OR\s+/i).every((b) => COPYLEFT_BLOCKERS.test(b));
  });
}

function readOflTexts(npmEntries) {
  const blocks = [];
  const oflPkgs = npmEntries.filter((e) => /OFL-1\.1/i.test(e.license));
  const seenNames = new Set();
  for (const pkg of oflPkgs) {
    if (seenNames.has(pkg.name)) continue;
    seenNames.add(pkg.name);
    for (const p of pkg.paths) {
      const candidate = path.join(p, 'LICENSE');
      if (fs.existsSync(candidate)) {
        blocks.push({
          name: pkg.name,
          version: pkg.version,
          text: fs.readFileSync(candidate, 'utf8').trim(),
        });
        break;
      }
    }
  }
  return blocks;
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function section(title, entries) {
  const lines = [title, '-'.repeat(title.length), ''];
  for (const e of entries) {
    lines.push(`${e.name} ${e.version} - ${e.license} - ${e.url}`);
  }
  lines.push('');
  return lines.join('\n');
}

function render({ rustCrates, npmPackages, oflTexts }) {
  const reciprocalRust = rustCrates.filter((e) => isReciprocal(e.license));
  const reciprocalNpm = npmPackages.filter((e) => isReciprocal(e.license));
  const reciprocal = [...reciprocalRust, ...reciprocalNpm].sort(cmpEntry);

  const parts = [];

  parts.push(
    [
      'NOTICE - Third-Party Licenses',
      '==============================',
      '',
      `Copyright ${COPYRIGHT_YEARS} 4DA Systems Pty Ltd (ACN 696 078 841)`,
      '',
      'This product includes software developed by third parties.',
      '4DA itself is licensed under FSL-1.1-Apache-2.0.',
      '',
      'THIS FILE IS GENERATED - DO NOT EDIT BY HAND.',
      'Regenerate with:  node scripts/generate-notice.cjs',
      'Verify with:      node scripts/generate-notice.cjs --check   (runs in CI)',
      '',
      'Rust crates are the non-dev dependency closure of src-tauri (normal +',
      'build edges, all target platforms). npm packages are the production',
      'dependency closure reported by `pnpm licenses list --prod`. Dev-only',
      'dependencies are omitted because they are not distributed.',
      '',
      `Totals: ${rustCrates.length} Rust crates, ${npmPackages.length} npm packages.`,
      '',
    ].join('\n')
  );

  parts.push(section(`Rust Crates (${rustCrates.length})`, rustCrates));
  parts.push(section(`npm Packages (${npmPackages.length})`, npmPackages));

  if (reciprocal.length > 0) {
    const lines = [
      `Reciprocal-Licence Components (${reciprocal.length})`,
      '-'.repeat(`Reciprocal-Licence Components (${reciprocal.length})`.length),
      '',
      'The components below are licensed under weak-copyleft terms (MPL-2.0,',
      'EPL-2.0 and similar). They are used unmodified, as libraries. Their',
      'licences require that this notice identify them and that their source',
      'remain available: each is published at the URL shown, and the exact',
      'version used is recorded in the lockfiles in this repository',
      '(src-tauri/Cargo.lock, pnpm-lock.yaml). Obtaining a copy of the source',
      'for any of them requires no request to 4DA Systems.',
      '',
    ];
    for (const e of reciprocal) {
      lines.push(`${e.name} ${e.version} - ${e.license} - ${e.url}`);
    }
    lines.push('');
    parts.push(lines.join('\n'));
  }

  for (const block of oflTexts) {
    const title = `Full Licence Text - ${block.name} ${block.version} (SIL Open Font License 1.1)`;
    parts.push(
      [title, '-'.repeat(title.length), '', block.text, ''].join('\n')
    );
  }

  return `${parts.join('\n').replace(/\s+$/, '')}\n`;
}

// ---------------------------------------------------------------------------

function main() {
  const check = process.argv.includes('--check');
  const require_ = process.argv.includes('--require');

  let rustCrates;
  let npmPackages;
  try {
    rustCrates = collectRustCrates();
    npmPackages = collectNpmPackages();
  } catch (err) {
    const msg = err && err.message ? err.message : String(err);
    if (check && !require_) {
      console.warn(`[notice] SKIP - could not build dependency graph: ${msg.split('\n')[0]}`);
      console.warn('[notice] Needs `cargo` and an installed node_modules. Not treated as drift.');
      process.exit(0);
    }
    console.error(`[notice] FAILED to build dependency graph: ${msg}`);
    console.error('[notice] Needs `cargo` on PATH and `pnpm install` already run.');
    process.exit(2);
  }

  const blockers = findCopyleftBlockers([...rustCrates, ...npmPackages]);
  if (blockers.length > 0) {
    console.error('[notice] BLOCKING: strong-copyleft dependency detected, incompatible with FSL-1.1-Apache-2.0 distribution:');
    for (const b of blockers) console.error(`  - ${b.name} ${b.version} (${b.license})`);
    process.exit(3);
  }

  const oflTexts = readOflTexts(npmPackages);
  const expectedOfl = new Set(
    npmPackages.filter((e) => /OFL-1\.1/i.test(e.license)).map((e) => e.name)
  );
  if (oflTexts.length < expectedOfl.size) {
    const got = new Set(oflTexts.map((b) => b.name));
    const missing = [...expectedOfl].filter((n) => !got.has(n));
    console.error(`[notice] BLOCKING: OFL-1.1 package(s) without a bundled licence text: ${missing.join(', ')}`);
    console.error('[notice] OFL-1.1 requires the licence to be distributed with the font software.');
    process.exit(4);
  }

  const rendered = render({ rustCrates, npmPackages, oflTexts });
  const current = fs.existsSync(NOTICE_PATH) ? fs.readFileSync(NOTICE_PATH, 'utf8') : '';

  if (check) {
    if (normalizeEol(current) === normalizeEol(rendered)) {
      console.log(`[notice] OK - ${rustCrates.length} Rust crates, ${npmPackages.length} npm packages, ${oflTexts.length} full licence text(s).`);
      process.exit(0);
    }
    console.error('[notice] STALE - NOTICE does not match the resolved dependency graph.');
    console.error('[notice] Regenerate and commit:  node scripts/generate-notice.cjs');
    console.error(summarizeDriftLines(current, rendered));
    process.exit(1);
  }

  fs.writeFileSync(NOTICE_PATH, rendered, 'utf8');
  console.log(`[notice] Wrote NOTICE - ${rustCrates.length} Rust crates, ${npmPackages.length} npm packages, ${oflTexts.length} full licence text(s).`);
}

function normalizeEol(s) {
  return s.replace(/\r\n/g, '\n');
}

function summarizeDriftLines(current, rendered) {
  const a = new Set(normalizeEol(current).split('\n'));
  const b = new Set(normalizeEol(rendered).split('\n'));
  const added = [...b].filter((l) => l.trim() && !a.has(l)).slice(0, 12);
  const removed = [...a].filter((l) => l.trim() && !b.has(l)).slice(0, 12);
  const out = [];
  if (added.length) out.push('  would add:', ...added.map((l) => `    + ${l}`));
  if (removed.length) out.push('  would remove:', ...removed.map((l) => `    - ${l}`));
  return out.join('\n');
}

if (require.main === module) {
  main();
}

module.exports = {
  NOTICE_PATH,
  cmpEntry,
  compareVersions,
  findCopyleftBlockers,
  isReciprocal,
  normalizeEol,
  normalizeLicense,
  render,
  summarizeDriftLines,
};
