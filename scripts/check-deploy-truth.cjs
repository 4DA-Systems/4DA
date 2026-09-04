#!/usr/bin/env node
/**
 * Deploy truth — is the code you are reading the code that is running?
 *
 *   pnpm run check:deploy-truth
 *
 * Exit codes:
 *   0  in sync — the built binaries postdate the last src-tauri commit and the
 *      source's PIPELINE_VERSION / PROMPT_VERSION both have live rows (or the
 *      binaries are too fresh for rows to exist yet — reported as "warming").
 *   1  drift —
 *        (a) a debug binary is older than the last commit touching src-tauri/
 *            by more than STALE_HOURS, or is missing: what runs is not what
 *            was merged; or
 *        (b) the source PIPELINE_VERSION or PROMPT_VERSION has NO live rows
 *            although the newest binary is already older than STALE_HOURS —
 *            the binary was built, but nothing is running it (a scheduled
 *            task still pointing at an old exe, the app never restarted).
 *   2  could not run — a constant is unreadable from the Rust source, or the
 *      database is missing. Never silently green.
 *
 * Why this exists: "Merged" is not "running". Between 2026-09-01 and
 * 2026-09-04 the source said PROMPT_VERSION v6 while every live judgment was
 * v5, because src-tauri/target/debug/fourda.exe — the binary the scheduled
 * refresh actually executes — predated the merge. Nothing compared the two.
 * The judge monitor reported on a cohort that did not exist and exited 0; the
 * Signal tab rendered scores from a pipeline version the reader had already
 * moved past. This script is the comparison. Read-only: it opens the database
 * with `readonly` + `query_only`, and touches nothing else.
 *
 * Every version constant is READ FROM THE RUST SOURCE by regex, never copied
 * here (exit 2 when a pattern stops matching) — a checker with its own copy of
 * a constant checks a build that no longer exists.
 *
 * Structure: `evaluate()` is pure over gathered facts, and `dbFacts()` reads
 * through an injected `one(sql, ...params)` row getter, so the tests never
 * touch SQLite. The native module is required lazily, inside the real
 * database path only — CI's "Repo guards" job installs without building
 * native modules, and a top-level `require('better-sqlite3')` made the test
 * file unloadable there.
 */

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..');
const DEFAULT_DB_PATH = process.env.FOURDA_DB_PATH || path.join(repoRoot, 'data', '4da.db');
const SCORING_SRC = path.join(repoRoot, 'src-tauri', 'src', 'scoring', 'mod.rs');
const JUDGE_SRC = path.join(repoRoot, 'src-tauri', 'src', 'llm_judgments.rs');
const BIN_DIR = process.env.FOURDA_BIN_DIR || path.join(repoRoot, 'src-tauri', 'target', 'debug');
const BINARIES = ['fourda.exe', 'fourda-engine.exe'];

/** A binary lagging the source (or leading the database) by more than this is drift, not a warm-up. */
const STALE_HOURS = 6;
const HOUR = 3_600_000;

// ── Constants from source ────────────────────────────────────────────────
class ConstantMissing extends Error {}

function constFromRust(source, name, pattern) {
  const m = source.match(pattern);
  if (!m) throw new ConstantMissing(name);
  return m[1];
}

function readSourceVersions({ scoringSrc, judgeSrc }) {
  return {
    pipelineVersion: Number(
      constFromRust(scoringSrc, 'PIPELINE_VERSION', /const PIPELINE_VERSION:\s*i32\s*=\s*(\d+)\s*;/),
    ),
    promptVersion: constFromRust(judgeSrc, 'PROMPT_VERSION', /const PROMPT_VERSION: &str = "([^"]+)"/),
  };
}

// ── Facts (all I/O lives here) ───────────────────────────────────────────
function lastSrcTauriCommit(cwd) {
  const r = spawnSync('git', ['log', '-1', '--format=%cI', '--', 'src-tauri/'], { cwd, encoding: 'utf8' });
  if (r.status !== 0) return null;
  const iso = r.stdout.trim();
  return iso ? iso : null;
}

function binaryFacts(binDir) {
  return BINARIES.map((name) => {
    const p = path.join(binDir, name);
    try {
      return { name, path: p, mtimeMs: fs.statSync(p).mtimeMs };
    } catch {
      return { name, path: p, mtimeMs: null };
    }
  });
}

/**
 * Read the database-side facts through `one(sql, ...params) -> row|undefined`
 * (the production getter is `db.prepare(sql).get(...params)`; the tests pass a
 * stub). Absent rows degrade to null / 0, never to a throw.
 */
function dbFacts(one, { pipelineVersion, promptVersion }) {
  return {
    maxPipelineVersion: one('SELECT MAX(scored_pipeline_version) v FROM source_items')?.v ?? null,
    rowsAtPipelineVersion:
      one('SELECT COUNT(*) n FROM source_items WHERE scored_pipeline_version = ?', pipelineVersion)?.n ?? 0,
    latestPromptVersion24h:
      one(
        `SELECT prompt_version v FROM llm_judgments
         WHERE judged_at >= datetime('now', '-1 day') AND prompt_version != 'drain_v1'
         ORDER BY judged_at DESC LIMIT 1`,
      )?.v ?? null,
    rowsAtPromptVersion:
      one('SELECT COUNT(*) n FROM llm_judgments WHERE prompt_version = ?', promptVersion)?.n ?? 0,
    latestEngineRun: one('SELECT started_at, ok FROM engine_runs ORDER BY started_at DESC LIMIT 1') ?? null,
  };
}

/** Open the live database read-only. The native module loads here and only here. */
function openReadOnly(dbPath) {
  const Database = require('better-sqlite3');
  const db = new Database(dbPath, { readonly: true, fileMustExist: true });
  db.pragma('query_only = 1');
  return db;
}

// ── Evaluation (pure — tested) ───────────────────────────────────────────
/**
 * @param {object} facts
 * @param {{pipelineVersion:number, promptVersion:string}} facts.source
 * @param {string|null} facts.lastCommitIso  last commit touching src-tauri/
 * @param {{name:string, path:string, mtimeMs:number|null}[]} facts.binaries
 * @param {ReturnType<typeof dbFacts>} facts.db
 * @param {{now?: number, staleHours?: number}} [opts]
 * @returns {{ rows: [string, string][], problems: string[], notes: string[], exitCode: number }}
 */
function evaluate(facts, { now = Date.now(), staleHours = STALE_HOURS } = {}) {
  const { source, lastCommitIso, binaries, db } = facts;
  const problems = [];
  const notes = [];
  const iso = (ms) => (ms == null ? 'missing' : new Date(ms).toISOString());
  const hours = (ms) => (ms / HOUR).toFixed(1);

  const commitMs = lastCommitIso ? Date.parse(lastCommitIso) : null;
  const built = binaries.filter((b) => b.mtimeMs != null);
  const newestBuiltMs = built.length ? Math.max(...built.map((b) => b.mtimeMs)) : null;

  // (a) binaries vs the last src-tauri commit
  for (const b of binaries) {
    if (b.mtimeMs == null) {
      problems.push(`${b.name} not found at ${b.path} — nothing built is running the source`);
      continue;
    }
    if (commitMs != null && commitMs - b.mtimeMs > staleHours * HOUR) {
      problems.push(
        `${b.name} built ${iso(b.mtimeMs)} is ${hours(commitMs - b.mtimeMs)}h OLDER than the last ` +
          `src-tauri commit (${lastCommitIso}) — MERGED != RUNNING, rebuild both binaries`,
      );
    }
  }

  // (b) source versions vs what the database is actually receiving
  const binaryAgeMs = newestBuiltMs == null ? null : now - newestBuiltMs;
  const noRowsYet = (label, rows, liveLabel) => {
    if (rows > 0) return;
    if (binaryAgeMs != null && binaryAgeMs <= staleHours * HOUR) {
      notes.push(`${label}: no live rows yet, binary built ${hours(binaryAgeMs)}h ago — warming, re-check after ${staleHours}h`);
      return;
    }
    const age = binaryAgeMs == null ? 'no binary' : `binary built ${hours(binaryAgeMs)}h ago`;
    problems.push(`${label}: source declares it, the database has NO rows carrying it, and the ${age} — MERGED != RUNNING (live: ${liveLabel})`);
  };
  noRowsYet(
    `PIPELINE_VERSION ${source.pipelineVersion}`,
    db.rowsAtPipelineVersion,
    `max scored_pipeline_version ${db.maxPipelineVersion ?? 'none'}`,
  );
  noRowsYet(
    `PROMPT_VERSION ${source.promptVersion}`,
    db.rowsAtPromptVersion,
    `newest 24h prompt_version ${db.latestPromptVersion24h ?? 'none'}`,
  );

  const rows = [
    ['PIPELINE_VERSION (source)', String(source.pipelineVersion)],
    ['max scored_pipeline_version (db)', String(db.maxPipelineVersion ?? 'none')],
    ['rows at source PIPELINE_VERSION', String(db.rowsAtPipelineVersion)],
    ['PROMPT_VERSION (source)', source.promptVersion],
    ['newest prompt_version, 24h (db)', db.latestPromptVersion24h ?? 'none'],
    ['rows at source PROMPT_VERSION', String(db.rowsAtPromptVersion)],
    [
      'latest engine run',
      db.latestEngineRun ? `${db.latestEngineRun.started_at} (ok=${db.latestEngineRun.ok})` : 'none',
    ],
    ['last src-tauri commit', lastCommitIso ?? 'unknown (git unavailable)'],
    ...binaries.map((b) => {
      const rel =
        b.mtimeMs == null || commitMs == null
          ? ''
          : b.mtimeMs >= commitMs
            ? `  (+${hours(b.mtimeMs - commitMs)}h after commit)`
            : `  (${hours(commitMs - b.mtimeMs)}h BEFORE commit)`;
      return [`${b.name} built`, `${iso(b.mtimeMs)}${rel}`];
    }),
  ];

  return { rows, problems, notes, exitCode: problems.length === 0 ? 0 : 1 };
}

function printReport({ rows, problems, notes, exitCode }, log = console.log) {
  const width = Math.max(...rows.map(([k]) => k.length));
  log('deploy truth');
  for (const [k, v] of rows) log(`  ${k.padEnd(width)}  ${v}`);
  for (const n of notes) log(`  ~ ${n}`);
  log('');
  if (exitCode === 0) {
    log('in sync — the binaries postdate the last src-tauri commit and the source versions have live rows.');
  } else {
    log(`${problems.length} problem(s):`);
    for (const p of problems) log(`  ! ${p}`);
  }
}

function main() {
  let source;
  try {
    source = readSourceVersions({
      scoringSrc: fs.readFileSync(SCORING_SRC, 'utf8'),
      judgeSrc: fs.readFileSync(JUDGE_SRC, 'utf8'),
    });
  } catch (e) {
    console.error(`Could not read ${e instanceof ConstantMissing ? e.message : 'the version constants'} from the Rust source.`);
    console.error('This check reads the shipped constants on purpose — a copy would check a build that no longer exists.');
    process.exit(2);
  }
  if (!fs.existsSync(DEFAULT_DB_PATH)) {
    console.error(`No database at ${DEFAULT_DB_PATH}. Set FOURDA_DB_PATH to point at one.`);
    process.exit(2);
  }

  const db = openReadOnly(DEFAULT_DB_PATH);
  let facts;
  try {
    facts = {
      source,
      lastCommitIso: lastSrcTauriCommit(repoRoot),
      binaries: binaryFacts(BIN_DIR),
      db: dbFacts((sql, ...params) => db.prepare(sql).get(...params), source),
    };
  } finally {
    db.close();
  }
  console.log(`  db: ${DEFAULT_DB_PATH}`);
  const result = evaluate(facts);
  printReport(result);
  process.exit(result.exitCode);
}

if (require.main === module) {
  main();
}

module.exports = { readSourceVersions, dbFacts, evaluate, printReport, STALE_HOURS };
