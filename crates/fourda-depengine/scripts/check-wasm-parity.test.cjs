// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The parity harness's rules, without a wasm build: a fake engine stands in
// for the module. node --test scripts/check-wasm-parity.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const { replay, truthProblem, answerableIds } = require('./check-wasm-parity.cjs');

const ok = (result) => ({ schema: 'depengine/0', ok: true, result });
const line = (kind, id, result, expect) => ({
  kind,
  id,
  request: { op: kind, id },
  response: ok(result),
  ...(expect === undefined ? {} : { expect }),
});
/** An engine that answers each request with the result `answers[id]`. */
const engine = (answers) => (request) => ok(answers[request.id]);

test('wasm equal to native and to the truth passes', () => {
  const lines = [line('version_order', 'vo-1', '<', '<'), line('fix_target', 'ft-1', { walk: null })];
  const { problems, counts } = replay(lines, engine({ 'vo-1': '<', 'ft-1': { walk: null } }));
  assert.deepEqual(problems, []);
  assert.deepEqual(counts, { version_order: 1, fix_target: 1 });
});

test('wasm differing from native fails, even where no truth is stated', () => {
  const lines = [line('fix_target', 'ft-1', { walk: { version: '1.2.4' } })];
  const { problems } = replay(lines, engine({ 'ft-1': { walk: { version: '1.2.5' } } }));
  assert.equal(problems.length, 1);
  assert.match(problems[0], /wasm != native/);
});

test('both builds agreeing on a wrong answer still fails against the vector', () => {
  const lines = [line('version_order', 'vo-1', '>', '<')];
  const { problems } = replay(lines, engine({ 'vo-1': '>' }));
  assert.equal(problems.length, 1);
  assert.match(problems[0], /disagree with the vector/);
});

test('a native error is a failure, not a pass', () => {
  const bad = { kind: 'range_check', id: 'rc-1', request: { id: 'rc-1' }, response: { ok: false, error: 'x' } };
  const { problems } = replay([bad], () => bad.response);
  assert.match(problems[0], /native engine did not answer/);
});

test('a vector the generator dropped, or too few lines of a kind, fails', () => {
  const lines = [line('osv_range', 'or-1', { affected: true, confirmed: true }, { affected: true, confirmed: true })];
  const answers = { 'or-1': { affected: true, confirmed: true } };
  const { problems } = replay(lines, engine(answers), { osv_range: ['or-1', 'or-2'] }, { match_inventory: 1 });
  assert.equal(problems.length, 2);
  assert.match(problems[0], /1 vectors never replayed \(or-2\)/);
  assert.match(problems[1], /match_inventory: 0 lines/);
});

test('range_check truth reads an absent smallest_safe_upgrade as null', () => {
  const expect = { smallest_safe_in_range: '7.5.11', smallest_safe_upgrade: null };
  const l = { kind: 'range_check', expect };
  assert.equal(truthProblem(l, ok({ smallest_safe_in_range: '7.5.11' })), null);
  assert.match(truthProblem(l, ok({ smallest_safe_in_range: '7.5.12' })), /smallest_safe_in_range/);
  assert.equal(truthProblem({ kind: 'fix_target' }, ok({})), null, 'no expect, no truth check');
});

test('the answered vector files are all found, held fix_target lines left out', () => {
  const dir = path.join(__dirname, '..', '..', '..', 'src-tauri', 'tests', 'conformance', 'vectors');
  const ids = answerableIds(dir);
  for (const kind of ['version_order', 'osv_range', 'fix_target', 'range_check']) {
    assert.ok(ids[kind].length > 0, `${kind} has vectors`);
  }
  assert.ok(ids.range_check.includes('rc-028'));
});
