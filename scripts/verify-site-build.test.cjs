// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const test = require('node:test');

const { parseWrittenCount, decide } = require('./verify-site-build.cjs');

test('parseWrittenCount reads the Eleventy summary line', () => {
  assert.equal(parseWrittenCount('[11ty] Copied 22 Wrote 128 files in 1.31 seconds (v3.1.8)'), 128);
  assert.equal(parseWrittenCount('[11ty] Wrote 1 file in 0.2 seconds'), 1);
});

test('parseWrittenCount reports 0 for the dry-run line that used to pass the hook', () => {
  assert.equal(parseWrittenCount('[11ty] Wrote 0 files in 1.83 seconds (v3.1.8)'), 0);
});

test('parseWrittenCount returns null when there is no summary line', () => {
  assert.equal(parseWrittenCount('npm warn exec The following package was not found'), null);
});

test('decide builds whenever the locked deps are installed', () => {
  assert.equal(decide({ depsInstalled: true, siteChanged: false }), 'build');
  assert.equal(decide({ depsInstalled: true, siteChanged: true }), 'build');
});

test('decide installs before building only when the push touches site/', () => {
  assert.equal(decide({ depsInstalled: false, siteChanged: true }), 'install-then-build');
  assert.equal(decide({ depsInstalled: false, siteChanged: false }), 'skip');
});
