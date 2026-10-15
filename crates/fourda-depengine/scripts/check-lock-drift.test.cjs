// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// node --test scripts/check-lock-drift.test.cjs
//
// Both Dependabot directions must be able to merge on their own: an app-side
// minor bump warns; a crate-side bump past the app fails until the app
// catches up, then passes.

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { parseLock, checkDrift } = require('./check-lock-drift.cjs');

const lock = (pkgs) =>
  '# generated\nversion = 4\n' +
  Object.entries(pkgs)
    .map(([name, v]) => `\n[[package]]\nname = "${name}"\nversion = "${v}"\n`)
    .join('');

const base = { regex: '1.13.1', semver: '1.0.28', 'fourda-depengine': '0.1.0' };
const app = { regex: '1.13.1', semver: '1.0.28', tauri: '2.0.0' };

test('same versions: nothing to report', () => {
  const r = checkDrift(parseLock(lock(base)), parseLock(lock(app)), parseLock(lock(base)));
  assert.deepStrictEqual([r.errors, r.warnings, r.notes], [[], [], []]);
  assert.strictEqual(r.shared, 2);
});

test('app-side Dependabot minor bump (regex 1.13 -> 1.14 in src-tauri): warning, not failure', () => {
  const r = checkDrift(
    parseLock(lock(base)),
    parseLock(lock({ ...app, regex: '1.14.0' })),
    parseLock(lock(base)),
  );
  assert.strictEqual(r.errors.length, 0);
  assert.strictEqual(r.warnings.length, 1);
  assert.match(r.warnings[0], /regex: crate 1\.13\.1, app 1\.14\.0/);
});

test('crate-side bump past the app: error', () => {
  const crate = { ...base, regex: '1.14.0' };
  const r = checkDrift(parseLock(lock(crate)), parseLock(lock(app)), parseLock(lock(base)));
  assert.strictEqual(r.errors.length, 1);
  assert.match(r.errors[0], /moved the crate's lockfile away/);
});

test('crate-side bump to the line the app already moved to: passes', () => {
  const crate = { ...base, regex: '1.14.0' };
  const r = checkDrift(
    parseLock(lock(crate)),
    parseLock(lock({ ...app, regex: '1.14.2' })),
    parseLock(lock(base)),
  );
  assert.deepStrictEqual(r.errors, []);
  assert.deepStrictEqual(r.warnings, []);
  assert.strictEqual(r.notes.length, 1, 'patch difference is a note');
});

test('no base known (schedule, dispatch): drift warns, never fails', () => {
  const crate = { ...base, regex: '1.14.0' };
  const r = checkDrift(parseLock(lock(crate)), parseLock(lock(app)), null);
  assert.strictEqual(r.errors.length, 0);
  assert.strictEqual(r.warnings.length, 1);
});

test('the CLI exits 0 on app-led drift and 1 on crate-led drift', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'lock-drift-'));
  const write = (name, pkgs) => {
    const p = path.join(dir, name);
    fs.writeFileSync(p, lock(pkgs));
    return p;
  };
  const script = path.join(__dirname, 'check-lock-drift.cjs');
  const run = (crate, appLock) => {
    try {
      execFileSync(
        process.execPath,
        [script, '--base-crate-lock', write('base.lock', base), crate, appLock],
        { stdio: 'pipe' },
      );
      return 0;
    } catch (e) {
      return e.status;
    }
  };
  assert.strictEqual(run(write('c.lock', base), write('a.lock', { ...app, regex: '1.14.0' })), 0);
  assert.strictEqual(run(write('c2.lock', { ...base, regex: '1.14.0' }), write('a2.lock', app)), 1);
  fs.rmSync(dir, { recursive: true, force: true });
});
