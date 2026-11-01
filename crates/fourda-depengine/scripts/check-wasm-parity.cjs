#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// wasm == native over the shared rule vectors and the conformance corpus
// (single-engine C0.6). The native half is
// `cargo run --locked --example wasm_parity_requests -- <native.jsonl>`: one
// line per `depengine/0` request, with the native engine's response and,
// where the vector states one, its truth (`expect`). This script sends every
// request through the wasm module and requires:
//   1. the wasm response to equal the native response (parsed JSON, deep);
//   2. where `expect` is given, the answer to agree with it (the vector's
//      own truth, so a shared mistake in both builds still fails);
//   3. every vector the JSON API can answer to be present (read from the
//      vector files themselves), so a request the generator dropped fails
//      rather than shrinking the run.
//
// Node builtins only. Usage:
//   node check-wasm-parity.cjs <fourda_depengine.wasm> <native.jsonl>
// Exit 1 on any difference, a broken truth, a missing vector, or a module
// that imports anything.

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { isDeepStrictEqual } = require('node:util');

const VECTORS_DIR = path.join(__dirname, '..', '..', '..', 'src-tauri', 'tests', 'conformance', 'vectors');
const CASES_DIR = path.join(__dirname, '..', '..', '..', 'src-tauri', 'tests', 'conformance', 'cases');

/** Vector files the JSON API answers, the line kind each becomes, and which lines it skips. */
const ANSWERED = {
  version_order: { file: 'version_order.jsonl', keep: () => true },
  osv_range: { file: 'osv_range.jsonl', keep: () => true },
  // `held` lines run the app's registry pipeline, not one op.
  fix_target: { file: 'fix_target.jsonl', keep: (v) => v.held === undefined },
  range_check: { file: 'range_check.jsonl', keep: () => true },
};

/** Instantiate the module (it must import nothing) and return its JSON call. */
function loadEngine(wasmPath) {
  const mod = new WebAssembly.Module(fs.readFileSync(wasmPath));
  const imports = WebAssembly.Module.imports(mod);
  if (imports.length > 0) {
    throw new Error(`the module imports ${imports.map((i) => `${i.module}.${i.name}`).join(', ')}`);
  }
  const { exports: x } = new WebAssembly.Instance(mod, {});
  return function call(request) {
    const input = Buffer.from(JSON.stringify(request), 'utf8');
    const ptr = x.depengine_alloc(input.length);
    new Uint8Array(x.memory.buffer, ptr, input.length).set(input);
    const packed = BigInt.asUintN(64, x.depengine_call(ptr, input.length));
    const outPtr = Number(packed >> 32n);
    const outLen = Number(packed & 0xffffffffn);
    const text = Buffer.from(new Uint8Array(x.memory.buffer, outPtr, outLen)).toString('utf8');
    x.depengine_free(outPtr, outLen);
    return JSON.parse(text);
  };
}

/** Whether an ok response's result agrees with the line's stated truth. Null when the line states none. */
function truthProblem(line, response) {
  if (line.expect === undefined) return null;
  if (!response || response.ok !== true) return `not ok: ${response && response.error}`;
  const r = response.result;
  switch (line.kind) {
    case 'version_order':
    case 'osv_range':
      return isDeepStrictEqual(r, line.expect) ? null : `got ${JSON.stringify(r)}, want ${JSON.stringify(line.expect)}`;
    case 'range_check': {
      // `smallest_safe_upgrade` is absent without `from`; the vectors write null.
      const wrong = Object.keys(line.expect).filter((k) => !isDeepStrictEqual(r[k] ?? null, line.expect[k]));
      return wrong.length === 0
        ? null
        : wrong.map((k) => `${k}: got ${JSON.stringify(r[k] ?? null)}, want ${JSON.stringify(line.expect[k])}`).join('; ');
    }
    default:
      return `no truth rule for kind ${line.kind}`;
  }
}

/** The ids every answered vector file holds, by kind. */
function answerableIds(vectorsDir) {
  const out = {};
  for (const [kind, { file, keep }] of Object.entries(ANSWERED)) {
    const text = fs.readFileSync(path.join(vectorsDir, file), 'utf8');
    out[kind] = text
      .split('\n')
      .filter((l) => l.trim())
      .map((l) => JSON.parse(l))
      .filter(keep)
      .map((v) => v.id);
  }
  return out;
}

/**
 * Replay `lines` through `call`. `required` maps a kind to the ids that must
 * be present (`answerableIds`); `minimum` maps a kind to the fewest lines it
 * may have. Returns `{ counts, problems }`.
 */
function replay(lines, call, required = {}, minimum = {}) {
  const counts = {};
  const problems = [];
  const seen = {};
  for (const line of lines) {
    const tag = `${line.kind} ${line.id}`;
    counts[line.kind] = (counts[line.kind] || 0) + 1;
    (seen[line.kind] ||= new Set()).add(line.id);
    if (!line.response || line.response.ok !== true) {
      problems.push(`${tag}: the native engine did not answer (${line.response && line.response.error})`);
      continue;
    }
    let got;
    try {
      got = call(line.request);
    } catch (e) {
      problems.push(`${tag}: wasm threw ${e && e.message}`);
      continue;
    }
    if (!isDeepStrictEqual(got, line.response)) {
      problems.push(`${tag}: wasm != native\n  native: ${clip(line.response)}\n  wasm:   ${clip(got)}`);
      continue;
    }
    const truth = truthProblem(line, got);
    if (truth) problems.push(`${tag}: both builds disagree with the vector: ${truth}`);
  }
  for (const [kind, ids] of Object.entries(required)) {
    const missing = ids.filter((id) => !(seen[kind] && seen[kind].has(id)));
    if (missing.length > 0) problems.push(`${kind}: ${missing.length} vectors never replayed (${missing.slice(0, 5).join(', ')})`);
  }
  for (const [kind, n] of Object.entries(minimum)) {
    if ((counts[kind] || 0) < n) problems.push(`${kind}: ${counts[kind] || 0} lines, want at least ${n}`);
  }
  return { counts, problems };
}

function clip(value) {
  const text = JSON.stringify(value);
  return text.length > 600 ? `${text.slice(0, 600)}...` : text;
}

function readLines(file) {
  return fs
    .readFileSync(file, 'utf8')
    .split('\n')
    .filter((l) => l.trim())
    .map((l) => JSON.parse(l));
}

function main(argv) {
  const [wasmPath, nativePath] = argv;
  if (!wasmPath || !nativePath) {
    console.error('usage: node check-wasm-parity.cjs <fourda_depengine.wasm> <native.jsonl>');
    return 2;
  }
  const call = loadEngine(wasmPath);
  const lines = readLines(nativePath);
  const cases = fs.readdirSync(CASES_DIR, { withFileTypes: true }).filter((d) => d.isDirectory()).length;
  const started = process.hrtime.bigint();
  const { counts, problems } = replay(lines, call, answerableIds(VECTORS_DIR), {
    match_inventory: cases,
    read_lockfiles: cases,
  });
  const ms = Number(process.hrtime.bigint() - started) / 1e6;
  for (const [kind, n] of Object.entries(counts)) console.log(`${kind}: ${n} requests`);
  const ok = problems.length === 0;
  console.log(`${lines.length} requests in ${ms.toFixed(0)} ms; wasm ${ok ? '==' : '!='} native`);
  for (const p of problems.slice(0, 20)) console.error(`::error::${p}`);
  if (problems.length > 20) console.error(`... and ${problems.length - 20} more`);
  return ok ? 0 : 1;
}

if (require.main === module) {
  process.exit(main(process.argv.slice(2)));
}

module.exports = { loadEngine, truthProblem, answerableIds, replay, ANSWERED };
