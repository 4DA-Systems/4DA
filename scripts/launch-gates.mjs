#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/*
 * launch-gates.mjs - read-only status of the AD-054 launch gates G1-G9.
 *
 *   pnpm run gates                      all gates from repo files, the DB, logs and CI
 *   pnpm run gates -- --json            machine-readable
 *   pnpm run gates -- --measure-idle 12 also sample idle CPU/GPU for 12 minutes (G6)
 *
 * Options:
 *   --db <path>             database (default: <main checkout>/data/4da.db, or FOURDA_DB)
 *   --app-root <dir>        the checkout the running app was built from (default: main checkout)
 *   --no-gh                 do not query GitHub (CI-backed gates show pending)
 *   --no-dbstat             skip the per-table size scan (~10 s on a 2 GB DB)
 *   --lane1 <file.json>     G5 weekly re-measure result {successes, n, measured_at}
 *   --dogfood-start <date>  G9 start date (YYYY-MM-DD)
 *   --measure-idle [min]    G6 idle sampler (Windows; default 12 minutes)
 *
 * Read-only by construction: the DB is opened with node:sqlite {readOnly: true}; settings.json is
 * read for one field; nothing is written, no setting changed, the app is never touched
 * beyond `document.getAnimations().length` through its Victauri bridge.
 * A gate whose data is missing reports `pending` with the reason, never `pass`.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  evaluateG1,
  evaluateG2,
  evaluateG3,
  evaluateG4,
  evaluateG5,
  evaluateG6,
  evaluateG7,
  evaluateG8,
  evaluateG9,
  parseColdStart,
  projectSteadyState,
  renderText,
} from './lib/launch-gates-core.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(HERE, '..');
const CONFORMANCE = join(REPO, 'src-tauri', 'tests', 'conformance');
const STACK_RETENTION_DEFAULT_DAYS = 30; // settings/types.rs RETENTION_DEFAULT_DAYS

function parseArgs(argv) {
  const a = { gh: true, dbstat: true, json: false };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    const next = () => argv[++i];
    if (k === '--json') a.json = true;
    else if (k === '--no-gh') a.gh = false;
    else if (k === '--no-dbstat') a.dbstat = false;
    else if (k === '--db') a.db = next();
    else if (k === '--app-root') a.appRoot = next();
    else if (k === '--lane1') a.lane1 = next();
    else if (k === '--dogfood-start') a.dogfoodStart = next();
    else if (k === '--measure-idle') a.measureIdle = /^\d+$/.test(argv[i + 1] || '') ? Number(next()) : 12;
    else throw new Error(`unknown option ${k}`);
  }
  return a;
}

/** The main checkout (parent of the git common dir), so worktrees read the real DB. */
export function mainCheckout(cwd = REPO) {
  try {
    const common = execFileSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], {
      cwd,
      encoding: 'utf8',
      windowsHide: true,
    }).trim();
    return dirname(common);
  } catch {
    return cwd;
  }
}

const readJson = (p) => (existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : null);
const readJsonl = (p) =>
  existsSync(p) ? readFileSync(p, 'utf8').split(/\r?\n/).filter(Boolean).map((l) => JSON.parse(l)) : [];

// --- conformance corpus (G1/G2) -------------------------------------------
export function readCorpus(dir = CONFORMANCE) {
  const casesDir = join(dir, 'cases');
  const cases = existsSync(casesDir)
    ? readdirSync(casesDir)
        .map((c) => readJson(join(casesDir, c, 'case.json')))
        .filter(Boolean)
    : [];
  const formats = new Set();
  const expDir = join(dir, 'expected');
  if (existsSync(expDir)) {
    for (const f of readdirSync(expDir).filter((f) => f.endsWith('.json'))) {
      for (const l of readJson(join(expDir, f))?.lockfiles || []) if (l.status === 'supported') formats.add(l.format);
    }
  }
  return {
    thresholds: readJson(join(dir, 'thresholds.json')),
    cases,
    formats: [...formats].sort(),
    fixTargets: readJsonl(join(dir, 'vectors', 'fix_target.jsonl')),
    skip: readJson(join(dir, 'vectors', 'engines_skip.json')),
  };
}

// --- GitHub ---------------------------------------------------------------
function gh(args, cwd = REPO) {
  return JSON.parse(execFileSync('gh', args, { cwd, encoding: 'utf8', windowsHide: true, timeout: 30000 }));
}

/** Latest merge-queue Validate run on 4DA: the Rust legs run the corpus + vectors. */
export function rustCi() {
  try {
    const [run] = gh(['run', 'list', '--workflow', 'validate.yml', '--event', 'merge_group', '--status', 'completed', '--limit', '1', '--json', 'databaseId,headSha,createdAt']);
    if (!run) return { error: 'no merge-queue Validate run found' };
    const { jobs } = gh(['run', 'view', String(run.databaseId), '--json', 'jobs']);
    const rust = jobs.filter((j) => /^Rust \(/.test(j.name) && !/calibration/i.test(j.name));
    if (!rust.length) return { error: `run ${run.databaseId} has no Rust job` };
    const bad = rust.find((j) => j.conclusion && j.conclusion !== 'success' && j.conclusion !== 'skipped');
    const running = rust.some((j) => !j.conclusion);
    return {
      label: `merge-queue Validate ${run.databaseId} Rust legs (${rust.map((j) => j.name).join(', ')})`,
      conclusion: bad ? bad.conclusion : running ? null : rust.every((j) => j.conclusion === 'skipped') ? 'skipped' : 'success',
      when: run.createdAt,
      sha: run.headSha.slice(0, 9),
    };
  } catch (e) {
    return { error: `gh failed: ${String(e.message).split('\n')[0]}` };
  }
}

// --- MCP repo (G3) ----------------------------------------------------------
export function readMcp(root, useGh) {
  const candidates = [
    process.env.FOURDA_MCP_REPO,
    join(dirname(root), 'runyourempire', '4da-mcp-server'),
    join(dirname(root), '4da-mcp-server'),
  ].filter(Boolean);
  const repo = candidates.find((c) => existsSync(join(c, 'src', '__tests__', 'g3-breaking-precision.test.ts')));
  if (!repo) return { testFound: false, where: `G3 test not found in ${candidates.join(', ')}` };
  const testPath = join(repo, 'src', '__tests__', 'g3-breaking-precision.test.ts');
  const src = readFileSync(testPath, 'utf8');
  const bars = [...src.matchAll(/precision\)\.toBeGreaterThanOrEqual\(([\d.]+)\)/g)].map((m) => Number(m[1]));
  let ci = { error: 'gh disabled' };
  if (useGh) {
    try {
      const [run] = gh(['run', 'list', '-R', '4DA-Systems/4da-mcp-server', '--workflow', 'ci.yml', '--branch', 'main', '--event', 'push', '--status', 'completed', '--limit', '1', '--json', 'databaseId,conclusion,headSha,createdAt,workflowName']);
      ci = run
        ? { label: `4da-mcp-server ${run.workflowName} ${run.databaseId} on main`, conclusion: run.conclusion || null, when: run.createdAt, sha: run.headSha.slice(0, 9) }
        : { error: 'no MCP main run found' };
    } catch (e) {
      ci = { error: `gh failed: ${String(e.message).split('\n')[0]}` };
    }
  }
  return { testFound: true, testPath, threshold: bars.length ? Math.min(...bars) : null, ci };
}

// --- DB (G4/G6) -------------------------------------------------------------
/** node:sqlite (built in, no native install step), opened read-only. */
export async function openReadOnly(dbPath) {
  if (!existsSync(dbPath)) throw new Error(`${dbPath} not found`);
  const emit = process.emitWarning;
  process.emitWarning = (w, ...rest) => (/SQLite/.test(String(w)) ? undefined : emit.call(process, w, ...rest));
  try {
    const { DatabaseSync } = await import('node:sqlite');
    const db = new DatabaseSync(dbPath, { readOnly: true });
    db.exec('PRAGMA busy_timeout = 5000');
    return db;
  } finally {
    process.emitWarning = emit;
  }
}

const localDay = (iso, offsetMin) => new Date(Date.parse(iso) + offsetMin * 60000).toISOString().slice(0, 10);

export function readBrief(db, offsetMin = -new Date().getTimezoneOffset()) {
  const row = db.prepare("SELECT value FROM kv_store WHERE key = 'morning_brief_outcomes_v1'").get();
  const outcomes = row ? JSON.parse(row.value) : {};
  let briefingDays = [];
  try {
    briefingDays = [
      ...new Set(db.prepare('SELECT created_at FROM briefings').all().map((r) => localDay(`${r.created_at.replace(' ', 'T')}Z`, offsetMin))),
    ].sort();
  } catch {
    briefingDays = [];
  }
  return { outcomes, briefingDays };
}

/** Retention the app uses: settings.json monitoring.cleanup_max_age_days, clamped like retention_days(). */
export function readRetention(settingsPath) {
  let v = null;
  try {
    v = JSON.parse(readFileSync(settingsPath, 'utf8'))?.monitoring?.cleanup_max_age_days ?? null;
  } catch {
    v = null;
  }
  const days = Math.min(365, Math.max(7, v ?? STACK_RETENTION_DEFAULT_DAYS));
  return { configured: v, days, defaultDays: STACK_RETENTION_DEFAULT_DAYS };
}

const ITEM_TABLE = /^(source_vec|source_items|scoring_explanations|item_vec|item_context|idx_source|idx_si_|sqlite_autoindex_source_items)/;

export function collectDbFootprint(db, dbPath, { retentionDays, dbstat = true } = {}) {
  const one = (sql) => db.prepare(sql).get();
  const pageSize = one('PRAGMA page_size').page_size;
  const pageCount = one('PRAGMA page_count').page_count;
  const freelist = one('PRAGMA freelist_count').freelist_count;
  const size = (p) => (existsSync(p) ? statSync(p).size : 0);
  const enabled = "source_type IN (SELECT source_type FROM sources WHERE enabled = 1)";
  const c = one(`SELECT COUNT(*) items,
      SUM(CASE WHEN ${enabled} THEN 0 ELSE 1 END) disabled,
      SUM(CASE WHEN ${enabled} AND created_at >= datetime('now','-30 days') THEN 1 ELSE 0 END) en_created30,
      SUM(CASE WHEN ${enabled} AND last_seen >= datetime('now','-30 days') THEN 1 ELSE 0 END) en_seen30,
      SUM(CASE WHEN ${enabled} AND created_at < datetime('now','-30 days') AND last_seen >= datetime('now','-3 days') THEN 1 ELSE 0 END) perennial,
      SUM(CASE WHEN last_seen < datetime('now', '-${retentionDays} days') THEN 1 ELSE 0 END) past_retention,
      MIN(last_seen) oldest_seen, MAX(CASE WHEN ${enabled} THEN NULL ELSE last_seen END) disabled_last_seen
    FROM source_items`);
  // sqlite-vec 0.1 frees a vector chunk only once every slot in it is empty and only
  // appends to the newest chunk, so a live item interleaved with pruned ones pins
  // the whole chunk (1024 x 768 floats, ~3 MiB).
  let vecChunks = null;
  try {
    vecChunks = { ...one(`SELECT (SELECT COUNT(*) FROM source_vec_chunks) chunks,
        (SELECT COUNT(DISTINCT r.chunk_id) FROM source_items si JOIN source_vec_rowids r ON r.rowid = si.id
          WHERE si.${enabled} AND si.last_seen >= datetime('now', '-${retentionDays} days')) pinned`) };
  } catch {
    vecChunks = null;
  }
  const total = pageCount * pageSize;
  let itemBytes = null;
  let tables = null;
  if (dbstat) {
    try {
      tables = db
        .prepare("SELECT name, pgsize FROM dbstat('main', 1) ORDER BY pgsize DESC")
        .all()
        .map((t) => ({ name: t.name, pgsize: t.pgsize }));
      itemBytes = tables.filter((t) => ITEM_TABLE.test(t.name)).reduce((a, t) => a + t.pgsize, 0);
    } catch {
      tables = null;
    }
  }
  // Without dbstat, every byte is attributed to items: an upper bound.
  const bytesPerItem = c.items ? (itemBytes ?? total) / c.items : 0;
  const fixedBytes = itemBytes == null ? 0 : total - itemBytes;
  const project = (days) =>
    projectSteadyState({
      retentionDays: days,
      enabledCreated30: c.en_created30 || 0,
      enabledSeen30: c.en_seen30 || 0,
      perennial: c.perennial || 0,
      bytesPerItem,
      fixedBytes,
    });
  const projection = project(retentionDays);
  const projectionDefault = project(STACK_RETENTION_DEFAULT_DAYS);
  return {
    path: dbPath,
    fileBytes: size(dbPath),
    walBytes: size(`${dbPath}-wal`),
    pageSize,
    pageCount,
    freelistBytes: freelist * pageSize,
    items: c.items,
    disabledItems: c.disabled || 0,
    pastRetention: c.past_retention || 0,
    oldestSeen: c.oldest_seen,
    disabledLastSeen: c.disabled_last_seen,
    bytesPerItem: Math.round(bytesPerItem),
    fixedBytes,
    topTables: tables ? tables.slice(0, 8) : null,
    sizeIsUpperBound: itemBytes == null,
    vecChunks,
    projection,
    projectionDefault,
  };
}

export function latestColdStart(logDir) {
  if (!existsSync(logDir)) return null;
  const logs = readdirSync(logDir)
    .filter((f) => f.startsWith('fourda.log'))
    .map((f) => ({ f, m: statSync(join(logDir, f)).mtimeMs }))
    .sort((a, b) => b.m - a.m);
  for (const { f } of logs.slice(0, 7)) {
    const parsed = parseColdStart(readFileSync(join(logDir, f), 'utf8'), join(logDir, f));
    if (parsed) return parsed;
  }
  return null;
}

// --- main -------------------------------------------------------------------
export async function collect(opts) {
  const root = mainCheckout();
  const appRoot = opts.appRoot || root;
  const dbPath = opts.db || process.env.FOURDA_DB || join(root, 'data', '4da.db');
  const corpus = readCorpus();
  const ci = opts.gh ? rustCi() : { error: 'gh disabled (--no-gh)' };
  const today = new Date(Date.now() - new Date().getTimezoneOffset() * 60000).toISOString().slice(0, 10);

  let brief = null;
  let footprint = null;
  let dbError = null;
  const retention = readRetention(join(root, 'data', 'settings.json'));
  if (existsSync(dbPath)) {
    let db;
    try {
      db = await openReadOnly(dbPath);
      brief = readBrief(db);
      footprint = collectDbFootprint(db, dbPath, { retentionDays: retention.days, dbstat: opts.dbstat });
    } catch (e) {
      dbError = String(e.message);
    } finally {
      db?.close();
    }
  } else dbError = `${dbPath} not found`;

  let idle = null;
  if (opts.measureIdle) {
    const { measureIdle } = await import('./lib/launch-gates-idle.mjs');
    const r = await measureIdle({ appRoot, minutes: opts.measureIdle, log: (l) => process.stderr.write(`${l}\n`) });
    idle = r.summary.idle;
  }

  const capSrc = join(REPO, 'src-tauri', 'src', 'morning_floor.rs');
  const capConstant = existsSync(capSrc)
    ? Number(/MORNING_ACT_NOW_SHOWN: usize = (\d+)/.exec(readFileSync(capSrc, 'utf8'))?.[1]) || null
    : null;
  const releaseYml = join(REPO, '.github', 'workflows', 'release.yml');
  const signingConfigured = existsSync(releaseYml) && /signCommand|artifact-signing|ArtifactSigning|AZURE_.*SIGN/i.test(readFileSync(releaseYml, 'utf8'));
  const lane1 = opts.lane1 ? { ...readJson(opts.lane1), source: opts.lane1 } : null;

  const g4 = brief ? evaluateG4({ ...brief, today, capConstant }) : evaluateG4({ outcomes: {}, today });
  if (dbError) g4.summary += `; DB: ${dbError}`;
  return {
    generated_at: new Date().toISOString(),
    db: dbPath,
    retention,
    footprint,
    gates: [
      evaluateG1({ ...corpus, ci }),
      evaluateG2({ ...corpus, ci }),
      evaluateG3({ mcp: readMcp(root, opts.gh) }),
      g4,
      evaluateG5({ lane1 }),
      evaluateG6({ db: footprint, idle, cold: latestColdStart(join(root, 'data', 'logs')) }),
      evaluateG7(),
      evaluateG8({ signingConfigured }),
      evaluateG9({ dogfoodStart: opts.dogfoodStart, today }),
    ],
  };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const report = await collect(opts);
  if (opts.json) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }
  console.log(`4DA launch gates (AD-054)  ${report.generated_at}`);
  console.log(`DB ${report.db} (read-only); retention ${report.retention.days} days (configured ${report.retention.configured ?? 'default'}, shipped default ${report.retention.defaultDays})`);
  console.log('');
  console.log(renderText(report.gates));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(`launch-gates: ${e.message}`);
    process.exit(2);
  });
}
