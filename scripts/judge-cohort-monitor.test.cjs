// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the judge cohort monitor (scripts/judge-cohort-monitor.cjs).
//
// Run: node --test scripts/judge-cohort-monitor.test.cjs   (or `pnpm run test:scripts`)
//
// THE CENTRAL CASE: the anomaly checks must run on the cohort that is
// actually being written (newest judged_at in the last 24h), not on the
// version the Rust source declares. For three days in September 2026 the
// source said v6 while every live row was v5; the monitor filed v5 as
// "retired", skipped every check on it, and exited 0. Now that situation is
// a finding in its own right (MERGED != RUNNING), and v5's spike still trips.
//
// No SQLite here. `run()` is pure over a `CohortReader`, and these tests hand
// it a reader over plain arrays — CI's "Repo guards" job installs without
// building native modules, so `better-sqlite3` must never be loaded by a
// test. The SQL behind the production reader is exercised by running the
// script itself against the live database (`pnpm run monitor:judge`).

const { test } = require('node:test');
const assert = require('node:assert/strict');

const {
  parseShippedConstants,
  liveCohortVersions,
  deployDrift,
  run,
  STALE_BINARY_HOURS,
  DRAIN_COHORT,
} = require('./judge-cohort-monitor.cjs');

const HOUR = 3_600_000;
const NOW = Date.parse('2026-09-04T12:00:00Z');

const RUST_SRC = `
pub(crate) const PROMPT_VERSION: &str = "v6";
pub(crate) const DEMOTION_RELEVANCE_BELOW: f64 = 0.35;
pub(crate) const DEMOTION_CONFIDENCE_MIN: f64 = 0.7;
`;

// ---------------------------------------------------------------------------
// Fixture reader — the same six named queries the SQL reader answers, over
// an in-memory array of judgment rows.
// ---------------------------------------------------------------------------

const stamp = (ms) => new Date(ms).toISOString().replace('T', ' ').slice(0, 19);

/**
 * @param {Array<{prompt_version:string, model:string, relevance_score:number, confidence:number, judged_at:number}>} rows
 * @param {{reach?: object, demotions?: object[]}} [extra]
 */
function fixtureReader(rows, { reach = { at_gate: 0, at_probe: 0, curated_judged: 0 }, demotions = [] } = {}) {
  const round = (x, dp) => Math.round(x * 10 ** dp) / 10 ** dp;
  return {
    cohorts(relevanceBelow) {
      const groups = new Map();
      for (const r of rows) {
        const key = `${r.prompt_version} ${r.model}`;
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(r);
      }
      return [...groups.values()]
        .map((g) => {
          const n = g.length;
          const first = Math.min(...g.map((r) => r.judged_at));
          return {
            prompt_version: g[0].prompt_version,
            model: g[0].model,
            n,
            avg_rel: round(g.reduce((a, r) => a + r.relevance_score, 0) / n, 3),
            avg_conf: round(g.reduce((a, r) => a + r.confidence, 0) / n, 3),
            omitted: g.filter((r) => r.confidence === 0).length,
            rejects: g.filter((r) => r.relevance_score < relevanceBelow).length,
            first: stamp(first),
            last: stamp(Math.max(...g.map((r) => r.judged_at))),
            _first: first,
          };
        })
        .sort((a, b) => a._first - b._first);
    },
    liveVersion() {
      const cutoff = NOW - 24 * HOUR;
      const recent = rows
        .filter((r) => r.judged_at >= cutoff && r.prompt_version !== DRAIN_COHORT)
        .sort((a, b) => b.judged_at - a.judged_at);
      return recent[0]?.prompt_version ?? null;
    },
    topConfidence(version, model) {
      const counts = new Map();
      for (const r of rows) {
        if (r.prompt_version !== version || r.model !== model) continue;
        const v = round(r.confidence, 2);
        counts.set(v, (counts.get(v) ?? 0) + 1);
      }
      let best;
      for (const [v, n] of counts) if (!best || n > best.n) best = { v, n };
      return best;
    },
    rowsAt: (version) => rows.filter((r) => r.prompt_version === version).length,
    gateReach: () => reach,
    demotions: () => demotions,
  };
}

/**
 * Append `n` judgments for a cohort. `confidence` / `relevance` may be
 * functions of the row index so a test can shape the distribution.
 */
function cohort(rows, version, { n, ageHours = 1, confidence = () => 0.8, relevance = () => 0.6 }) {
  for (let i = 0; i < n; i++) {
    rows.push({
      prompt_version: version,
      model: 'test-model',
      relevance_score: relevance(i),
      confidence: confidence(i),
      judged_at: NOW - ageHours * HOUR,
    });
  }
  return rows;
}

/** A healthy, discriminating cohort shape: spread confidence, mixed verdicts. */
const healthy = { confidence: (i) => 0.55 + (i % 10) / 25, relevance: (i) => (i % 3 ? 0.7 : 0.2) };

const quiet = () => {};
const constants = parseShippedConstants(RUST_SRC);
const go = (rows, binary, extra) =>
  run({ reader: fixtureReader(rows, extra), constants, binary, now: NOW, log: quiet });

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

test('reads the shipped constants from Rust source', () => {
  assert.deepEqual(constants, { promptVersion: 'v6', relevanceBelow: 0.35, confidenceMin: 0.7 });
});

test('refuses to run on source it cannot read a constant from', () => {
  assert.throws(() => parseShippedConstants('// nothing here'), /PROMPT_VERSION/);
});

test('loading this test never loads the native SQLite module', () => {
  // The exact CI failure: `require('better-sqlite3')` at module top made the
  // test file unloadable where native modules are not built.
  const loaded = Object.keys(require.cache).some((k) => /[\\/]better-sqlite3[\\/]/.test(k));
  assert.equal(loaded, false, 'better-sqlite3 must only load inside the real database path');
});

// ---------------------------------------------------------------------------
// Live cohort = newest judged_at, not the source constant
// ---------------------------------------------------------------------------

test('the live cohort is whatever was judged most recently in 24h, plus drain_v1', () => {
  const rows = [];
  cohort(rows, 'v5', { n: 30, ageHours: 2 });
  cohort(rows, 'v4', { n: 30, ageHours: 72 });
  cohort(rows, DRAIN_COHORT, { n: 5, ageHours: 0.5 }); // drain never masks the real cohort
  const live = liveCohortVersions(fixtureReader(rows));
  assert.deepEqual([...live].sort(), [DRAIN_COHORT, 'v5']);
});

test('with nothing judged in 24h only the drain cohort is live', () => {
  const rows = cohort([], 'v5', { n: 30, ageHours: 72 });
  assert.deepEqual([...liveCohortVersions(fixtureReader(rows))], [DRAIN_COHORT]);
});

// ---------------------------------------------------------------------------
// MERGED != RUNNING
// ---------------------------------------------------------------------------

test('THE CASE: source v6, live rows v5, stale binary -> MERGED != RUNNING and v5 is still checked', () => {
  // v5 is live AND broken: 93% of its confidence sits on exactly 0.5.
  const rows = cohort([], 'v5', { n: 100, ageHours: 1, confidence: (i) => (i < 93 ? 0.5 : 0.9) });
  const binary = { path: 'fourda.exe', mtimeMs: NOW - 3 * 24 * HOUR };

  const r = go(rows, binary);

  assert.equal(r.exitCode, 1);
  const merged = r.findings.find((f) => f.startsWith('MERGED != RUNNING'));
  assert.ok(merged, `expected a MERGED != RUNNING finding, got: ${r.findings.join(' | ')}`);
  assert.match(merged, /source says v6/);
  assert.match(merged, /live rows are v5/);
  assert.match(merged, /binary built \d{4}-\d{2}-\d{2}T/);
  // The old classifier filed v5 as retired and skipped this. Not any more.
  assert.ok(
    r.findings.some((f) => f.startsWith('v5:') && /single value 0.5/.test(f)),
    `v5 spike must be a finding on the LIVE cohort, got: ${r.findings.join(' | ')}`,
  );
});

test('a healthy live cohort under a stale binary still fails on the deploy gap alone', () => {
  const rows = cohort([], 'v5', { n: 60, ageHours: 1, ...healthy });
  const binary = { path: 'fourda.exe', mtimeMs: NOW - (STALE_BINARY_HOURS + 1) * HOUR };
  const r = go(rows, binary);
  assert.equal(r.exitCode, 1);
  assert.deepEqual(r.findings.filter((f) => !f.startsWith('MERGED != RUNNING')), []);
  assert.equal(r.drift.status, 'merged_not_running');
});

test('a binary built within the stale window is warming, not a finding', () => {
  const rows = cohort([], 'v5', { n: 60, ageHours: 1, ...healthy });
  const binary = { path: 'fourda.exe', mtimeMs: NOW - 10 * 60_000 };
  const r = go(rows, binary);
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.equal(r.drift.status, 'warming');
});

test('rows at the source version mean in sync, whatever the binary age', () => {
  const rows = cohort([], 'v6', { n: 60, ageHours: 1, ...healthy });
  const binary = { path: 'fourda.exe', mtimeMs: NOW - 30 * 24 * HOUR };
  const r = go(rows, binary);
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.equal(r.drift.status, 'in_sync');
});

test('no binary at all cannot be running the source version', () => {
  const d = deployDrift({ promptVersion: 'v6', sourceRows: 0, liveVersion: 'v5', binary: null, now: NOW });
  assert.equal(d.status, 'merged_not_running');
  assert.match(d.message, /no built binary found/);
});

// ---------------------------------------------------------------------------
// The existing checks still fire on the live cohort
// ---------------------------------------------------------------------------

test('a one-sided live judge is still a finding', () => {
  const rows = cohort([], 'v6', { n: 50, ageHours: 1, confidence: (i) => 0.5 + (i % 10) / 20, relevance: () => 0.9 });
  const r = go(rows, { path: 'x', mtimeMs: NOW });
  assert.equal(r.exitCode, 1);
  assert.ok(r.findings.some((f) => /reject rate 0%/.test(f)), r.findings.join(' | '));
});

test('an omitted-confidence share of 10% on the live cohort is a finding', () => {
  const rows = cohort([], 'v6', { n: 50, ageHours: 1, confidence: (i) => (i < 6 ? 0 : 0.55 + (i % 10) / 25), relevance: (i) => (i % 3 ? 0.7 : 0.2) });
  const r = go(rows, { path: 'x', mtimeMs: NOW });
  assert.ok(r.findings.some((f) => /confidence absent on 12%/.test(f)), r.findings.join(' | '));
});

test('a retired cohort with a spike is displayed but never a finding', () => {
  const rows = [];
  cohort(rows, 'v3', { n: 100, ageHours: 30 * 24, confidence: () => 0.5 });
  cohort(rows, 'v6', { n: 60, ageHours: 1, ...healthy });
  const lines = [];
  const r = run({ reader: fixtureReader(rows), constants, binary: { path: 'x', mtimeMs: NOW }, now: NOW, log: (l) => lines.push(l) });
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.ok(lines.some((l) => /v3.*spike \(retired cohort\)/.test(l)));
});

test('a gate that matches nothing while a looser one would is a finding', () => {
  const rows = cohort([], 'v6', { n: 60, ageHours: 1, ...healthy });
  const r = go(rows, { path: 'x', mtimeMs: NOW }, { reach: { at_gate: 0, at_probe: 15, curated_judged: 55 } });
  assert.equal(r.exitCode, 1);
  assert.ok(r.findings.some((f) => /15 curated item\(s\) qualify one notch looser/.test(f)), r.findings.join(' | '));
});
