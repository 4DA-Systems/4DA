// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the vanity-metrics pre-commit gate (scripts/check-vanity-metrics.cjs),
// which enforces intelligence-doctrine rule 3 ("no vanity metrics").
//
// Run: node --test scripts/check-vanity-metrics.test.cjs   (or `pnpm run test:scripts`)
//
// The gate only catches the SYNTACTICALLY detectable part: the four named banned labels
// rendered AS A COUNTER (adjacent to a number or {{count}}). These tests pin that, prove
// it does NOT flag the same words in legitimate prose, and make the semantic blind spots
// explicit (coverage gauges, single-point sparklines, swapped wording) so the gate is
// never mistaken for full doctrine coverage.

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { isVanityCounter, scanText } = require('./check-vanity-metrics.cjs');

const collapse = (s) => s.replace(/\s+/g, ' ');
const flagged = (line) => isVanityCounter(collapse(line));

test('CATCHES: "Decisions tracked: 0"', () => {
  assert.ok(flagged('Decisions tracked: 0'));
});

test('CATCHES: "Items monitored = 12"', () => {
  assert.ok(flagged('Items monitored = 12'));
});

test('CATCHES: an i18n {{count}} interpolation before the label', () => {
  assert.ok(flagged('{{count}} items monitored'));
});

test('CATCHES: the label immediately before a {{count}} interpolation', () => {
  assert.ok(flagged('decisions tracked {{count}}'));
});

test('CATCHES: "Validated principles: {{n}}"', () => {
  assert.ok(flagged('Validated principles: {{n}}'));
});

test('CATCHES: case-insensitively (SOURCES PRODUCING: 5)', () => {
  assert.ok(flagged('SOURCES PRODUCING: 5'));
});

test('CATCHES: a label + count split across two adjacent lines (2-line window)', () => {
  const text = ['Items monitored:', '{{count}}'].join('\n');
  assert.equal(scanText(text).length, 1, 'label and count on adjacent lines must be caught');
});

test('CLEAN: the doctrine\'s own legit-prose example ("decisions tracked with evidence")', () => {
  assert.ok(!flagged('Every decision tracked with evidence and calibrated confidence.'));
});

test('CLEAN: "items monitored" / "sources producing" as descriptive prose (no number)', () => {
  assert.ok(!flagged('Items monitored by the system surface in your daily brief.'));
  assert.ok(!flagged('Sources producing relevant content rise to the top.'));
});

test('ESCAPE HATCH: a vanity-ok marker on the line above suppresses the violation', () => {
  const text = [
    '// vanity-ok: this count drives the "review N stale items" action',
    'Decisions tracked: 0',
  ].join('\n');
  assert.equal(scanText(text).length, 0, 'marker on the preceding line must suppress');
});

// --- KNOWN GAPS (asserted so the blind spots are explicit, not assumed-covered) ---

test('KNOWN GAP: swapped wording ("Principles validated: 0") evades the label list', () => {
  assert.ok(!flagged('Principles validated: 0'),
    'the gate matches the four exact labels — reworded counters are a PR-review catch');
});

test('KNOWN GAP: intervening JSX tags between label and {{count}} defeat the adjacency match', () => {
  // <span>Items monitored</span><span>{{count}}</span> — the </span><span> between the
  // label and the interpolation is not whitespace, so the 2-line window does not match.
  const text = ['<span>Items monitored</span>', '<span>{{count}}</span>'].join('\n');
  assert.equal(scanText(text).length, 0,
    'documents that tag-separated label/count is NOT caught — PR review must catch it');
});

test('KNOWN GAP: a semantically-vain metric with a different label is invisible', () => {
  // A coverage gauge whose denominator ~= numerator, or a sparkline from one data point,
  // is banned by doctrine rule 3 but not syntactically detectable. Documented, not caught.
  assert.ok(!flagged('Coverage: 100% (1 of 1)'),
    'semantic vanity (coverage gauges, single-point sparklines) is a PR-review responsibility');
});
