// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Main is aligned today, so a green run of the real check proves nothing on its
 * own. These tests drive the pure core and the lockfile parsers with fixtures,
 * including the exact shape main had between #1044 and #1062 (crate
 * tauri-plugin-updater 2.14.0, npm @tauri-apps/plugin-updater 2.13.3). The real
 * lockfiles are never edited.
 */
const test = require('node:test');
const assert = require('node:assert');
const {
  evaluate,
  check,
  parseCargoLock,
  parsePnpmLockRoot,
  npmCounterpart,
  majorMinor,
  summarize,
  annotations,
  exitCodeFor,
} = require('./check-tauri-alignment.cjs');

const DECLARED = ['@tauri-apps/api', '@tauri-apps/plugin-opener', '@tauri-apps/plugin-updater', '@tauri-apps/cli', 'react'];
const CRATES = {
  tauri: ['2.12.5'],
  'tauri-plugin-opener': ['2.7.2'],
  'tauri-plugin-updater': ['2.14.1'],
  'tauri-plugin-deep-link': ['2.6.3'],
  'tauri-plugin-autostart': ['2.7.0'],
  'tauri-plugin-notification': ['2.8.0'],
  'tauri-plugin-single-instance': ['2.5.5'],
  'tauri-plugin': ['2.6.0'], // the plugin-builder crate, not a plugin
  'tauri-build': ['2.6.1'],
  serde: ['1.0.228'],
};
const NPM = {
  '@tauri-apps/api': '2.12.4',
  '@tauri-apps/plugin-opener': '2.7.2',
  '@tauri-apps/plugin-updater': '2.14.1',
  '@tauri-apps/cli': '2.12.4',
  react: '19.3.0',
};

test('crate -> npm mapping follows tauri-cli: tauri is the api, tauri-plugin-<x> is plugin-<x>', () => {
  assert.strictEqual(npmCounterpart('tauri'), '@tauri-apps/api');
  assert.strictEqual(npmCounterpart('tauri-plugin-updater'), '@tauri-apps/plugin-updater');
  assert.strictEqual(npmCounterpart('tauri-plugin'), null);
  assert.strictEqual(npmCounterpart('tauri-build'), null);
  assert.strictEqual(npmCounterpart('tauri-runtime-wry'), null);
  assert.strictEqual(majorMinor('2.14.1'), '2.14');
  assert.strictEqual(majorMinor('2.0.0-rc.3'), '2.0');
  assert.strictEqual(majorMinor('link:../foo'), null);
});

test('aligned: today\'s main passes, every declared pair is compared, patch skew is fine', () => {
  const r = evaluate(CRATES, NPM, DECLARED);
  assert.strictEqual(r.ok, true);
  assert.strictEqual(r.inconclusive, false);
  assert.strictEqual(exitCodeFor(r), 0);
  assert.deepStrictEqual(
    r.pairs.map((p) => [p.crate, p.npm, p.status]),
    [
      ['tauri', '@tauri-apps/api', 'aligned'], // 2.12.5 vs 2.12.4: same major.minor
      ['tauri-plugin-opener', '@tauri-apps/plugin-opener', 'aligned'],
      ['tauri-plugin-updater', '@tauri-apps/plugin-updater', 'aligned'],
    ],
  );
  assert.strictEqual(summarize(r), '');
});

test('THE case: mismatched plugin (main between #1044 and #1062) fails with the fix', () => {
  const r = evaluate({ ...CRATES, 'tauri-plugin-updater': ['2.14.0'] }, { ...NPM, '@tauri-apps/plugin-updater': '2.13.3' }, DECLARED);
  assert.strictEqual(r.ok, false);
  assert.strictEqual(exitCodeFor(r), 1);
  assert.deepStrictEqual(r.mismatches.map((p) => p.crate), ['tauri-plugin-updater']);
  assert.strictEqual(r.mismatches[0].reason, 'crate-ahead');
  assert.strictEqual(
    summarize(r),
    'Tauri crate/npm major.minor mismatch (tauri-plugin-updater 2.14 vs @tauri-apps/plugin-updater 2.13): the next release build will fail',
  );
  // The annotation names the fix: same PR, check the version first, then pnpm install.
  const pkg = '{\n  "dependencies": {\n    "@tauri-apps/plugin-updater": "^2.13.3"\n  }\n}\n';
  const [a] = annotations({ ...r, missing: [] }, pkg);
  assert.match(a, /^::warning file=package\.json,line=3,title=Tauri version mismatch/);
  assert.match(a, /SAME PR/);
  assert.match(a, /npm view @tauri-apps\/plugin-updater@~2\.14\.0 version/);
  assert.match(a, /pnpm install/);
  assert.doesNotMatch(a, /\n/, 'a workflow command must stay on one line');
});

test('mismatched api: tauri 2.12 vs @tauri-apps/api 2.11 fails (the #966 shape)', () => {
  const r = evaluate({ ...CRATES, tauri: ['2.12.1'] }, { ...NPM, '@tauri-apps/api': '2.11.1' }, DECLARED);
  assert.strictEqual(r.ok, false);
  assert.deepStrictEqual(r.mismatches.map((p) => `${p.crate}:${p.npm}`), ['tauri:@tauri-apps/api']);
});

test('npm ahead of the crate (a one-sided npm Dependabot PR) is a mismatch with the reverse fix', () => {
  const r = evaluate(CRATES, { ...NPM, '@tauri-apps/plugin-opener': '2.8.0' }, DECLARED);
  assert.strictEqual(r.ok, false);
  assert.strictEqual(r.mismatches[0].reason, 'npm-ahead');
  const [a] = annotations({ ...r, missing: [] }, '');
  assert.match(a, /cargo update -p tauri-plugin-opener/);
  assert.match(a, /^::warning file=package\.json,title=/, 'no line when the declaration is not found');
});

test('a major bump on one side is a mismatch too', () => {
  const r = evaluate({ ...CRATES, tauri: ['3.0.0'] }, NPM, DECLARED);
  assert.deepStrictEqual(r.mismatches.map((p) => p.crate), ['tauri']);
});

test('a plugin without an npm counterpart is ignored, not flagged, whatever its version', () => {
  // deep-link 9.9 would be a "mismatch" against anything; it has no npm package here.
  const r = evaluate({ ...CRATES, 'tauri-plugin-deep-link': ['9.9.0'] }, NPM, DECLARED);
  assert.strictEqual(r.ok, true);
  assert.strictEqual(exitCodeFor(r), 0);
  assert.deepStrictEqual(
    r.ignored.map((i) => i.crate).sort(),
    ['tauri-plugin-autostart', 'tauri-plugin-deep-link', 'tauri-plugin-notification', 'tauri-plugin-single-instance'],
  );
  assert.ok(!r.pairs.some((p) => p.crate === 'tauri-plugin-deep-link'));
});

test('an npm package resolved in the lockfile but NOT declared in package.json is not paired', () => {
  // Pairing keys off package.json, so a transitive @tauri-apps/plugin-x cannot create a pair.
  const r = evaluate(CRATES, { ...NPM, '@tauri-apps/plugin-notification': '1.0.0' }, DECLARED);
  assert.strictEqual(r.ok, true);
});

test('missing lockfile entry: declared npm package absent from pnpm-lock.yaml is UNKNOWN, never a mismatch', () => {
  const npm = { ...NPM };
  delete npm['@tauri-apps/plugin-updater'];
  const r = evaluate(CRATES, npm, DECLARED);
  assert.strictEqual(r.ok, true, 'unobserved is not a mismatch');
  assert.strictEqual(r.inconclusive, true);
  assert.strictEqual(exitCodeFor(r), 2);
  assert.match(r.unknown[0].reason, /no resolved version.*pnpm install/);
  assert.strictEqual(summarize(r), '');
});

test('missing lockfile entry: declared npm package whose crate is not in Cargo.lock is a note only', () => {
  const crates = { ...CRATES };
  delete crates['tauri-plugin-opener'];
  const r = evaluate(crates, NPM, DECLARED);
  assert.strictEqual(exitCodeFor(r), 0);
  assert.ok(r.notes.some((n) => /plugin-opener is in package\.json but tauri-plugin-opener is not in src-tauri\/Cargo\.lock/.test(n.message)));
});

test('two versions of one crate in Cargo.lock is UNKNOWN (tauri-cli cannot pick either)', () => {
  const r = evaluate({ ...CRATES, 'tauri-plugin-updater': ['2.13.0', '2.14.1'] }, NPM, DECLARED);
  assert.strictEqual(r.ok, true);
  assert.strictEqual(r.unknown[0].crate, 'tauri-plugin-updater');
  assert.match(r.unknown[0].reason, /2 versions/);
});

test('@tauri-apps/cli drift is a WARNING note, never a failure', () => {
  const r = evaluate(CRATES, { ...NPM, '@tauri-apps/cli': '2.11.5' }, DECLARED);
  assert.strictEqual(r.ok, true);
  assert.strictEqual(exitCodeFor(r), 0);
  const cli = r.notes.find((n) => n.message.startsWith('@tauri-apps/cli'));
  assert.strictEqual(cli.level, 'warning');
  const aligned = evaluate(CRATES, NPM, DECLARED).notes.find((n) => n.message.startsWith('@tauri-apps/cli'));
  assert.strictEqual(aligned.level, 'note');
});

test('a missing lockfile FILE is inconclusive (exit 2), not aligned and not a mismatch', () => {
  const r = check((rel) => (rel === 'src-tauri/Cargo.lock' ? null : rel === 'package.json' ? '{}' : ''));
  assert.deepStrictEqual(r.missing, ['src-tauri/Cargo.lock']);
  assert.strictEqual(exitCodeFor(r), 2);
  assert.match(annotations(r, '')[0], /inconclusive.*could not read src-tauri\/Cargo\.lock/);
});

// ── Parsers, on fixtures shaped like the real files ──

const CARGO_LOCK_FIXTURE = [
  '# This file is automatically @generated by Cargo.',
  'version = 4',
  '',
  '[[package]]',
  'name = "tauri"',
  'version = "2.12.5"',
  'source = "registry+https://github.com/rust-lang/crates.io-index"',
  'dependencies = [',
  ' "tauri-build",',
  ']',
  '',
  '[[package]]',
  'name = "tauri-plugin-updater"',
  'version = "2.14.0"',
  '',
  '[[package]]',
  'name = "fourda"',
  'version = "1.0.3"',
  'dependencies = [',
  ' "tauri-plugin-updater",',
  ']',
].join('\r\n'); // CRLF: a Windows checkout with autocrlf

test('Cargo.lock: names and versions are paired per [[package]], CRLF included', () => {
  const crates = parseCargoLock(CARGO_LOCK_FIXTURE);
  assert.deepStrictEqual(crates.tauri, ['2.12.5']);
  assert.deepStrictEqual(crates['tauri-plugin-updater'], ['2.14.0']);
  assert.deepStrictEqual(crates.fourda, ['1.0.3']);
  assert.strictEqual(crates['tauri-build'], undefined, 'a dependency list entry is not a package');
});

const PNPM_LOCK_FIXTURE = `lockfileVersion: '9.0'

settings:
  autoInstallPeers: true

importers:

  .:
    dependencies:
      '@tauri-apps/api':
        specifier: ^2.12.4
        version: 2.12.4
      '@tauri-apps/plugin-updater':
        specifier: ^2.13.3
        version: 2.13.3
      react-dom:
        specifier: ^19.3.0
        version: 19.3.0(react@19.3.0)
    devDependencies:
      '@tauri-apps/cli':
        specifier: ^2.12.4
        version: 2.12.4

  site:
    dependencies:
      '@tauri-apps/plugin-updater':
        specifier: ^9.0.0
        version: 9.0.0

packages:

  '@tauri-apps/api@2.12.4':
    resolution: {integrity: sha512-x}
`;

test('pnpm-lock.yaml: only the ROOT importer is read; peer suffixes are cut', () => {
  const r = parsePnpmLockRoot(PNPM_LOCK_FIXTURE);
  assert.deepStrictEqual(r, {
    '@tauri-apps/api': '2.12.4',
    '@tauri-apps/plugin-updater': '2.13.3', // not site's 9.0.0
    'react-dom': '19.3.0',
    '@tauri-apps/cli': '2.12.4',
  });
});

test('pnpm-lock.yaml: CRLF and a lockfile with no root importer', () => {
  assert.strictEqual(parsePnpmLockRoot(PNPM_LOCK_FIXTURE.replace(/\n/g, '\r\n'))['@tauri-apps/api'], '2.12.4');
  assert.deepStrictEqual(parsePnpmLockRoot("lockfileVersion: '9.0'\n\nimporters:\n\n  site:\n    dependencies:\n      a:\n        version: 1.0.0\n"), {});
});

test('end to end on fixtures: the parsed #1044 shape fails through check()', () => {
  const files = {
    'src-tauri/Cargo.lock': CARGO_LOCK_FIXTURE,
    'pnpm-lock.yaml': PNPM_LOCK_FIXTURE,
    'package.json': JSON.stringify({
      dependencies: { '@tauri-apps/api': '^2.12.4', '@tauri-apps/plugin-updater': '^2.13.3' },
      devDependencies: { '@tauri-apps/cli': '^2.12.4' },
    }),
  };
  const r = check((rel) => files[rel] ?? null);
  assert.strictEqual(exitCodeFor(r), 1);
  assert.match(summarize(r), /tauri-plugin-updater 2\.14 vs @tauri-apps\/plugin-updater 2\.13/);
});

// Deliberately NO test asserting the real checkout is aligned: test:scripts runs
// in the required Repo guards job, so such a test would turn this warning into
// the hard gate that one-sided Dependabot PRs can never pass.
test('the real checkout parses: every input is readable and at least one pair is compared', () => {
  const r = check();
  assert.deepStrictEqual(r.missing, []);
  assert.ok(r.pairs.length >= 1, 'no Tauri pair found; the lockfile parsers no longer match the real files');
});
