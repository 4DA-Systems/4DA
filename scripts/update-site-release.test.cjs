// update-site-release.cjs rewrites site/src/_data/release.js from a real
// release's asset list. CodeQL js/identity-replacement #13 flagged
// `name.replace(/\$/g, '\$')` — '\$' is just '$', so the "escape" did nothing
// and a `$&` / `$1` in a name would have been expanded by String.replace.
// These tests pin the replacement: names are validated, then inserted literally.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const { SAFE_ASSET_NAME, selectInstallers, matchInstallers, rewriteRelease } = require('./update-site-release.cjs');

const RELEASE_JS = fs.readFileSync(path.join(__dirname, '..', 'site', 'src', '_data', 'release.js'), 'utf8');
const MB = 1048576;

function assetsFor(v) {
  return [
    { name: `4DA_${v}_x64-setup.exe`, size: 121 * MB },
    { name: `4DA_${v}_aarch64.dmg`, size: 131 * MB },
    { name: `4DA_${v}_x64.dmg`, size: 134 * MB },
    { name: `4DA_${v}_amd64.AppImage`, size: 202 * MB },
    { name: `4DA_${v}_amd64.deb`, size: 134 * MB },
    { name: `4DA-${v}-1.x86_64.rpm`, size: 134 * MB },
    { name: `4DA_${v}_x64-setup.exe.sig`, size: 1 },
    { name: 'latest.json', size: 1 },
    { name: `sbom-${v}.json`, size: 1 },
  ];
}

test('a normal release rewrites every url, size, version and tag', () => {
  const { found, problems } = matchInstallers(selectInstallers(assetsFor('9.8.7')));
  assert.deepEqual(problems, []);
  const out = rewriteRelease(RELEASE_JS, 'v9.8.7', found);
  assert.match(out, /const VERSION = "9\.8\.7";/);
  assert.match(out, /const TAG = "v9\.8\.7";/);
  for (const a of Object.values(found)) {
    assert.ok(out.includes('url: `${base}/' + a.name + '`'), `${a.name} written verbatim`);
  }
  assert.match(out, /win:.*size: "~121 MB"/);
  assert.ok(!out.includes('1.0.2_'), 'no stale asset name survives');
  // The template-literal prefix is untouched (a string replacement would have
  // been free to reinterpret `$` sequences here).
  assert.equal(out.match(/\$\{base\}\//g).length, 6);
});

test('an asset name with replacement or template metacharacters is refused', () => {
  for (const bad of ['4DA_$&_x64-setup.exe', '4DA_$1_x64-setup.exe', '4DA_`x`_x64-setup.exe', '4DA_${x}_x64-setup.exe', '4DA 1_x64-setup.exe', '4DA\\1_x64-setup.exe']) {
    assert.equal(SAFE_ASSET_NAME.test(bad), false, bad);
    const assets = assetsFor('9.8.7').map((a) => (a.name.endsWith('x64-setup.exe') ? { ...a, name: bad } : a));
    const { problems } = matchInstallers(selectInstallers(assets));
    assert.ok(problems.some((p) => p.startsWith('win: unsafe asset name')), `${bad} must be refused`);
  }
});

test('rewriteRelease refuses an unsafe name even if a caller skips matchInstallers', () => {
  assert.throws(() => rewriteRelease(RELEASE_JS, 'v9.8.7', { win: { name: 'x_$&.exe', size: MB } }), /unsafe asset name/);
});

// The site says "the download is older than these screenshots" while VERSION
// (the newest published release) trails NEXT_VERSION. Running this script for
// the release that catches up must retire that line with no hand edit.
test('publishing NEXT_VERSION flips `published`, retiring the stale-download line', async () => {
  const os = require('node:os');
  const { pathToFileURL } = require('node:url');
  const next = RELEASE_JS.match(/const NEXT_VERSION = "([^"]+)";/);
  assert.ok(next, 'release.js declares NEXT_VERSION');
  const load = async (src, name) => {
    const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'site-release-')), name);
    fs.writeFileSync(file, src);
    return (await import(pathToFileURL(file).href)).default;
  };
  const before = await load(RELEASE_JS, 'before.mjs');
  if (before.version !== next[1]) assert.equal(before.published, false);
  const { found } = matchInstallers(selectInstallers(assetsFor(next[1])));
  const after = await load(rewriteRelease(RELEASE_JS, `v${next[1]}`, found), 'after.mjs');
  assert.equal(after.version, next[1]);
  assert.equal(after.next, next[1]);
  assert.equal(after.published, true);
  assert.ok(after.files.win.url.endsWith(`/v${next[1]}/4DA_${next[1]}_x64-setup.exe`));
});
