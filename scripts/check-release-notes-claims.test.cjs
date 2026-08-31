// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The negative test uses the REAL text that was live on the v1.0.0 release
 * until 2026-08-28. If this gate cannot catch that, it is worthless — a clean
 * run against today's already-corrected releases proves nothing.
 */
const test = require('node:test');
const assert = require('node:assert');
const { scanReleases } = require('./check-release-notes-claims.cjs');

// Verbatim opening of the v1.0.0 release body as published 2026-04-20.
// retired-ok: historical record — this is the text the gate must reject, quoted so it can be tested
const V1_0_0_ORIGINAL_BODY = [
  '**4DA reads the internet for developers — privately, locally — and gets sharper every day.**',
  '',
  "It learns from how you engage with what it shows you. Yesterday's noise becomes tomorrow's signal.",
  '',
  '## Download',
  '',
  'Pick your platform at **[4da.ai/download](https://4da.ai/download)**.',
].join('\n');

const CORRECTED_BODY = [
  "**4DA reads the internet for developers — privately, locally. Your codebase decides what's relevant.**",
  '',
  'Every item is scored against your actual stack, and everything else is rejected.',
  '',
  "Yesterday's noise becomes tomorrow's signal.",
].join('\n');

test('the real v1.0.0 body that shipped for four months is caught', () => {
  const findings = scanReleases([{ tag_name: 'v1.0.0', body: V1_0_0_ORIGINAL_BODY }]);
  assert.ok(findings.length >= 2, `expected both retired claims, got ${findings.length}`);
  const phrases = findings.map((f) => f.phrase.toLowerCase()).join(' | ');
  assert.match(phrases, /sharper every day/);
  assert.match(phrases, /learns from how you engage/);
  assert.strictEqual(findings[0].tag, 'v1.0.0');
});

test('the corrected body passes, including the claim AD-030 deliberately kept', () => {
  // "Yesterday's noise becomes tomorrow's signal" is true and explicitly NOT
  // retired. A gate that flags it would push someone into deleting good copy.
  assert.deepStrictEqual(scanReleases([{ tag_name: 'v1.0.0', body: CORRECTED_BODY }]), []);
});

test('the release NAME is scanned, not just the body', () => {
  const findings = scanReleases([
    { tag_name: 'v9.9.9', name: '4DA v9.9.9 — now gets sharper every day', body: 'clean' },
  ]);
  assert.strictEqual(findings.length, 1);
  assert.strictEqual(findings[0].field, 'name');
});

test('a clean release set produces no findings', () => {
  assert.deepStrictEqual(
    scanReleases([
      { tag_name: 'mcp-v5.0.2', body: 'Bug fixes and dependency bumps.' },
      { tag_name: 'v1.0.0', body: CORRECTED_BODY },
      { tag_name: 'v0.0.5-test', body: '' },
      { tag_name: 'v0.0.7-test' },
    ]),
    []
  );
});

test('every release in a list is scanned, not just the first', () => {
  // Guards the /g-flag lastIndex hazard: a stateful regex reused across calls
  // can skip a later match. Two identical offenders must both be reported.
  const findings = scanReleases([
    { tag_name: 'a', body: 'it gets sharper every day' },
    { tag_name: 'b', body: 'clean release' },
    { tag_name: 'c', body: 'it gets sharper every day' },
  ]);
  const tags = findings.map((f) => f.tag);
  assert.ok(tags.includes('a'), 'first offender missed');
  assert.ok(tags.includes('c'), 'later offender missed — regex lastIndex leaked between releases');
});

test('empty and malformed input does not throw', () => {
  assert.deepStrictEqual(scanReleases([]), []);
  assert.deepStrictEqual(scanReleases(undefined), []);
  assert.deepStrictEqual(scanReleases([{}]), []);
});
