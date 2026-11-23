// Tests for the pure logic of installer-smoke.cjs. Run: node --test scripts/installer-smoke.test.cjs
'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  DEFAULT_READY,
  DEFAULT_FATAL,
  findRelease,
  pickInstaller,
  parseShasums,
  checkHash,
  sha256File,
  scanLog,
  verdict,
  annotation,
  parseArgs,
} = require('./installer-smoke.cjs');

// The v1.0.3 release's real asset list (names only matter here).
const V103 = [
  '4DA-1.0.3-1.x86_64.rpm', '4DA-1.0.3-1.x86_64.rpm.sha256', '4DA-1.0.3-1.x86_64.rpm.sig',
  '4da-cli-aarch64-apple-darwin', '4da-cli-aarch64-apple-darwin.sha256',
  '4da-cli-x86_64-apple-darwin', '4da-cli-x86_64-pc-windows-msvc.exe', '4da-cli-x86_64-unknown-linux-gnu',
  '4DA_1.0.3_aarch64.dmg', '4DA_1.0.3_aarch64.dmg.sha256',
  '4DA_1.0.3_amd64.AppImage', '4DA_1.0.3_amd64.AppImage.sha256', '4DA_1.0.3_amd64.AppImage.sig',
  '4DA_1.0.3_amd64.deb', '4DA_1.0.3_amd64.deb.sha256', '4DA_1.0.3_amd64.deb.sig',
  '4DA_1.0.3_x64-setup.exe', '4DA_1.0.3_x64-setup.exe.sha256', '4DA_1.0.3_x64-setup.exe.sig',
  '4DA_1.0.3_x64.dmg', '4DA_1.0.3_x64.dmg.sha256',
  '4DA_aarch64.app.tar.gz', '4DA_aarch64.app.tar.gz.sig', '4DA_x64.app.tar.gz', '4DA_x64.app.tar.gz.sig',
  'latest.json', 'sbom-npm.json', 'sbom-npm.sig.bundle', 'sbom-rust.json', 'sbom-rust.sig.bundle',
  'SHASUMS256-aarch64-apple-darwin.txt', 'SHASUMS256-x86_64-unknown-linux-gnu.txt', 'SHASUMS256.txt',
].map((name, id) => ({ id, name, size: 1 }));

const SUMS_103 = [
  '583712d7275918cf7f259ff439b5477a466577e7a95a8925e27c6e994433af00  4DA-1.0.3-1.x86_64.rpm',
  '4375cac25fe67ad248a51b2020c3bedfd95aa3d9e58d513042b291c3f476362b  4DA_1.0.3_aarch64.dmg',
  '1c9ac619999844f74eabad94bd9c928049ecd473dc362e0fd501f96105aec9d8  4DA_1.0.3_amd64.AppImage',
  'b8f95dfe85d1b20536cd7be4f9b93f1350872a22b3b873de368cabd6ce572c60  4DA_1.0.3_amd64.deb',
  'e3ed832c362db746feb3a7286df7edc1ed920e2ddf988bbeb837885161080cef  4DA_1.0.3_x64-setup.exe',
  '17e88d9c44877a6a0d182892a1d7e60b25f60225477e7e482f46f5b42ff7988d  4DA_1.0.3_x64.dmg',
  '',
].join('\n');

test('picks each platform installer from the real v1.0.3 assets by shape', () => {
  const pick = (kind) => pickInstaller(V103, kind).name;
  assert.equal(pick('win'), '4DA_1.0.3_x64-setup.exe');
  assert.equal(pick('macArm'), '4DA_1.0.3_aarch64.dmg');
  assert.equal(pick('macIntel'), '4DA_1.0.3_x64.dmg');
  assert.equal(pick('linuxDeb'), '4DA_1.0.3_amd64.deb');
  assert.equal(pick('linuxAppImage'), '4DA_1.0.3_amd64.AppImage');
});

test('picker refuses a missing, ambiguous, unknown or unsafe installer', () => {
  const noWin = V103.filter((a) => !a.name.endsWith('x64-setup.exe'));
  assert.throws(() => pickInstaller(noWin, 'win'), /no win installer/);
  const twoDebs = [...V103, { id: 99, name: '4DA_1.0.4_amd64.deb', size: 1 }];
  assert.throws(() => pickInstaller(twoDebs, 'linuxDeb'), /ambiguous linuxDeb installer: 4DA_1\.0\.3_amd64\.deb, 4DA_1\.0\.4_amd64\.deb/);
  assert.throws(() => pickInstaller(V103, 'windows'), /unknown kind "windows"/);
  const unsafe = [{ id: 1, name: '4DA $(x)_x64-setup.exe', size: 1 }];
  assert.throws(() => pickInstaller(unsafe, 'win'), /unsafe asset name/);
});

test('picker never mistakes a checksum, signature or updater bundle for an installer', () => {
  const decoys = ['4DA_1.0.3_amd64.deb.sha256', '4DA_1.0.3_amd64.deb.sig', '4DA_x64.app.tar.gz'].map((name, id) => ({ id, name, size: 1 }));
  assert.throws(() => pickInstaller(decoys, 'linuxDeb'), /no linuxDeb installer/);
  assert.throws(() => pickInstaller(decoys, 'macIntel'), /no macIntel installer/);
});

test('finds a draft release by tag, and says what a draft needs when it is invisible', () => {
  const releases = [
    { id: 1, tag_name: 'v1.0.3', draft: false, assets: [] },
    { id: 2, tag_name: 'v1.0.4-rc.1', draft: true, assets: [] },
  ];
  assert.equal(findRelease(releases, 'v1.0.4-rc.1').id, 2);
  assert.equal(findRelease(releases, 'v1.0.3').id, 1);
  assert.throws(() => findRelease(releases, 'v9.9.9'), /contents: write/);
  const dup = [...releases, { id: 3, tag_name: 'v1.0.3', draft: true, assets: [] }];
  assert.throws(() => findRelease(dup, 'v1.0.3'), /2 releases carry tag v1\.0\.3 \(1, 3 \(draft\)\)/);
});

test('parses SHASUMS256.txt, including binary-mode lines', () => {
  const sums = parseShasums(SUMS_103);
  assert.equal(sums.size, 6);
  assert.equal(sums.get('4DA_1.0.3_amd64.deb'), 'b8f95dfe85d1b20536cd7be4f9b93f1350872a22b3b873de368cabd6ce572c60');
  const bin = parseShasums(`${'A'.repeat(64)} *file.exe\r\n`);
  assert.equal(bin.get('file.exe'), 'a'.repeat(64));
});

test('a malformed, conflicting or empty checksum file is an error, not a pass', () => {
  assert.throws(() => parseShasums(`${'a'.repeat(63)}  short.exe`), /line 1 is not/);
  assert.throws(() => parseShasums(`${'a'.repeat(64)}  x.exe\nnot a checksum line`), /line 2 is not/);
  assert.throws(() => parseShasums(`${'a'.repeat(64)}  x.exe\n${'b'.repeat(64)}  x.exe`), /twice with different hashes/);
  assert.throws(() => parseShasums('\n\n'), /lists no files/);
});

test('checksum check passes the real file and fails a single flipped byte', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'installer-smoke-'));
  try {
    const file = path.join(dir, '4DA_9.9.9_x64-setup.exe');
    const body = crypto.randomBytes(3 * 1024 * 1024 + 7); // spans several read chunks
    fs.writeFileSync(file, body);
    const good = crypto.createHash('sha256').update(body).digest('hex');
    const sums = parseShasums(`${good}  4DA_9.9.9_x64-setup.exe\n`);
    assert.equal(sha256File(file), good);
    checkHash(sums, '4DA_9.9.9_x64-setup.exe', sha256File(file));

    body[body.length - 1] ^= 0xff;
    fs.writeFileSync(file, body);
    assert.throws(() => checkHash(sums, '4DA_9.9.9_x64-setup.exe', sha256File(file)), /SHA-256 mismatch for 4DA_9\.9\.9_x64-setup\.exe/);
    assert.throws(() => checkHash(sums, 'other.exe', good), /has no entry for other\.exe/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

const LOG = [
  '2026-11-23T01:00:00.100Z  INFO 4da::startup: 4DA — Personalized Intelligence',
  '2026-11-23T01:00:01.200Z  INFO 4da::startup: Main window shown source="production frontend"',
  `2026-11-23T01:00:01.300Z  INFO 4da::startup: ${DEFAULT_READY} phase=0 elapsed_ms=96`,
].join('\n');

test('finds the readiness line and the first fatal line', () => {
  assert.match(scanLog(LOG, DEFAULT_READY, DEFAULT_FATAL).readyLine, /elapsed_ms=96/);
  assert.equal(scanLog(LOG, DEFAULT_READY, DEFAULT_FATAL).fatalLine, null);
  assert.equal(scanLog(LOG, 'a line no build writes', DEFAULT_FATAL).readyLine, null);
  const panicked = `${LOG}\n2026-11-23T01:00:02Z ERROR 4da::security: Panic detected — zeroizing sensitive memory`;
  assert.match(scanLog(panicked, DEFAULT_READY, DEFAULT_FATAL).fatalLine, /Panic detected/);
});

test('verdict: waits, passes only after the alive window, fails on every miss', () => {
  const ok = { readyLine: 'x', fatalLine: null };
  const none = { readyLine: null, fatalLine: null };
  const base = { budgetMs: 120000, aliveMs: 45000 };
  assert.equal(verdict({ ...base, scan: none, alive: true, elapsedMs: 5000, readyAtMs: null }).status, 'waiting');
  assert.equal(verdict({ ...base, scan: ok, alive: true, elapsedMs: 30000, readyAtMs: 10000 }).status, 'waiting');
  assert.equal(verdict({ ...base, scan: ok, alive: true, elapsedMs: 55000, readyAtMs: 10000 }).status, 'pass');

  const fail = (over) => verdict({ ...base, ...over });
  assert.match(fail({ scan: none, alive: true, elapsedMs: 121000, readyAtMs: null }).reason, /no readiness line .* within 120 s/);
  assert.match(fail({ scan: none, alive: false, elapsedMs: 3000, readyAtMs: null }).reason, /exited 3 s after launch, before its readiness line/);
  assert.match(fail({ scan: ok, alive: false, elapsedMs: 20000, readyAtMs: 10000 }).reason, /exited 10 s after its readiness line/);
  assert.match(fail({ scan: { readyLine: 'x', fatalLine: 'FATAL: disk' }, alive: true, elapsedMs: 50000, readyAtMs: 1000 }).reason, /fatal line: FATAL: disk/);
});

test('annotation text cannot break out of the workflow command', () => {
  assert.equal(annotation('50% done\r\n::warning::x'), '50%25 done%0D%0A::warning::x');
});

test('argument parsing collects repeated --fatal and refuses a missing value', () => {
  const a = parseArgs(['watch', '--pid', '42', '--fatal', 'A', '--fatal', 'B']);
  assert.deepEqual(a._, ['watch']);
  assert.equal(a.pid, '42');
  assert.deepEqual(a.fatal, ['A', 'B']);
  assert.throws(() => parseArgs(['watch', '--pid']), /--pid needs a value/);
});
