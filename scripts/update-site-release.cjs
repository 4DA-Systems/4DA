// Update site/src/_data/release.js from the REAL published release.
//
// Usage: node scripts/update-site-release.cjs vX.Y.Z [--apply]
// Run from the repo root after publishing a release, then deploy the site
// (cd site && pnpm run cf:deploy). Promoted from .claude/tmp on 2026-08-31 —
// this is the canonical per-release procedure; do not hand-edit release.js.
//
// Asset filenames are DISCOVERED from the release, never templated: Tauri
// derives them from productName, which changed "4DA Home" -> "4DA" in 1.0.1
// and silently broke every hardcoded download URL. Match on shape instead.
const { execFileSync } = require('child_process');
const fs = require('fs');

const TAG = process.argv[2];
const APPLY = process.argv.includes('--apply');
if (!TAG || !/^v\d+\.\d+\.\d+(-\w+)?$/.test(TAG)) {
  console.error('usage: node update_site_release.cjs vX.Y.Z [--apply]');
  process.exit(1);
}
const VERSION = TAG.slice(1);
const FILE = 'site/src/_data/release.js';

let assets;
try {
  assets = JSON.parse(
    execFileSync('gh', ['release', 'view', TAG, '--repo', '4DA-Systems/4DA', '--json', 'assets'],
      { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
  ).assets;
} catch (e) {
  console.error('INCONCLUSIVE: could not read the release — ' + e.message.split('\n')[0]);
  process.exit(2);
}

// Ignore checksums, signatures, CLI binaries, updater bundles (.app.tar.gz),
// SBOM artifacts and the updater manifest; we only link installers.
const installers = assets.filter(
  (a) =>
    !/\.(sha256|sig|txt)$/.test(a.name) &&
    !/^4da-cli-/.test(a.name) &&
    !/\.app\.tar\.gz$/.test(a.name) &&
    !/^sbom-/.test(a.name) &&
    a.name !== 'latest.json'
);

// key -> predicate describing the installer's SHAPE, not its name.
const shapes = {
  win:           (n) => /x64[-_]setup\.exe$/i.test(n),
  macArm:        (n) => /aarch64\.dmg$/i.test(n),
  macIntel:      (n) => /x64\.dmg$/i.test(n),
  linuxAppImage: (n) => /amd64\.AppImage$/i.test(n),
  linuxDeb:      (n) => /amd64\.deb$/i.test(n),
  linuxRpm:      (n) => /x86_64\.rpm$/i.test(n),
};

const found = {};
const problems = [];
for (const [key, pred] of Object.entries(shapes)) {
  const hits = installers.filter((a) => pred(a.name));
  if (hits.length === 0) problems.push(`${key}: no asset matches`);
  else if (hits.length > 1) problems.push(`${key}: ambiguous — ${hits.map((h) => h.name).join(', ')}`);
  else found[key] = hits[0];
}

const unclaimed = installers.filter((a) => !Object.values(found).some((f) => f.name === a.name));
if (unclaimed.length) problems.push(`unclaimed installers: ${unclaimed.map((u) => u.name).join(', ')}`);

console.log(`${TAG}: ${assets.length} assets, ${installers.length} installers`);
for (const [k, a] of Object.entries(found)) {
  console.log(`  ${k.padEnd(14)} ${(Math.round(a.size / 1048576) + ' MB').padStart(8)}  ${a.name}`);
}
if (problems.length) {
  console.error('\nREFUSING — the download page would be wrong:');
  problems.forEach((p) => console.error('  ' + p));
  process.exit(1);
}
if (!APPLY) { console.log('\n(dry run — pass --apply to write)'); process.exit(0); }

let src = fs.readFileSync(FILE, 'utf8');
src = src.replace(/const VERSION = "[^"]+";/, `const VERSION = "${VERSION}";`);
src = src.replace(/const TAG = "[^"]+";/, `const TAG = "${TAG}";`);
src = src.replace(/verified against the v[\d.]+ release/, `verified against the ${TAG} release`);
src = src.replace(/gh release view v[\d.]+ --repo/, `gh release view ${TAG} --repo`);

for (const [key, a] of Object.entries(found)) {
  // Replace the url template and the size inside this key's object literal.
  // Entries are single-line object literals whose template urls contain `}`
  // (`${base}`), so capture the rest of the LINE greedily and let the final
  // `\}` backtrack to the entry's closing brace.
  const objRe = new RegExp(`(${key}:\\s*\\{)(.*)(\\},?)`);
  const m = src.match(objRe);
  if (!m) { console.error(`could not locate entry for ${key}`); process.exit(1); }
  let body = m[2];
  body = body.replace(/url:\s*`[^`]*`/, 'url: `${base}/' + a.name.replace(/\$/g, '\$') + '`');
  body = body.replace(/size:\s*"[^"]*"/, `size: "~${Math.round(a.size / 1048576)} MB"`);
  src = src.replace(objRe, `$1${body}$3`);
}

fs.writeFileSync(FILE, src);

// Verify what we wrote: no stale version, and every url matches a real asset.
const out = fs.readFileSync(FILE, 'utf8');
const stale = out.split('\n').filter((l) => /4DA\.Home|1\.0\.0/.test(l));
if (stale.length) {
  console.error('\nWARNING: stale references survived:');
  stale.forEach((l) => console.error('  ' + l.trim()));
  process.exit(1);
}
for (const a of Object.values(found)) {
  if (!out.includes(a.name)) { console.error(`written file is missing ${a.name}`); process.exit(1); }
}
console.log(`\nupdated ${FILE} -> ${TAG} (all 6 urls match real assets)`);
