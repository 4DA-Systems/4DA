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
// Every test builds a throwaway SQLite database in os.tmpdir().

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const Database = require('better-sqlite3');

const {
  parseShippedConstants,
  liveCohortVersions,
  deployDrift,
  run,
  STALE_BINARY_HOURS,
  DRAIN_COHORT,
} = require('./judge-cohort-monitor.cjs');

const HOUR = 3_600_000;

const RUST_SRC = `
pub(crate) const PROMPT_VERSION: &str = "v6";
pub(crate) const DEMOTION_RELEVANCE_BELOW: f64 = 0.35;
pub(crate) const DEMOTION_CONFIDENCE_MIN: f64 = 0.7;
`;

// ---------------------------------------------------------------------------
// Fixture DB
// ---------------------------------------------------------------------------

function fixtureDb() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), '4da-judge-monitor-'));
  const db = new Database(path.join(dir, 'fixture.db'));
  db.exec(`
    CREATE TABLE source_items (
      id INTEGER PRIMARY KEY,
      feed_relevant INTEGER,
      feed_verdict_source TEXT,
      feed_verdict_reason TEXT,
      feed_verdict_at TEXT
    );
    CREATE TABLE llm_judgments (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      source_item_id INTEGER,
      relevance_score REAL,
      explanation TEXT,
      actions TEXT,
      confidence REAL,
      model TEXT,
      prompt_version TEXT,
      judged_at TEXT
    );
  `);
  return db;
}

/**
 * Insert `n` judgments for a cohort. `confidence` may be a function of the
 * row index so a test can shape the distribution.
 */
function cohort(db, version, { n, age = '-1 hour', confidence = () => 0.8, relevance = () => 0.6 }) {
  const ins = db.prepare(
    `INSERT INTO llm_judgments (source_item_id, relevance_score, confidence, model, prompt_version, judged_at)
     VALUES (?, ?, ?, 'test-model', ?, datetime('now', ?))`,
  );
  for (let i = 0; i < n; i++) ins.run(i + 1, relevance(i), confidence(i), version, age);
}

const quiet = () => {};
const constants = parseShippedConstants(RUST_SRC);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

test('reads the shipped constants from Rust source', () => {
  assert.deepEqual(constants, { promptVersion: 'v6', relevanceBelow: 0.35, confidenceMin: 0.7 });
});

test('refuses to run on source it cannot read a constant from', () => {
  assert.throws(() => parseShippedConstants('// nothing here'), /PROMPT_VERSION/);
});

// ---------------------------------------------------------------------------
// Live cohort = newest judged_at, not the source constant
// ---------------------------------------------------------------------------

test('the live cohort is whatever was judged most recently in 24h, plus drain_v1', () => {
  const db = fixtureDb();
  cohort(db, 'v5', { n: 30, age: '-2 hours' });
  cohort(db, 'v4', { n: 30, age: '-3 days' });
  cohort(db, DRAIN_COHORT, { n: 5, age: '-30 minutes' }); // drain never masks the real cohort
  const live = liveCohortVersions(db);
  assert.deepEqual([...live].sort(), [DRAIN_COHORT, 'v5']);
  db.close();
});

test('with nothing judged in 24h only the drain cohort is live', () => {
  const db = fixtureDb();
  cohort(db, 'v5', { n: 30, age: '-3 days' });
  assert.deepEqual([...liveCohortVersions(db)], [DRAIN_COHORT]);
  db.close();
});

// ---------------------------------------------------------------------------
// MERGED != RUNNING
// ---------------------------------------------------------------------------

test('THE CASE: source v6, live rows v5, stale binary -> MERGED != RUNNING and v5 is still checked', () => {
  const db = fixtureDb();
  // v5 is live AND broken: 93% of its confidence sits on exactly 0.5.
  cohort(db, 'v5', { n: 100, age: '-1 hour', confidence: (i) => (i < 93 ? 0.5 : 0.9) });
  const binary = { path: 'fourda.exe', mtimeMs: Date.now() - 3 * 24 * HOUR };

  const r = run({ db, constants, binary, log: quiet });

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
  db.close();
});

test('a healthy live cohort under a stale binary still fails on the deploy gap alone', () => {
  const db = fixtureDb();
  cohort(db, 'v5', { n: 60, age: '-1 hour', confidence: (i) => 0.55 + (i % 10) / 25, relevance: (i) => (i % 3 ? 0.7 : 0.2) });
  const binary = { path: 'fourda.exe', mtimeMs: Date.now() - (STALE_BINARY_HOURS + 1) * HOUR };
  const r = run({ db, constants, binary, log: quiet });
  assert.equal(r.exitCode, 1);
  assert.deepEqual(r.findings.filter((f) => !f.startsWith('MERGED != RUNNING')), []);
  assert.equal(r.drift.status, 'merged_not_running');
  db.close();
});

test('a binary built within the stale window is warming, not a finding', () => {
  const db = fixtureDb();
  cohort(db, 'v5', { n: 60, age: '-1 hour', confidence: (i) => 0.55 + (i % 10) / 25, relevance: (i) => (i % 3 ? 0.7 : 0.2) });
  const binary = { path: 'fourda.exe', mtimeMs: Date.now() - 10 * 60_000 };
  const r = run({ db, constants, binary, log: quiet });
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.equal(r.drift.status, 'warming');
  db.close();
});

test('rows at the source version mean in sync, whatever the binary age', () => {
  const db = fixtureDb();
  cohort(db, 'v6', { n: 60, age: '-1 hour', confidence: (i) => 0.55 + (i % 10) / 25, relevance: (i) => (i % 3 ? 0.7 : 0.2) });
  const binary = { path: 'fourda.exe', mtimeMs: Date.now() - 30 * 24 * HOUR };
  const r = run({ db, constants, binary, log: quiet });
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.equal(r.drift.status, 'in_sync');
  db.close();
});

test('no binary at all cannot be running the source version', () => {
  const d = deployDrift({ promptVersion: 'v6', sourceRows: 0, liveVersion: 'v5', binary: null });
  assert.equal(d.status, 'merged_not_running');
  assert.match(d.message, /no built binary found/);
});

// ---------------------------------------------------------------------------
// The existing checks still fire on the live cohort
// ---------------------------------------------------------------------------

test('a one-sided live judge is still a finding', () => {
  const db = fixtureDb();
  cohort(db, 'v6', { n: 50, age: '-1 hour', confidence: (i) => 0.5 + (i % 10) / 20, relevance: () => 0.9 });
  const r = run({ db, constants, binary: { path: 'x', mtimeMs: Date.now() }, log: quiet });
  assert.equal(r.exitCode, 1);
  assert.ok(r.findings.some((f) => /reject rate 0%/.test(f)), r.findings.join(' | '));
  db.close();
});

test('a retired cohort with a spike is displayed but never a finding', () => {
  const db = fixtureDb();
  cohort(db, 'v3', { n: 100, age: '-30 days', confidence: () => 0.5 });
  cohort(db, 'v6', { n: 60, age: '-1 hour', confidence: (i) => 0.55 + (i % 10) / 25, relevance: (i) => (i % 3 ? 0.7 : 0.2) });
  const lines = [];
  const r = run({ db, constants, binary: { path: 'x', mtimeMs: Date.now() }, log: (l) => lines.push(l) });
  assert.equal(r.exitCode, 0, r.findings.join(' | '));
  assert.ok(lines.some((l) => /v3.*spike \(retired cohort\)/.test(l)));
  db.close();
});
