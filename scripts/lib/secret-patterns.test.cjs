// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Regression test for the credential-shape list.
 *
 * The bug this pins: `/sk-[a-zA-Z0-9]{32,}/` stops at the first hyphen, so the
 * two key formats the app actually accepts today — `sk-ant-api03-...` and
 * `sk-proj-...` — were invisible to the public-readiness audit while it
 * reported "no findings". A detector fix without a negative test is how that
 * happens twice, so every pattern here is asserted in BOTH directions.
 */
const test = require('node:test');
const assert = require('node:assert');
const { SECRET_PATTERNS } = require('./secret-patterns.cjs');

const hits = (s) => SECRET_PATTERNS.filter((p) => p.regex.test(s)).map((p) => p.label);

// Synthetic, structurally valid, deliberately not real credentials.
const MUST_CATCH = {
  'Anthropic current': 'sk-' + 'ant-api03-' + 'A1b2C3d4E5f6G7h8'.repeat(3) + '-AAAAAA',
  'OpenAI project': 'sk-proj-' + 'Zz9Yy8Xx7Ww6Vv5U'.repeat(2) + 'abcd',
  'OpenAI legacy': 'sk-' + 'a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6',
  'Groq': 'gsk_' + 'aB3dE6gH9jK2mN5pQ8sT1vW4xY7zA0bC',
  'Resend': 're_' + 'aB3dE6gH' + '_' + '9jK2mN5pQ8sT1vW4xY7zA0bC',
  'Slack bot': 'xoxb-' + '111111111111-2222222222222-abcdefghijklmnop',
  'Slack user': 'xoxp-' + '111111111111-2222222222222-abcdefghijklmnop',
  'GitHub classic': 'ghp_' + 'a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8',
  'GitHub server-to-server': 'ghs_' + 'a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8',
  'AWS': 'AKIA' + 'IOSFODNN7EXAMPLE',
  'Google': 'AIza' + 'SyD-aBcDeFgHiJkLmNoPqRsTuVwXyZ01234',
  'Stripe live': 'sk_live_' + 'a1B2c3D4e5F6g7H8i9J0k1L2',
  'Stripe restricted': 'rk_live_' + 'a1B2c3D4e5F6g7H8i9J0k1L2',
  'DeepL': '12345678-90ab-cdef-1234-567890abcdef:fx',
  'JWT': 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9' + '.' + 'eyJzdWIiOiIxMjM0NTY3ODkwIn0' + '.' + 'dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXkA',
  'Private key block': '-----' + 'BEGIN OPENSSH PRIVATE KEY' + '-----',
};

// Strings that must NOT trip the scanner. A gate that fires on ordinary source
// gets switched off, which is the same outcome as having no gate.
const MUST_NOT_CATCH = [
  'sk-short',                                  // too short to be a key
  'const sk = 1;',                             // bare identifier
  'https://api.anthropic.com/v1/messages',     // provider URL
  'Set your API key in Settings -> AI Provider',
  're_export',                                 // `re_` prefix, too short
  'gsk_',                                      // prefix alone
  'AKIA',                                      // prefix alone
  'x-scheme-handler/fourda',                   // desktop entry line
  'sk_test_' + 'a1B2c3D4e5F6g7H8i9J0k1L2',     // Stripe TEST key is not a secret leak
  '12345678-90ab-cdef-1234-567890abcdef',      // plain UUID, no :fx suffix
];

test('every current key format is detected', () => {
  for (const [name, sample] of Object.entries(MUST_CATCH)) {
    assert.ok(hits(sample).length > 0, `${name} was NOT detected: ${sample.slice(0, 24)}...`);
  }
});

test('the two formats the old pattern missed are the ones that matter', () => {
  // Pinned separately: this is the exact regression, and it should fail loudly
  // rather than being absorbed into the loop above.
  const legacyPattern = /sk-[a-zA-Z0-9]{32,}/;
  for (const key of [MUST_CATCH['Anthropic current'], MUST_CATCH['OpenAI project']]) {
    assert.ok(!legacyPattern.test(key), 'sample no longer reproduces the old blind spot');
    assert.ok(hits(key).length > 0, 'current pattern must catch it');
  }
});

test('ordinary source text does not trip the scanner', () => {
  for (const sample of MUST_NOT_CATCH) {
    assert.deepStrictEqual(hits(sample), [], `false positive on: ${sample}`);
  }
});

const { looksLikePlaceholder } = require('./secret-patterns.cjs');

test('fixtures are recognised as placeholders', () => {
  // Every one of these is a real string from this repo's own test suite. They
  // are why the filter exists: broadening the patterns made all of them fire.
  for (const s of [
    'sk-' + 'ant-api03-TESTKEYDONOTUSE-1234567890abcdef',
    'sk-' + 'ant-realkey-1234567890',
    'sk-' + 'ant-cannot-reach',
    'sk-' + 'ant-rate-limited',
    'sk-' + 'ant-1234567890123456789',
    'AKIA' + 'IOSFODNN7EXAMPLE',
  ]) {
    assert.ok(looksLikePlaceholder(s), `should be treated as a fixture: ${s}`);
  }
});

test('a realistic key is NOT excused as a placeholder', () => {
  // The load-bearing direction. If this ever passes, the filter has swallowed
  // the gate and the audit's "no findings" means nothing.
  for (const s of [
    'sk-' + 'ant-api03-' + 'A1b2C3d4E5f6G7h8'.repeat(3) + '-AAAAAA',
    'sk-proj-' + 'Zz9Yy8Xx7Ww6Vv5U'.repeat(2) + 'abcd',
    'gsk_' + 'aB3dE6gH9jK2mN5pQ8sT1vW4xY7zA0bC',
    'AIza' + 'SyD-aBcDeFgHiJkLmNoPqRsTuVwXyZ01234',
  ]) {
    assert.ok(!looksLikePlaceholder(s), `must NOT be excused: ${s.slice(0, 24)}...`);
  }
});
