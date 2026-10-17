#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Proves the fourda-depengine wasm module is pure: it must declare ZERO
// imports (no WASI, no host functions — so it cannot reach a file, socket or
// clock), and it must actually run. Reports its size, raw and gzipped. Then
// replays one real conformance case (tests/fixtures/match_inventory_case.json)
// and requires the wasm answer to equal the native engine's.
//
// Usage: node check-wasm.cjs <path/to/fourda_depengine.wasm> [fixture.json]
// Exit 1 on any import, a missing export, a wrong answer, or wasm != native.

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const { isDeepStrictEqual } = require('node:util');

const file = process.argv[2];
if (!file) {
  console.error('usage: node check-wasm.cjs <fourda_depengine.wasm>');
  process.exit(2);
}

const bytes = fs.readFileSync(file);
const mod = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(mod);
const exportNames = WebAssembly.Module.exports(mod).map((e) => e.name);
const gz = zlib.gzipSync(bytes, { level: 9 }).length;
const kb = (n) => `${(n / 1024).toFixed(1)} KB`;

console.log(`wasm: ${file}`);
console.log(`size: ${bytes.length} bytes (${kb(bytes.length)}), gzip ${gz} bytes (${kb(gz)})`);
console.log(`imports: ${imports.length}`);
console.log(`exports: ${exportNames.join(', ')}`);

let failed = false;
if (imports.length > 0) {
  for (const i of imports) console.error(`::error::wasm import ${i.module}.${i.name} (${i.kind})`);
  failed = true;
}
for (const name of ['memory', 'depengine_alloc', 'depengine_call', 'depengine_free']) {
  if (!exportNames.includes(name)) {
    console.error(`::error::wasm export ${name} missing`);
    failed = true;
  }
}
if (failed) process.exit(1);

// Run it: zero imports means it instantiates with an empty import object.
const { exports: x } = new WebAssembly.Instance(mod, {});
function call(request) {
  const input = Buffer.from(JSON.stringify(request), 'utf8');
  const ptr = x.depengine_alloc(input.length);
  new Uint8Array(x.memory.buffer, ptr, input.length).set(input);
  const packed = BigInt.asUintN(64, x.depengine_call(ptr, input.length));
  const outPtr = Number(packed >> 32n);
  const outLen = Number(packed & 0xffffffffn);
  const text = Buffer.from(new Uint8Array(x.memory.buffer, outPtr, outLen)).toString('utf8');
  x.depengine_free(outPtr, outLen);
  return JSON.parse(text);
}

const ranges = [{ type: 'SEMVER', events: [{ introduced: '0' }, { fixed: '1.2.4' }] }];
const checks = [
  [{ op: 'version_cmp', eco: 'PyPI', a: '1.0.0-1', b: '1.0.0' }, '>'],
  [{ op: 'version_cmp', eco: 'Go', a: 'v0.0.0-20190308221718-c2843e01d9a2', b: '0.0.0' }, '<'],
  [{ op: 'affected', eco: 'npm', version: '1.2.3', ranges }, { affected: true, confirmed: true }],
  [
    { op: 'fix_target', eco: 'npm', installed: '1.2.3', ranges: [ranges] },
    { line_fixes: ['1.2.4'], walk: { version: '1.2.4', clears_all: true }, withheld: [] },
  ],
  // The registry withdrew the fix (yanked / deprecated): the next release it offers.
  [
    {
      op: 'fix_target',
      eco: 'npm',
      installed: '1.2.3',
      ranges: [ranges],
      registry: { published: ['1.2.4', '1.2.5'], excluded: ['1.2.4'] },
    },
    { line_fixes: ['1.2.5'], walk: { version: '1.2.5', clears_all: true }, withheld: ['1.2.4'] },
  ],
];
for (const [request, want] of checks) {
  const got = call(request);
  const ok = got.ok === true && isDeepStrictEqual(got.result, want);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${request.op}: ${JSON.stringify(got.result ?? got.error)}`);
  if (!ok) failed = true;
}

// One real corpus case through the wasm module, compared with the native
// engine's answer for the same request. The fixture is written and checked by
// the app's conformance harness (lockfile readers -> inventory, pinned OSV
// records, native `match_inventory`) and replayed natively by
// tests/match_fixture.rs, so equality here means wasm == native on real input.
const fixturePath =
  process.argv[3] || path.join(__dirname, '..', 'tests', 'fixtures', 'match_inventory_case.json');
const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
const started = process.hrtime.bigint();
const got = call(fixture.request);
const ms = Number(process.hrtime.bigint() - started) / 1e6;
const findings = got.result?.findings?.length ?? 0;
const same = isDeepStrictEqual(got, fixture.response);
console.log(
  `${same ? 'ok  ' : 'FAIL'} match_inventory on corpus case ${fixture.case}: ` +
    `${fixture.request.inventory.length} packages, ${fixture.request.osv.length} OSV records, ` +
    `${findings} findings, ${ms.toFixed(1)} ms; wasm ${same ? '==' : '!='} native`,
);
if (!same) {
  const want = fixture.response.result?.findings ?? [];
  const have = got.result?.findings ?? [];
  for (let i = 0; i < Math.max(want.length, have.length); i++) {
    if (!isDeepStrictEqual(want[i], have[i])) {
      console.error(`::error::first difference at finding ${i}`);
      console.error(`native: ${JSON.stringify(want[i])}`);
      console.error(`wasm:   ${JSON.stringify(have[i])}`);
      break;
    }
  }
  if (!got.ok) console.error(`::error::wasm error: ${got.error}`);
  failed = true;
}
process.exit(failed ? 1 : 0);
