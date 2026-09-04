// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for scripts/check-deploy-truth.cjs — "is the code you are reading the
// code that is running?". The evaluation is pure over gathered facts, so every
// case here is a fixture; the I/O (git, stat, sqlite) is exercised by running
// the script itself against the live database.
//
// Run: node --test scripts/check-deploy-truth.test.cjs   (or `pnpm run test:scripts`)

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { readSourceVersions, dbFacts, evaluate, STALE_HOURS } = require('./check-deploy-truth.cjs');

const HOUR = 3_600_000;

test('loading this test never loads the native SQLite module', () => {
  // The exact CI failure: `require('better-sqlite3')` at module top made the
  // test file unloadable where native modules are not built.
  const loaded = Object.keys(require.cache).some((k) => /[\\/]better-sqlite3[\\/]/.test(k));
  assert.equal(loaded, false, 'better-sqlite3 must only load inside the real database path');
});

// ---------------------------------------------------------------------------
// dbFacts over a stubbed row getter (no SQLite)
// ---------------------------------------------------------------------------

/** A `one(sql, ...params)` stub keyed on the distinguishing SQL fragment. */
function rowGetter(answers) {
  return (sql) => {
    for (const [fragment, row] of Object.entries(answers)) {
      if (sql.includes(fragment)) return row;
    }
    throw new Error(`unexpected query: ${sql}`);
  };
}

test('dbFacts assembles the live shape from the five queries', () => {
  const one = rowGetter({
    'MAX(scored_pipeline_version)': { v: 28 },
    'scored_pipeline_version = ?': { n: 70_960 },
    'ORDER BY judged_at DESC': { v: 'v6' },
    'llm_judgments WHERE prompt_version = ?': { n: 128 },
    'FROM engine_runs': { started_at: '2026-09-04T03:49:58+00:00', ok: 1 },
  });
  assert.deepEqual(dbFacts(one, { pipelineVersion: 28, promptVersion: 'v6' }), {
    maxPipelineVersion: 28,
    rowsAtPipelineVersion: 70_960,
    latestPromptVersion24h: 'v6',
    rowsAtPromptVersion: 128,
    latestEngineRun: { started_at: '2026-09-04T03:49:58+00:00', ok: 1 },
  });
});

test('dbFacts on an empty database degrades to null / 0, never a throw', () => {
  const one = rowGetter({
    'MAX(scored_pipeline_version)': { v: null },
    'scored_pipeline_version = ?': { n: 0 },
    'ORDER BY judged_at DESC': undefined,
    'llm_judgments WHERE prompt_version = ?': { n: 0 },
    'FROM engine_runs': undefined,
  });
  assert.deepEqual(dbFacts(one, { pipelineVersion: 29, promptVersion: 'v7' }), {
    maxPipelineVersion: null,
    rowsAtPipelineVersion: 0,
    latestPromptVersion24h: null,
    rowsAtPromptVersion: 0,
    latestEngineRun: null,
  });
});
const NOW = Date.parse('2026-09-04T12:00:00Z');
const COMMIT = '2026-09-01T16:37:38Z';
const COMMIT_MS = Date.parse(COMMIT);

function facts(overrides = {}) {
  const base = {
    source: { pipelineVersion: 28, promptVersion: 'v6' },
    lastCommitIso: COMMIT,
    binaries: [
      { name: 'fourda.exe', path: 'x/fourda.exe', mtimeMs: COMMIT_MS + 2 * HOUR },
      { name: 'fourda-engine.exe', path: 'x/fourda-engine.exe', mtimeMs: COMMIT_MS + 2 * HOUR },
    ],
    db: {
      maxPipelineVersion: 28,
      rowsAtPipelineVersion: 70_960,
      latestPromptVersion24h: 'v6',
      rowsAtPromptVersion: 128,
      latestEngineRun: { started_at: '2026-09-04T03:49:58+00:00', ok: 1 },
    },
  };
  return { ...base, ...overrides, db: { ...base.db, ...(overrides.db ?? {}) } };
}

test('reads both version constants from Rust source', () => {
  const v = readSourceVersions({
    scoringSrc: 'pub(crate) const PIPELINE_VERSION: i32 = 28;\n',
    judgeSrc: 'pub(crate) const PROMPT_VERSION: &str = "v6";\n',
  });
  assert.deepEqual(v, { pipelineVersion: 28, promptVersion: 'v6' });
});

test('refuses to run when a constant pattern stops matching (exit 2 path)', () => {
  assert.throws(
    () => readSourceVersions({ scoringSrc: 'const PIPELINE_VERSION = 28', judgeSrc: '' }),
    /PIPELINE_VERSION/,
  );
});

test('in sync: binaries postdate the commit and both source versions have rows', () => {
  const r = evaluate(facts(), { now: NOW });
  assert.equal(r.exitCode, 0, r.problems.join(' | '));
  assert.deepEqual(r.problems, []);
  assert.ok(r.rows.some(([k, v]) => k === 'PIPELINE_VERSION (source)' && v === '28'));
  assert.ok(r.rows.some(([k]) => k === 'fourda.exe built'));
});

test('(a) a binary older than the last src-tauri commit by > STALE_HOURS is drift', () => {
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x', mtimeMs: COMMIT_MS - (STALE_HOURS + 1) * HOUR },
        { name: 'fourda-engine.exe', path: 'x', mtimeMs: COMMIT_MS + HOUR },
      ],
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 1);
  assert.equal(r.problems.length, 1);
  assert.match(r.problems[0], /fourda\.exe .* OLDER than the last src-tauri commit/);
  assert.match(r.problems[0], /MERGED != RUNNING/);
});

test('(a) a binary a few hours older than the commit is tolerated (same-day rebuild lag)', () => {
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x', mtimeMs: COMMIT_MS - 2 * HOUR },
        { name: 'fourda-engine.exe', path: 'x', mtimeMs: COMMIT_MS - 2 * HOUR },
      ],
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 0, r.problems.join(' | '));
});

test('(a) a missing binary is drift, never silently in sync', () => {
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x/fourda.exe', mtimeMs: COMMIT_MS + HOUR },
        { name: 'fourda-engine.exe', path: 'x/fourda-engine.exe', mtimeMs: null },
      ],
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 1);
  assert.match(r.problems[0], /fourda-engine\.exe not found/);
});

test('(b) THE CASE: source says v6, no v6 rows, binary older than STALE_HOURS -> MERGED != RUNNING', () => {
  // 2026-09-01..04: the merge landed, the scheduled task kept running the old exe.
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x', mtimeMs: NOW - 3 * 24 * HOUR },
        { name: 'fourda-engine.exe', path: 'x', mtimeMs: NOW - 3 * 24 * HOUR },
      ],
      lastCommitIso: new Date(NOW - 4 * 24 * HOUR).toISOString(), // binaries postdate the commit: (a) is quiet
      db: { latestPromptVersion24h: 'v5', rowsAtPromptVersion: 0 },
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 1);
  assert.equal(r.problems.length, 1);
  assert.match(r.problems[0], /PROMPT_VERSION v6: source declares it, the database has NO rows/);
  assert.match(r.problems[0], /live: newest 24h prompt_version v5/);
});

test('(b) applies to PIPELINE_VERSION too', () => {
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x', mtimeMs: NOW - 8 * HOUR },
        { name: 'fourda-engine.exe', path: 'x', mtimeMs: NOW - 8 * HOUR },
      ],
      lastCommitIso: new Date(NOW - 9 * HOUR).toISOString(),
      source: { pipelineVersion: 29, promptVersion: 'v6' },
      db: { maxPipelineVersion: 28, rowsAtPipelineVersion: 0 },
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 1);
  assert.match(r.problems[0], /PIPELINE_VERSION 29: .*NO rows/);
  assert.match(r.problems[0], /max scored_pipeline_version 28/);
});

test('(b) a freshly built binary with no rows yet is warming, exit 0', () => {
  const r = evaluate(
    facts({
      binaries: [
        { name: 'fourda.exe', path: 'x', mtimeMs: NOW - 20 * 60_000 },
        { name: 'fourda-engine.exe', path: 'x', mtimeMs: NOW - 25 * 60_000 },
      ],
      db: { rowsAtPromptVersion: 0, latestPromptVersion24h: 'v5' },
    }),
    { now: NOW },
  );
  assert.equal(r.exitCode, 0, r.problems.join(' | '));
  assert.ok(r.notes.some((n) => /PROMPT_VERSION v6: no live rows yet/.test(n)));
});

test('git unavailable degrades to a visible "unknown", not a false pass on (a)', () => {
  const r = evaluate(facts({ lastCommitIso: null }), { now: NOW });
  assert.equal(r.exitCode, 0);
  assert.ok(r.rows.some(([k, v]) => k === 'last src-tauri commit' && /unknown/.test(v)));
});
