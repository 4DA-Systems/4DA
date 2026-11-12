// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for scripts/pin-updater-manifest.cjs (audit 2026-11-11, R5). The
// fixture is the shape of the real v1.0.3 desktop-latest/latest.json, whose
// URLs all point at /releases/latest/download/.
//
// Run: node --test scripts/pin-updater-manifest.test.cjs   (or `pnpm run test:scripts`)

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { pinManifest, payloadUrls, checkUrls } = require('./pin-updater-manifest.cjs');

const REPO = '4DA-Systems/4DA';
const latest = (f) => `https://github.com/${REPO}/releases/latest/download/${f}`;

function manifest(v) {
  return {
    version: v,
    notes: '',
    pub_date: '2026-11-20T00:00:00Z',
    platforms: {
      'windows-x86_64': { signature: 's', url: latest(`4DA_${v}_x64-setup.exe`) },
      'darwin-aarch64': { signature: 's', url: latest('4DA_aarch64.app.tar.gz') },
      'darwin-x86_64': { signature: 's', url: latest('4DA_x64.app.tar.gz') },
      'linux-x86_64': { signature: 's', url: latest(`4DA_${v}_amd64.AppImage`) },
    },
  };
}
const assetsOf = (v) => [`4DA_${v}_x64-setup.exe`, '4DA_aarch64.app.tar.gz', '4DA_x64.app.tar.gz', `4DA_${v}_amd64.AppImage`, 'latest.json'];

test('every "latest" URL is pinned to the release tag', () => {
  const { manifest: out, problems } = pinManifest(manifest('1.0.4'), REPO, 'v1.0.4', assetsOf('1.0.4'));
  assert.deepEqual(problems, []);
  for (const u of payloadUrls(out)) {
    assert.match(u, /^https:\/\/github\.com\/4DA-Systems\/4DA\/releases\/download\/v1\.0\.4\/[^/]+$/);
  }
  assert.equal(out.platforms['windows-x86_64'].signature, 's', 'signatures are untouched');
});

test('pinning is idempotent (a re-run of the job does not fail)', () => {
  const once = pinManifest(manifest('1.0.4'), REPO, 'v1.0.4', assetsOf('1.0.4')).manifest;
  const twice = pinManifest(once, REPO, 'v1.0.4', assetsOf('1.0.4'));
  assert.deepEqual(twice.problems, []);
  assert.deepEqual(twice.manifest, once);
});

test('a payload the release does not carry is refused', () => {
  const { problems } = pinManifest(manifest('1.0.4'), REPO, 'v1.0.4', assetsOf('1.0.4').filter((n) => !n.endsWith('.AppImage')));
  assert.ok(problems.some((p) => p.startsWith('linux-x86_64: 4DA_1.0.4_amd64.AppImage is not an asset')));
});

test('a manifest for another version, another repo or another tag is refused', () => {
  assert.ok(pinManifest(manifest('1.0.3'), REPO, 'v1.0.4', assetsOf('1.0.3')).problems.some((p) => /version 1\.0\.3 does not match/.test(p)));
  const foreign = manifest('1.0.4');
  foreign.platforms['linux-x86_64'].url = 'https://example.com/4DA.AppImage';
  assert.ok(pinManifest(foreign, REPO, 'v1.0.4', assetsOf('1.0.4')).problems.some((p) => p.startsWith('linux-x86_64:')));
  const otherTag = pinManifest(manifest('1.0.4'), REPO, 'v1.0.4', assetsOf('1.0.4')).manifest;
  assert.ok(pinManifest(otherTag, REPO, 'v1.0.5', assetsOf('1.0.4')).problems.length > 0);
});

test('check-urls passes only when every URL answers 200, retrying a lagging CDN', async () => {
  const urls = ['https://a/1', 'https://a/2'];
  let calls = 0;
  const fetchImpl = async (u) => {
    calls++;
    // /2 is 404 on the first round (CDN lag), 200 after.
    return { status: u.endsWith('/2') && calls <= 2 ? 404 : 200, body: null };
  };
  const ok = await checkUrls(urls, { attempts: 3, delayMs: 0, fetchImpl, sleep: async () => undefined });
  assert.equal(ok.ok, true);

  const never = await checkUrls(urls, {
    attempts: 2,
    delayMs: 0,
    fetchImpl: async (u) => ({ status: u.endsWith('/1') ? 200 : 404, body: null }),
    sleep: async () => undefined,
  });
  assert.equal(never.ok, false);
  assert.deepEqual(never.failed.map((f) => [f.url, f.status]), [['https://a/2', 404]]);

  const offline = await checkUrls(['https://a/3'], {
    attempts: 1,
    fetchImpl: async () => { throw new Error('getaddrinfo ENOTFOUND'); },
  });
  assert.equal(offline.ok, false);
  assert.match(offline.failed[0].error, /ENOTFOUND/);
});

test('the real v1.0.3 manifest shape is exactly what this fixes', () => {
  // Every URL in the published desktop-latest/latest.json for v1.0.3 used
  // /releases/latest/download/ (checked 2026-11-12).
  for (const u of payloadUrls(manifest('1.0.3'))) assert.match(u, /\/releases\/latest\/download\//);
});
