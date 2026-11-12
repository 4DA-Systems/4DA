#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/*
 * engine-e2e.mjs - the engine end-to-end gate (audit 2026-11-11, Q1).
 *
 * Runs the real `fourda-engine` binary, as a user's machine runs it, over
 * conformance cases copied into a fresh context directory, against a FROZEN
 * advisory mirror served from 127.0.0.1 (no network), and asserts on the
 * database it leaves behind (opened read-only):
 *
 *   run 1  fresh profile, mirror up      inventory per case == the conformance truth
 *                                        (minus the directories the coverage record names
 *                                        as skipped), every truth finding stored as an
 *                                        advisory, every ecosystem verified (error NULL),
 *                                        coverage record == golden, plan snapshot == golden
 *   run 2  one lockfile changed, mirror up   the change is in the inventory, the plan follows it
 *   run 3  mirror down (network blocked)     exit 1, every ecosystem "could not check",
 *                                        verified times NOT moved, inventory and plan kept
 *
 * plus a wall-time and peak-memory budget per run. Configuration and goldens live in
 * src-tauri/tests/e2e-engine/ (README there says how to update them).
 *
 *   node scripts/ci/engine-e2e.mjs --engine <path to fourda-engine> [--work <dir>]
 *        [--update-goldens] [--keep]
 *
 * The engine must be a debug build with `--features e2e-osv-mirror`, which is what
 * makes it read FOURDA_E2E_OSV_MIRROR (osv/cache.rs). "Network blocked" is enforced by
 * pointing every proxy variable at a closed port (NO_PROXY exempts only 127.0.0.1) and,
 * for run 3, stopping the mirror: the engine's HTTP stack honours the proxy variables,
 * so nothing reaches a remote host.
 *
 * Exit: 0 every assertion held; 1 an assertion failed; 2 the harness could not run.
 */

import { spawn, execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  applyMutation,
  buildZip,
  diffSets,
  etagOf,
  expectedFindingKeys,
  expectedInventory,
  firstDifference,
  groupByEcosystem,
  inventoryByCase,
  locate,
  normalizePlan,
  normName,
  normVersion,
  osvEcosystem,
} from './engine-e2e-lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '..', '..');
const CONFORMANCE = join(REPO, 'src-tauri', 'tests', 'conformance');
const GATE_DIR = join(REPO, 'src-tauri', 'tests', 'e2e-engine');
const DEAD_PROXY = 'http://127.0.0.1:9';

function parseArgs(argv) {
  const a = { update: false, keep: false };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    if (k === '--engine') a.engine = argv[++i];
    else if (k === '--work') a.work = argv[++i];
    else if (k === '--update-goldens') a.update = true;
    else if (k === '--keep') a.keep = true;
    else throw new Error(`unknown option ${k}`);
  }
  if (!a.engine) throw new Error('--engine <path to fourda-engine> is required');
  // Not under the OS temp folder: the engine treats a project there as a
  // temporary copy and does not check it (ace/lockfile/report.rs TEMP_DIR_REASON).
  a.work = resolve(a.work || join(homedir(), 'fourda-engine-e2e'));
  return a;
}

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));
const log = (...xs) => console.log('[engine-e2e]', ...xs);

// --- setup ---------------------------------------------------------------------------

function buildMirror(dir) {
  const osvDir = join(CONFORMANCE, 'osv');
  // SYNT-* are the corpus's synthetic rule records, not OSV data: the truth's
  // findings come from OSV alone, so the mirror serves OSV records only.
  const records = readdirSync(osvDir)
    .filter((f) => f.endsWith('.json') && !f.startsWith('SYNT-'))
    .sort()
    .map((f) => ({ name: f, json: readJson(join(osvDir, f)) }));
  const files = {};
  for (const [eco, recs] of Object.entries(groupByEcosystem(records))) {
    const zip = buildZip(recs.map((r) => ({ name: r.name, data: Buffer.from(JSON.stringify(r.json)) })));
    mkdirSync(join(dir, eco), { recursive: true });
    writeFileSync(join(dir, eco, 'all.zip'), zip);
    files[`/${eco}/all.zip`] = { bytes: zip, etag: etagOf(zip) };
  }
  return { files, records: records.length };
}

function startMirror(files) {
  const requests = [];
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    requests.push(`${req.method} ${path}`);
    const f = files[path];
    if (!f || (req.method !== 'GET' && req.method !== 'HEAD')) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, {
      'content-type': 'application/zip',
      'content-length': f.bytes.length,
      etag: f.etag,
      'last-modified': 'Thu, 01 Oct 2026 00:00:00 GMT',
    });
    res.end(req.method === 'HEAD' ? undefined : f.bytes);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, port: server.address().port, requests })));
}

function writeProfile(dataDir, ctxDir) {
  const settings = readJson(join(REPO, 'data', 'settings.example.json'));
  settings.llm = { ...settings.llm, provider: 'none', api_key: '', openai_api_key: '' };
  settings.rerank = { ...settings.rerank, enabled: false };
  settings.llm_limits = { daily_token_limit: 0, daily_cost_limit_cents: 0 };
  settings.context_dirs = [ctxDir];
  settings.onboarding_complete = true;
  settings.auto_discovery_completed = true;
  mkdirSync(dataDir, { recursive: true });
  writeFileSync(join(dataDir, 'settings.json'), JSON.stringify(settings, null, 2));
}

function copyCases(cases, ctxDir) {
  for (const id of cases) {
    cpSync(join(CONFORMANCE, 'cases', id), join(ctxDir, id), {
      recursive: true,
      filter: (src) => !src.endsWith('case.json'),
    });
  }
}

// --- one engine run ------------------------------------------------------------------

/** Peak resident memory of a live process in bytes, or null where it cannot be read. */
function peakRss(pid) {
  try {
    if (process.platform === 'linux') {
      const m = /VmHWM:\s+(\d+) kB/.exec(readFileSync(`/proc/${pid}/status`, 'utf8'));
      return m ? Number(m[1]) * 1024 : null;
    }
    if (process.platform === 'win32') {
      const out = execFileSync('powershell', ['-NoProfile', '-Command', `(Get-Process -Id ${pid}).PeakWorkingSet64`], {
        encoding: 'utf8',
        windowsHide: true,
        timeout: 10000,
      });
      return Number(out.trim()) || null;
    }
  } catch {
    /* the process exited between samples */
  }
  return null;
}

function runEngine(engine, env, logFile) {
  return new Promise((ok) => {
    const started = process.hrtime.bigint();
    const child = spawn(engine, ['--once', '--force'], { env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    const chunks = [];
    child.stdout.on('data', (d) => chunks.push(d));
    child.stderr.on('data', (d) => chunks.push(d));
    let peak = null;
    const sample = () => {
      const v = peakRss(child.pid);
      if (v != null) peak = Math.max(peak ?? 0, v);
    };
    const timer = setInterval(sample, process.platform === 'win32' ? 2000 : 250);
    child.on('exit', (code, signal) => {
      clearInterval(timer);
      const out = Buffer.concat(chunks).toString('utf8');
      writeFileSync(logFile, out);
      ok({ code: code ?? (signal ? 128 : -1), signal, ms: Number(process.hrtime.bigint() - started) / 1e6, peakBytes: peak, out });
    });
  });
}

// --- reading the result (read-only) -----------------------------------------------------

async function readDb(dbPath) {
  const emit = process.emitWarning;
  process.emitWarning = (w, ...rest) => (/SQLite/.test(String(w)) ? undefined : emit.call(process, w, ...rest));
  const { DatabaseSync } = await import('node:sqlite');
  process.emitWarning = emit;
  const db = new DatabaseSync(dbPath, { readOnly: true });
  try {
    const kv = (key) => {
      const row = db.prepare('SELECT value FROM kv_store WHERE key = ?').get(key);
      return row ? JSON.parse(row.value) : null;
    };
    return {
      instances: db.prepare('SELECT project_path, ecosystem, package_name, version FROM dependency_instances').all(),
      advisories: db.prepare('SELECT advisory_id, aliases, package_name, ecosystem FROM osv_advisories').all(),
      sync: db.prepare('SELECT ecosystem, last_synced_at, advisory_count, error FROM osv_sync_status ORDER BY ecosystem').all(),
      report: kv('ace.lockfile_report.v1'),
      plan: kv('upgrade_plan_snapshot'),
      lastRun: db.prepare('SELECT ok, error FROM engine_runs ORDER BY id DESC LIMIT 1').get() || null,
    };
  } finally {
    db.close();
  }
}

/**
 * The coverage record, case-relative: every list the walk saved (what it read, skipped,
 * could not read, does not support, found unpinned or without a lockfile) with paths
 * relative to the context root, and every count. Lists the engine does not write are
 * absent, so an older or newer record shape is a golden difference, never a silent pass.
 */
function coverageRecord(report, ctxDir) {
  if (!report) return null;
  const rel = (p) => {
    const at = locate(p, ctxDir);
    return at ? `${at.caseId}/${at.dir}` : `<outside>${p}`;
  };
  const out = { counts: {} };
  for (const [k, v] of Object.entries(report).sort(([a], [b]) => a.localeCompare(b))) {
    if (Array.isArray(v)) {
      out[k] = v
        .map((e) => (e && typeof e === 'object' && 'path' in e ? { ...e, path: rel(e.path) } : e))
        .sort((a, b) => String(a?.path ?? JSON.stringify(a)).localeCompare(String(b?.path ?? JSON.stringify(b))));
    } else if (typeof v === 'number') out.counts[k] = v;
  }
  return out;
}

/** The case-relative directories a coverage record says were skipped, per case. */
function skippedDirsByCase(coverage) {
  const out = {};
  for (const s of coverage?.skipped_dirs || []) {
    const [caseId, ...rest] = s.path.split('/');
    (out[caseId] ||= []).push(rest.join('/'));
  }
  return out;
}

// --- assertions ------------------------------------------------------------------------

class Checks {
  constructor() {
    this.failures = [];
    this.passes = 0;
  }
  ok(cond, what, detail = '') {
    if (cond) this.passes++;
    else this.failures.push(detail ? `${what}: ${detail}` : what);
    return cond;
  }
}

const show = (xs, n = 8) => `${xs.slice(0, n).join(', ')}${xs.length > n ? ` (+${xs.length - n} more)` : ''}`;

function checkInventory(checks, label, db, cfg, ctxDir, coverage, expectedByCase) {
  const lower = Object.fromEntries(cfg.cases.map((c) => [c.toLowerCase(), c]));
  const { byCase, outside } = inventoryByCase(db.instances, ctxDir, lower);
  checks.ok(outside.length === 0, `${label}: every stored dependency belongs to a case`, show(outside));
  const skipped = skippedDirsByCase(coverage);
  for (const id of cfg.cases) {
    const want = expectedByCase[id] ?? expectedInventory(readJson(join(CONFORMANCE, 'expected', `${id}.json`)), skipped[id]);
    const d = diffSets(byCase[id] || [], want);
    checks.ok(
      d.missing.length === 0 && d.extra.length === 0,
      `${label}: inventory of ${id} equals the truth (${want.length} packages)`,
      `missing ${d.missing.length} [${show(d.missing)}], extra ${d.extra.length} [${show(d.extra)}]`,
    );
  }
  return byCase;
}

/** Every truth finding on a stored package has its advisory (or an alias) stored for that package. */
function checkFindings(checks, label, db, cfg, coverage, removedKeys = new Set()) {
  const stored = new Map();
  for (const a of db.advisories) {
    const eco = osvEcosystem(a.ecosystem);
    const k = `${eco}|${normName(eco, a.package_name)}`;
    if (!stored.has(k)) stored.set(k, new Set());
    const ids = stored.get(k);
    ids.add(a.advisory_id);
    for (const al of JSON.parse(a.aliases || '[]')) ids.add(al);
  }
  const skipped = skippedDirsByCase(coverage);
  for (const id of cfg.cases) {
    const want = expectedFindingKeys(readJson(join(CONFORMANCE, 'expected', `${id}.json`)), skipped[id]);
    const missing = [];
    let groups = 0;
    for (const [key, idGroups] of want) {
      if (removedKeys.has(`${id}:${key}`)) continue;
      const [eco, , name] = key.split('|');
      const have = stored.get(`${eco}|${name}`) || new Set();
      for (const g of idGroups) {
        groups++;
        if (![...g].some((x) => have.has(x))) missing.push(`${key} ${[...g][0]}`);
      }
    }
    checks.ok(missing.length === 0, `${label}: the ${groups} truth findings of ${id} have their advisory stored`, show(missing));
  }
}

/** Compare with a golden; on a mismatch the actual value is saved beside the logs (`actual-<name>`). */
function checkGolden(checks, label, actual, goldenPath, update, workDir) {
  if (update) {
    writeFileSync(goldenPath, `${JSON.stringify(actual, null, 2)}\n`);
    log(`golden written: ${goldenPath}`);
    return;
  }
  if (!checks.ok(existsSync(goldenPath), `${label}: golden ${goldenPath} exists`, 'run with --update-goldens')) return;
  const d = firstDifference(actual, readJson(goldenPath));
  if (d && workDir) writeFileSync(join(workDir, `actual-${goldenPath.split(/[\/]/).pop()}`), `${JSON.stringify(actual, null, 2)}
`);
  checks.ok(!d, `${label}: equals ${goldenPath.slice(REPO.length + 1)}`, d && `${d.path}: got ${JSON.stringify(d.actual)?.slice(0, 200)}, golden ${JSON.stringify(d.expected)?.slice(0, 200)}`);
}

function checkBudget(checks, label, run, budget) {
  checks.ok(run.ms <= budget.wall_seconds * 1000, `${label}: wall time ${(run.ms / 1000).toFixed(1)} s within ${budget.wall_seconds} s`);
  if (run.peakBytes == null) {
    checks.ok(process.platform !== 'linux', `${label}: peak memory measured`, 'VmHWM unreadable');
    log(`${label}: peak memory not measured on ${process.platform}`);
    return;
  }
  const mb = run.peakBytes / 1024 ** 2;
  checks.ok(mb <= budget.peak_rss_mb, `${label}: peak memory ${mb.toFixed(0)} MB within ${budget.peak_rss_mb} MB`);
}

function planSummary(plan, ctxDir) {
  if (!plan) return null;
  return normalizePlan(plan, ctxDir);
}

// --- main ----------------------------------------------------------------------------

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const cfg = readJson(join(GATE_DIR, 'gate.json'));
  const engine = resolve(opts.engine);
  if (!existsSync(engine)) throw new Error(`engine not found: ${engine}`);

  rmSync(opts.work, { recursive: true, force: true });
  const ctxDir = join(opts.work, 'ctx');
  const dataDir = join(opts.work, 'data');
  const mirrorDir = join(opts.work, 'mirror');
  copyCases(cfg.cases, ctxDir);
  writeProfile(dataDir, ctxDir);
  const mirror = buildMirror(mirrorDir);
  const { server, port, requests } = await startMirror(mirror.files);
  log(`work ${opts.work}; ${cfg.cases.length} cases; mirror of ${mirror.records} OSV records on 127.0.0.1:${port}`);

  const env = {
    ...process.env,
    FOURDA_DATA_DIR: dataDir,
    FOURDA_E2E_OSV_MIRROR: `http://127.0.0.1:${port}`,
    HTTPS_PROXY: DEAD_PROXY,
    HTTP_PROXY: DEAD_PROXY,
    ALL_PROXY: DEAD_PROXY,
    https_proxy: DEAD_PROXY,
    http_proxy: DEAD_PROXY,
    all_proxy: DEAD_PROXY,
    NO_PROXY: '127.0.0.1',
    no_proxy: '127.0.0.1',
    // Step 1 (news and registry feeds) only spends its retry budget with the network
    // blocked; the hook exists only in the e2e build (src-tauri/src/e2e_gate.rs).
    FOURDA_E2E_SKIP_FEED_FETCH: '1',
    FOURDA_IGNORE_CONTENTION: '1',
    RUST_LOG: process.env.RUST_LOG || 'info',
  };
  delete env.FOURDA_DB_PATH;

  const checks = new Checks();
  const timings = [];
  const run = async (label) => {
    const r = await runEngine(engine, env, join(opts.work, `${label}.log`));
    timings.push({ label, seconds: +(r.ms / 1000).toFixed(1), peak_mb: r.peakBytes == null ? null : Math.round(r.peakBytes / 1024 ** 2), exit: r.code });
    log(`${label}: exit ${r.code} in ${(r.ms / 1000).toFixed(1)} s, peak ${r.peakBytes == null ? 'n/a' : `${Math.round(r.peakBytes / 1024 ** 2)} MB`}`);
    return { r, db: await readDb(join(dataDir, '4da.db')) };
  };

  try {
    // Run 1: a fresh profile, mirror up.
    const one = await run('run1');
    checks.ok(one.r.code === 0, 'run1: engine exits 0', `exit ${one.r.code}; log ${join(opts.work, 'run1.log')}`);
    checkBudget(checks, 'run1', one.r, cfg.budgets.fresh);
    const cov1 = coverageRecord(one.db.report, ctxDir);
    checks.ok(cov1 != null, 'run1: the walk stored its coverage record');
    checkGolden(checks, 'run1 coverage record', cov1, join(GATE_DIR, 'golden', 'coverage.json'), opts.update, opts.work);
    checkInventory(checks, 'run1', one.db, cfg, ctxDir, cov1, {});
    checkFindings(checks, 'run1', one.db, cfg, cov1);
    const verified = one.db.sync.filter((s) => s.error == null && s.last_synced_at);
    checks.ok(verified.length === one.db.sync.length && verified.length > 0, `run1: every synced ecosystem verified against the mirror (${verified.map((s) => s.ecosystem).join(', ')})`, JSON.stringify(one.db.sync));
    checks.ok(one.db.plan != null, 'run1: the upgrade plan snapshot exists');
    checkGolden(checks, 'run1 plan', planSummary(one.db.plan, ctxDir), join(GATE_DIR, 'golden', 'plan-run1.json'), opts.update, opts.work);
    checks.ok(requests.some((q) => q.startsWith('GET ')), 'run1: the archives came from the frozen mirror', show(requests));

    // Run 2: one lockfile changed, as a user applying a fix does.
    const m = cfg.mutation;
    const file = join(ctxDir, m.case, m.file);
    const before = readFileSync(file, 'utf8');
    const occurrences = before.split(m.find).length - 1;
    if (occurrences !== m.occurrences) throw new Error(`mutation: expected ${m.occurrences} occurrence(s) of the find text in ${m.file}, found ${occurrences}`);
    writeFileSync(file, before.split(m.find).join(m.replace));
    // mtime resolution: the rescan compares modification times with the last scan.
    const later = new Date(Date.now() + 2000);
    utimesSync(file, later, later);
    const two = await run('run2');
    checks.ok(two.r.code === 0, 'run2: engine exits 0', `exit ${two.r.code}`);
    checkBudget(checks, 'run2', two.r, cfg.budgets.rerun);
    const cov2 = coverageRecord(two.db.report, ctxDir);
    const truth = expectedInventory(readJson(join(CONFORMANCE, 'expected', `${m.case}.json`)), skippedDirsByCase(cov2)[m.case]);
    checkInventory(checks, 'run2', two.db, cfg, ctxDir, cov2, { [m.case]: applyMutation(truth, m) });
    checkFindings(checks, 'run2', two.db, cfg, cov2, new Set((m.removed || []).map((k) => `${m.case}:${k}`)));
    checks.ok(two.db.sync.every((s) => s.error == null), 'run2: every ecosystem verified', JSON.stringify(two.db.sync));
    checkGolden(checks, 'run2 plan', planSummary(two.db.plan, ctxDir), join(GATE_DIR, 'golden', 'plan-run2.json'), opts.update, opts.work);

    // Run 3: the mirror is gone (offline, a dead proxy, a blocked host).
    await new Promise((ok) => server.close(ok));
    server.closeAllConnections?.();
    const three = await run('run3-offline');
    checks.ok(three.r.code === 1, 'run3: engine exits 1 when no advisory source could be checked', `exit ${three.r.code}`);
    checkBudget(checks, 'run3', three.r, cfg.budgets.rerun);
    const prior = Object.fromEntries(two.db.sync.map((s) => [s.ecosystem, s.last_synced_at]));
    for (const s of three.db.sync) {
      checks.ok(/could not check for newer advisories/.test(s.error || ''), `run3: ${s.ecosystem} records "not verified", not fresh`, `error ${JSON.stringify(s.error)}`);
      checks.ok(s.last_synced_at === prior[s.ecosystem], `run3: ${s.ecosystem} keeps its last verified time`, `${prior[s.ecosystem]} -> ${s.last_synced_at}`);
    }
    checks.ok(three.db.lastRun && three.db.lastRun.ok === 0 && /OSV sync/.test(three.db.lastRun.error || ''), 'run3: the engine_runs receipt is a failed run naming the OSV sync', JSON.stringify(three.db.lastRun));
    const inv2 = diffSets(three.db.instances.map((r) => `${r.project_path}|${r.ecosystem}|${r.package_name}|${r.version}`), two.db.instances.map((r) => `${r.project_path}|${r.ecosystem}|${r.package_name}|${r.version}`));
    checks.ok(inv2.missing.length === 0 && inv2.extra.length === 0, 'run3: inventory unchanged offline');
    checks.ok(three.db.plan != null, 'run3: the plan snapshot is still there');
    checkGolden(checks, 'run3 plan', planSummary(three.db.plan, ctxDir), join(GATE_DIR, 'golden', 'plan-run2.json'), false, opts.work);
  } finally {
    server.close();
  }

  const report = { passed: checks.passes, failed: checks.failures.length, timings, failures: checks.failures };
  writeFileSync(join(opts.work, 'report.json'), JSON.stringify(report, null, 2));
  console.log('');
  for (const t of timings) console.log(`  ${t.label.padEnd(13)} ${String(t.seconds).padStart(7)} s  peak ${t.peak_mb ?? 'n/a'} MB  exit ${t.exit}`);
  console.log(`\nengine e2e: ${checks.passes} checks passed, ${checks.failures.length} failed`);
  for (const f of checks.failures) console.log(`  FAIL ${f}`);
  if (process.env.GITHUB_STEP_SUMMARY) {
    const lines = ['### Engine end-to-end gate', '', '| run | seconds | peak MB | exit |', '|---|---:|---:|---:|'];
    for (const t of timings) lines.push(`| ${t.label} | ${t.seconds} | ${t.peak_mb ?? 'n/a'} | ${t.exit} |`);
    lines.push('', `${checks.passes} checks passed, ${checks.failures.length} failed.`, ...checks.failures.map((f) => `- FAIL ${f}`));
    writeFileSync(process.env.GITHUB_STEP_SUMMARY, `${lines.join('\n')}\n`, { flag: 'a' });
  }
  if (!opts.keep && checks.failures.length === 0 && !opts.update) rmSync(opts.work, { recursive: true, force: true });
  return checks.failures.length && !opts.update ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main()
    .then((code) => process.exit(code))
    .catch((e) => {
      console.error(`engine-e2e: ${e.stack || e.message}`);
      process.exit(2);
    });
}
