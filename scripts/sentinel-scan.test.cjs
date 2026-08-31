// Negative tests for sentinel-scan's compile-outcome classification.
//
// Origin (2026-08-31): a cold `tsc` exceeded the scan's 60s timeout, produced
// zero "error TS" lines, and the sentinel reported "TypeScript compilation
// failed (0 errors)" as CRITICAL — deploying an expert against a compiler
// that had in fact never finished. The detector matched text ("!ok"), not
// meaning ("the compiler reported errors"). Per the gate-precision doctrine,
// every detector fix ships with the negative case that previously slipped.
const { test } = require("node:test");
const assert = require("node:assert");
const { classifyTscResult } = require("./sentinel-scan.cjs");

test("clean run is ok", () => {
  const o = classifyTscResult({ ok: true, output: "" });
  assert.strictEqual(o.severity, "ok");
});

test("real TS errors are critical and counted", () => {
  const o = classifyTscResult({
    ok: false,
    code: 2,
    output: "src/a.tsx(10,5): error TS2322: Type 'x' is not assignable.\nsrc/b.ts(3,1): error TS2551: nope.",
  });
  assert.strictEqual(o.severity, "critical");
  assert.match(o.message, /failed \(2 errors\)/);
});

test("REGRESSION: timeout with zero TS errors is inconclusive, never critical", () => {
  // The exact live shape: killed by timeout, no error lines.
  const o = classifyTscResult({ ok: false, code: null, timedOut: true, output: "" });
  assert.strictEqual(o.severity, "warning");
  assert.match(o.message, /inconclusive/);
  assert.doesNotMatch(o.message, /compilation failed/);
});

test("REGRESSION: non-zero exit with zero TS errors is inconclusive, never '0 errors' critical", () => {
  // npx/tooling noise: exit 1, output that contains no `error TS` line.
  const o = classifyTscResult({ ok: false, code: 1, output: "npm warn deprecated something\n" });
  assert.strictEqual(o.severity, "warning");
  assert.match(o.message, /inconclusive/);
  assert.doesNotMatch(o.message, /\(0 errors\)/);
});

test("spawn failure (ENOENT) is inconclusive and names the cause", () => {
  const o = classifyTscResult({ ok: false, code: undefined, errCode: "ENOENT", output: "" });
  assert.strictEqual(o.severity, "warning");
  assert.match(o.message, /inconclusive/);
});
