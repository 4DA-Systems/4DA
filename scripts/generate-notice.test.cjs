// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Self-tests for the NOTICE generator. The point of the generator is that
// third-party attribution cannot silently drift from what ships, so these tests
// pin the behaviours that would let it drift again: reciprocal licences must be
// called out, strong copyleft must block, dual licences must not be
// misclassified, and the committed NOTICE must actually carry the attribution
// the hand-maintained version was missing.

const assert = require('node:assert/strict');
const fs = require('node:fs');
const test = require('node:test');

const {
  NOTICE_PATH,
  compareVersions,
  findCopyleftBlockers,
  isReciprocal,
  normalizeLicense,
  render,
  summarizeDriftLines,
} = require('./generate-notice.cjs');

// ---------------------------------------------------------------------------
// Licence classification
// ---------------------------------------------------------------------------

test('MPL-2.0 is treated as reciprocal', () => {
  assert.equal(isReciprocal('MPL-2.0'), true);
  assert.equal(isReciprocal('EPL-2.0'), true);
});

test('a dual licence with a permissive branch is NOT reciprocal', () => {
  // uhlc ships as EPL-2.0 OR Apache-2.0 — we can take Apache-2.0, so it does
  // not need the source-availability notice.
  assert.equal(isReciprocal('EPL-2.0 OR Apache-2.0'), false);
  assert.equal(isReciprocal('MIT OR MPL-2.0'), false);
});

test('permissive licences are not reciprocal', () => {
  for (const l of ['MIT', 'Apache-2.0', 'ISC', 'BSD-3-Clause', '0BSD OR MIT OR Apache-2.0']) {
    assert.equal(isReciprocal(l), false, `${l} should not be reciprocal`);
  }
});

test('strong copyleft blocks distribution', () => {
  const blockers = findCopyleftBlockers([
    { name: 'a', version: '1', license: 'GPL-3.0' },
    { name: 'b', version: '1', license: 'AGPL-3.0' },
    { name: 'c', version: '1', license: 'SSPL-1.0' },
  ]);
  assert.deepEqual(blockers.map((b) => b.name), ['a', 'b', 'c']);
});

test('tri-licensed crates with a permissive branch do not block', () => {
  // r-efi is `MIT OR Apache-2.0 OR LGPL-2.1-or-later` and is in the real graph.
  // Flagging it would be a false positive that blocks every build.
  assert.deepEqual(
    findCopyleftBlockers([
      { name: 'r-efi', version: '5.3.0', license: 'MIT OR Apache-2.0 OR LGPL-2.1-or-later' },
    ]),
    []
  );
});

test('a GPL linking exception does not block', () => {
  assert.deepEqual(
    findCopyleftBlockers([
      { name: 'x', version: '1', license: 'GPL-2.0 WITH Classpath-exception-2.0' },
    ]),
    []
  );
});

test('missing licence metadata is surfaced, never silently blank', () => {
  assert.equal(normalizeLicense(null), 'UNKNOWN');
  assert.equal(normalizeLicense(undefined), 'UNKNOWN');
  assert.equal(normalizeLicense(null, 'LICENSE'), 'See bundled licence file');
  assert.equal(normalizeLicense('MIT'), 'MIT');
});

// ---------------------------------------------------------------------------
// Deterministic ordering — output must not churn between runs
// ---------------------------------------------------------------------------

test('versions sort numerically, not lexically', () => {
  assert.ok(compareVersions('0.9.0', '0.10.0') < 0);
  assert.ok(compareVersions('1.2.0', '1.10.0') < 0);
  assert.equal(compareVersions('1.2.3', '1.2.3'), 0);
});

test('prerelease versions compare without throwing', () => {
  assert.doesNotThrow(() => compareVersions('1.0.0-alpha.1', '1.0.0'));
});

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function fixture() {
  return {
    rustCrates: [
      { name: 'cssparser', version: '0.37.0', license: 'MPL-2.0', url: 'https://example.invalid/cssparser' },
      { name: 'serde', version: '1.0.0', license: 'MIT OR Apache-2.0', url: 'https://example.invalid/serde' },
    ],
    npmPackages: [
      { name: 'react', version: '19.0.0', license: 'MIT', url: 'https://example.invalid/react', paths: [] },
    ],
    oflTexts: [{ name: '@fontsource-variable/inter', version: '5.2.8', text: 'SIL OPEN FONT LICENSE' }],
  };
}

test('render is deterministic for identical input', () => {
  assert.equal(render(fixture()), render(fixture()));
});

test('render emits a reciprocal section naming the MPL crate', () => {
  const out = render(fixture());
  assert.match(out, /Reciprocal-Licence Components \(1\)/);
  assert.match(out, /cssparser 0\.37\.0 - MPL-2\.0/);
});

test('render omits the reciprocal section when nothing is reciprocal', () => {
  const f = fixture();
  f.rustCrates = f.rustCrates.filter((c) => c.name !== 'cssparser');
  assert.doesNotMatch(render(f), /Reciprocal-Licence Components/);
});

test('render embeds full OFL licence text', () => {
  assert.match(render(fixture()), /SIL Open Font License 1\.1/);
  assert.match(render(fixture()), /SIL OPEN FONT LICENSE/);
});

test('render marks the file as generated so nobody hand-edits it', () => {
  assert.match(render(fixture()), /THIS FILE IS GENERATED - DO NOT EDIT BY HAND/);
});

test('drift summary reports both additions and removals', () => {
  const summary = summarizeDriftLines('alpha\nshared\n', 'shared\nbravo\n');
  assert.match(summary, /\+ bravo/);
  assert.match(summary, /- alpha/);
});

// ---------------------------------------------------------------------------
// The committed NOTICE — the attribution gaps that motivated this generator
// ---------------------------------------------------------------------------

test('committed NOTICE exists and is marked generated', () => {
  const notice = fs.readFileSync(NOTICE_PATH, 'utf8');
  assert.match(notice, /THIS FILE IS GENERATED - DO NOT EDIT BY HAND/);
});

test('committed NOTICE attributes every shipping MPL-2.0 crate', () => {
  const notice = fs.readFileSync(NOTICE_PATH, 'utf8');
  // MPL-2.0 s3.2 requires notice. None of these were attributed before.
  for (const crate of ['cssparser', 'selectors', 'cssparser-macros', 'dtoa-short', 'option-ext']) {
    assert.match(notice, new RegExp(`^${crate} \\S+ - MPL-2\\.0`, 'm'), `${crate} missing MPL-2.0 attribution`);
  }
  assert.match(notice, /Reciprocal-Licence Components/);
});

test('committed NOTICE ships the OFL text for both bundled fonts', () => {
  const notice = fs.readFileSync(NOTICE_PATH, 'utf8');
  // OFL-1.1 requires the licence to travel with the font software.
  for (const font of ['@fontsource-variable/inter', '@fontsource-variable/jetbrains-mono']) {
    assert.match(notice, new RegExp(`Full Licence Text - ${font.replace(/[/@-]/g, '\\$&')}`), `${font} OFL text missing`);
  }
});

test('committed NOTICE carries ammonia, which the hand-maintained list omitted', () => {
  assert.match(fs.readFileSync(NOTICE_PATH, 'utf8'), /^ammonia \d/m);
});
