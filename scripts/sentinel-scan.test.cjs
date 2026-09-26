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

// ── Merge-gate health (2026-09-24: "all clear" while main could not merge) ──
const { classifyMergeGateHealth } = require("./sentinel-scan.cjs");
const marker = (rel, date) => ({ rel, lineNo: 1, date });

test("merge gate: nothing known and nothing due is healthy (no findings)", () => {
  assert.deepStrictEqual(classifyMergeGateHealth({}), []);
});

test("merge gate: UNKNOWN CI state is never reported (gh offline must not page)", () => {
  const out = classifyMergeGateHealth({ scheduledValidate: null, nightlyAudit: null });
  assert.deepStrictEqual(out, []);
});

test("merge gate: green CI is not a finding", () => {
  const out = classifyMergeGateHealth({ scheduledValidate: "success", nightlyAudit: "success" });
  assert.deepStrictEqual(out, []);
});

test("merge gate: an expired unallowlisted marker is CRITICAL (it blocks every PR)", () => {
  const out = classifyMergeGateHealth({ blockingExpired: [marker("src-tauri/src/lib.rs", "2026-09-15")] });
  assert.strictEqual(out.length, 1);
  assert.strictEqual(out[0].severity, "critical");
  assert.match(out[0].detail, /lib\.rs:1 due 2026-09-15/);
});

test("merge gate: red scheduled Validate on main is CRITICAL; red nightly is a WARNING", () => {
  const out = classifyMergeGateHealth({ scheduledValidate: "failure", nightlyAudit: "failure" });
  assert.deepStrictEqual(out.map((f) => f.severity), ["critical", "warning"]);
});

test("merge gate: a cancelled scheduled run is not 'main is red'", () => {
  assert.deepStrictEqual(classifyMergeGateHealth({ scheduledValidate: "cancelled" }), []);
});

test("merge gate: due-soon deadlines warn and name the EARLIEST date", () => {
  const out = classifyMergeGateHealth({
    dueSoon: [marker("b.rs", "2026-10-05"), marker("a.rs", "2026-10-01")],
  });
  assert.strictEqual(out.length, 1);
  assert.strictEqual(out[0].severity, "warning");
  assert.match(out[0].message, /2 REMOVE BY deadline\(s\).*earliest 2026-10-01/);
});

// ── Stale PRs (2026-09-24: 36 Dependabot PRs up to 73 days old, 18 green) ──
test("merge gate: unknown PR state (gh unavailable) is never a finding", () => {
  assert.deepStrictEqual(classifyMergeGateHealth({ stalePrs: null }), []);
});

test("merge gate: no stale PRs is not a finding", () => {
  assert.deepStrictEqual(classifyMergeGateHealth({ stalePrs: [] }), []);
});

test("merge gate: stale PRs warn, naming the count and the OLDEST", () => {
  const out = classifyMergeGateHealth({
    stalePrs: [
      { number: 517, ageDays: 31, title: "jest-axe" },
      { number: 361, ageDays: 66, title: "lease client" },
    ],
  });
  assert.strictEqual(out.length, 1);
  assert.strictEqual(out[0].severity, "warning");
  assert.match(out[0].message, /2 open PR\(s\) older than 14 days \(oldest #361, 66d\)/);
  assert.match(out[0].detail.split("\n")[0], /^#361 66d/);
});

// ── cargo diagnostics parsing (the scan runs --message-format=short) ──
const { parseCargoDiagnostics } = require("./sentinel-scan.cjs");

test("cargo short format: errors AND locations are extracted (the old parser found no locations)", () => {
  const out = parseCargoDiagnostics(
    [
      "src/scoring/mod.rs:12:5: error[E0425]: cannot find value `x` in this scope",
      "src/db/mod.rs:40:9: error: mismatched types",
      "src/lib.rs:3:1: warning: unused import",
      "error: could not compile `fourda` (lib) due to 2 previous errors",
    ].join("\n")
  );
  assert.deepStrictEqual(out.errorFiles, ["src/scoring/mod.rs:12:5", "src/db/mod.rs:40:9"]);
  assert.deepStrictEqual(out.errors, [
    "error[E0425]: cannot find value `x` in this scope",
    "error: mismatched types",
  ]);
});

test("cargo short format on Windows: backslash paths are normalised so domain routing matches", () => {
  const out = parseCargoDiagnostics("src\\scoring\\gate.rs:7:1: error: boom\r\n");
  assert.deepStrictEqual(out.errorFiles, ["src/scoring/gate.rs:7:1"]);
});

test("cargo human format: the location comes from the following --> line", () => {
  const out = parseCargoDiagnostics(
    ["error[E0308]: mismatched types", "  --> src/db/mod.rs:40:9", "   |", "error: aborting due to 1 previous error"].join("\n")
  );
  assert.deepStrictEqual(out.errorFiles, ["src/db/mod.rs:40:9"]);
  assert.deepStrictEqual(out.errors, ["error[E0308]: mismatched types"]);
});

test("cargo summary is kept only when nothing specific parsed (never '0 errors' on a failure)", () => {
  const out = parseCargoDiagnostics("error: could not compile `fourda` (lib)\n");
  assert.deepStrictEqual(out.errorFiles, []);
  assert.deepStrictEqual(out.errors, ["error: could not compile `fourda` (lib)"]);
});
