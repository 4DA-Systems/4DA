// SPDX-License-Identifier: FSL-1.1-Apache-2.0
const { test } = require("node:test");
const assert = require("node:assert");
const { isFixtureManifest, splitAlerts, listOpenAlerts, COMMENT } = require("./dismiss-fixture-alerts.cjs");

test("fixture manifests are exactly the conformance corpus", () => {
  assert.strictEqual(isFixtureManifest("src-tauri/tests/conformance/cases/go-caddy-v2.6/go.mod"), true);
  assert.strictEqual(isFixtureManifest("site/pnpm-lock.yaml"), false);
  assert.strictEqual(isFixtureManifest("src-tauri/Cargo.lock"), false);
  assert.strictEqual(isFixtureManifest("x/src-tauri/tests/conformance/a"), false, "prefix, not substring");
  assert.strictEqual(isFixtureManifest(undefined), false);
});

test("splitAlerts never files a real manifest as a fixture", () => {
  const { fixture, real } = splitAlerts([
    { number: 1, manifest: "src-tauri/tests/conformance/cases/a/package-lock.json" },
    { number: 2, manifest: "site/pnpm-lock.yaml" },
  ]);
  assert.deepStrictEqual(fixture.map((a) => a.number), [1]);
  assert.deepStrictEqual(real.map((a) => a.number), [2]);
});

test("listOpenAlerts parses gh's tsv rows", () => {
  const rows = listOpenAlerts(() => "7\tsite/pnpm-lock.yaml\tmedium\tsprintf-js\n8\tsrc-tauri/tests/conformance/x/go.mod\thigh\tgolang.org/x/net\n");
  assert.deepStrictEqual(rows[0], { number: 7, manifest: "site/pnpm-lock.yaml", severity: "medium", pkg: "sprintf-js" });
  assert.strictEqual(rows.length, 2);
});

test("the dismissal comment fits GitHub's 280-character limit", () => {
  assert.ok(COMMENT.length <= 280, `${COMMENT.length} chars`);
});
