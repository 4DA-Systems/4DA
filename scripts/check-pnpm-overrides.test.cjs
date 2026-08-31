// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The gate is inert today (the pin is pnpm 9), so a green run proves nothing on
 * its own. These tests drive the pure core across the pin boundary — that is
 * the only place the guard has any value.
 */
const test = require('node:test');
const assert = require('node:assert');
const { evaluate, pnpmMajorFrom, PNPM_FIELD_DROPPED_IN_MAJOR } = require('./check-pnpm-overrides.cjs');

const REAL_USAGES = [
  { file: 'package.json', count: 15 },
  { file: 'site/package.json', count: 16 },
  { file: 'paddle-webhook/package.json', count: 22 },
  { file: 'mcp-4da-server/package.json', count: 16 },
];

test('pnpm major is parsed from the packageManager pin', () => {
  assert.strictEqual(pnpmMajorFrom('pnpm@9.15.0'), 9);
  assert.strictEqual(pnpmMajorFrom('pnpm@11.13.0'), 11);
  assert.strictEqual(pnpmMajorFrom('pnpm@10.0.0-beta.1'), 10);
  assert.strictEqual(pnpmMajorFrom(undefined), null);
  assert.strictEqual(pnpmMajorFrom('yarn@4.0.0'), null);
});

test('THE case: bumping the pin to 11 with overrides still in package.json FAILS', () => {
  const r = evaluate(PNPM_FIELD_DROPPED_IN_MAJOR, REAL_USAGES);
  assert.strictEqual(r.ok, false, 'a pnpm 11 pin must not pass while 69 overrides sit in the dead field');
  assert.match(r.message, /DOES NOT READ/);
  assert.match(r.message, /69 security override/);
  // The message has to name the files, or whoever hits this cannot act on it.
  assert.match(r.message, /paddle-webhook\/package\.json \(22 override\(s\)\)/);
  assert.match(r.message, /pnpm-workspace\.yaml/);
});

test('the current pin passes, and reports what is live', () => {
  const r = evaluate(9, REAL_USAGES);
  assert.strictEqual(r.ok, true);
  assert.match(r.message, /still reads/);
  assert.match(r.message, /69 override\(s\) across 4 manifest\(s\)/);
});

test('pnpm 11 is fine once the overrides have actually been moved', () => {
  // The gate must not become a permanent blocker on upgrading — it has to go
  // green the moment the migration is done.
  const r = evaluate(11, []);
  assert.strictEqual(r.ok, true);
  assert.match(r.message, /no manifest relies/);
});

test('pnpm 12+ is treated the same as 11, not just the exact version', () => {
  assert.strictEqual(evaluate(12, REAL_USAGES).ok, false);
  assert.strictEqual(evaluate(99, REAL_USAGES).ok, false);
});

test('an unreadable pin fails closed rather than passing silently', () => {
  const r = evaluate(null, REAL_USAGES);
  assert.strictEqual(r.ok, false);
  assert.match(r.message, /Could not read a pnpm major/);
});
