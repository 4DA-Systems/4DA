// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the LLM-gate-honesty pre-commit gate (scripts/check-llm-gate-honesty.cjs).
//
// Run: node --test scripts/check-llm-gate-honesty.test.cjs   (or `pnpm run test:scripts`)
//
// The gate is a regex tripwire, not a proof. These tests do two jobs:
//   1. Pin the shapes it MUST catch (the exact proxy-derived-state constructs from the
//      antibody) and the legitimate shapes it must NOT flag (so the gate can't silently
//      rot into either a no-op or a false-positive nuisance).
//   2. Make the gate's BLIND SPOTS explicit — the "KNOWN GAP" cases assert the current
//      (non-)behaviour so nobody mistakes this syntactic gate for semantic coverage.
//      Capability-claim correctness that the gate can't see is a PR-review responsibility.

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { matchProxyConstruct, scanText, collapse } = require('./check-llm-gate-honesty.cjs');

const isViolation = (line) => matchProxyConstruct(collapse(line)) !== null;

test('CATCHES: Rust key-first OR-shortcut (api_key.is_empty() || provider=="ollama")', () => {
  assert.ok(isViolation('let has = api_key.is_empty() || provider == "ollama";'));
});

test('CATCHES: Rust provider-first OR-shortcut (provider=="ollama" || !api_key.is_empty())', () => {
  assert.ok(isViolation('let has = provider == "ollama" || !api_key.is_empty();'));
});

test('CATCHES: the builtin variant of the OR-shortcut', () => {
  assert.ok(isViolation('provider == "builtin" || !api_key.is_empty()'));
});

test('CATCHES: qualified receiver (self.provider == "ollama" || !self.api_key.is_empty())', () => {
  assert.ok(isViolation('self.provider == "ollama" || !self.api_key.is_empty()'));
});

test('CATCHES: !matches!(provider, "ollama"|"builtin") && api_key.is_empty() (the missing-"none" form)', () => {
  assert.ok(isViolation('if !matches!(provider, "ollama" | "builtin") && api_key.is_empty() {'));
});

test('CATCHES: TS has_api_key || provider === "ollama" (either order)', () => {
  assert.ok(isViolation("const ok = has_api_key || provider === 'ollama';"));
  assert.ok(isViolation("const ok = provider === 'builtin' || llm.has_api_key;"));
});

test('CATCHES: a line-split expression via the 3-line scan window', () => {
  const text = [
    'let configured =',
    '    provider == "ollama"',
    '    || !api_key.is_empty();',
  ].join('\n');
  assert.equal(scanText(text).length, 1, 'rustfmt line-split must still be caught');
});

test('CLEAN: the canonical compute_has_llm call is not flagged', () => {
  assert.ok(!isViolation('let has = compute_has_llm(&provider, &api_key);'));
});

test('CLEAN: a legitimate provider allowlist (no key term) is not flagged', () => {
  assert.ok(!isViolation('if provider == "ollama" || provider == "anthropic" {'));
  assert.ok(!isViolation("if (provider === 'ollama' || provider === 'openai') {"));
});

test('CLEAN: an honest key-saved display (has_api_key alone, no provider OR) is not flagged', () => {
  assert.ok(!isViolation('placeholder={llm.has_api_key ? saved : enter}'));
});

test('ESCAPE HATCH: an llm-gate-ok marker on the line above suppresses the violation', () => {
  const text = [
    '// llm-gate-ok: this is the canonical helper definition, not a proxy',
    'provider == "ollama" || !api_key.is_empty()',
  ].join('\n');
  assert.equal(scanText(text).length, 0, 'marker on the preceding line must suppress');
});

// --- KNOWN GAPS (asserted so the blind spots are explicit, not assumed-covered) ---

test('KNOWN GAP: a proxy hidden behind an intermediate variable evades the gate', () => {
  // `let configured = !api_key.is_empty();` then `configured || provider == "ollama"`
  // — the key-presence test and the OR-shortcut are no longer in the same window.
  assert.ok(!isViolation('let ok = configured || provider == "ollama";'),
    'documents that variable-indirection is NOT caught — PR review must catch it');
});

test('KNOWN GAP: a non-is_empty() key check (api_key.len() > 0) evades the gate', () => {
  assert.ok(!isViolation('provider == "ollama" || api_key.len() > 0'),
    'documents that alternate key-presence spellings are NOT caught');
});

test('KNOWN GAP: a renamed TS flag (hasKey, not has_api_key) evades the gate', () => {
  assert.ok(!isViolation("const ok = hasKey || provider === 'ollama';"),
    'the TS patterns are anchored on the literal has_api_key field name');
});
