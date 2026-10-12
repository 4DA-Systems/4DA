// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Tests for scripts/launch-gates.mjs: the evaluators, the read-only DB readers on a
// fixture database, the cold-start log parser and the idle-window classifier.
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';

import { collectDbFootprint, openReadOnly, readBrief, readCorpus, readRetention } from './launch-gates.mjs';
import {
  evaluateG1,
  evaluateG2,
  evaluateG3,
  evaluateG4,
  evaluateG5,
  evaluateG6,
  evaluateG9,
  parseColdStart,
  prevDay,
  projectSteadyState,
  wilsonLower,
} from './lib/launch-gates-core.mjs';
import { annotateQuiet, descendants, summarise } from './lib/launch-gates-idle.mjs';

const OK_CI = { label: 'rust', conclusion: 'success', when: 'today', sha: 'abc' };

let dir;
let dbPath;

before(async () => {
  dir = mkdtempSync(join(tmpdir(), 'launch-gates-'));
  dbPath = join(dir, '4da.db');
  const { DatabaseSync } = await import('node:sqlite');
  const db = new DatabaseSync(dbPath);
  db.exec(`
    CREATE TABLE kv_store (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT);
    CREATE TABLE briefings (id INTEGER PRIMARY KEY, content TEXT, created_at TEXT);
    CREATE TABLE sources (source_type TEXT UNIQUE, enabled INTEGER);
    CREATE TABLE source_items (id INTEGER PRIMARY KEY, source_type TEXT, created_at TEXT, last_seen TEXT, content TEXT);
    INSERT INTO sources VALUES ('crates_io', 1), ('hackernews', 0);
  `);
  const ins = db.prepare('INSERT INTO source_items (source_type, created_at, last_seen, content) VALUES (?, ?, ?, ?)');
  const ago = (d) => `datetime('now', '-${d} days')`;
  const at = (d) => db.prepare(`SELECT ${ago(d)} t`).get().t;
  // 30 stack items written in the last 30 days, 10 old stack items, 2 of them still re-seen.
  for (let i = 0; i < 30; i++) ins.run('crates_io', at(i), at(i), 'x'.repeat(200));
  for (let i = 0; i < 10; i++) ins.run('crates_io', at(60), at(i < 2 ? 1 : 50), 'x'.repeat(200));
  // 100 items from a turned-off interest source.
  for (let i = 0; i < 100; i++) ins.run('hackernews', at(5), at(5), 'y'.repeat(200));
  db.prepare('INSERT INTO kv_store VALUES (?, ?, ?)').run(
    'morning_brief_outcomes_v1',
    JSON.stringify({
      '2026-10-09': { outcome: 'failed', reason: 'x', path: 'scheduled', at: '' },
      '2026-10-10': { outcome: 'narrated', path: 'scheduled', at: '' },
      '2026-10-11': { outcome: 'facts', reason: 'failed', path: 'scheduled', at: '' },
    }),
    '',
  );
  db.prepare('INSERT INTO briefings (content, created_at) VALUES (?, ?)').run('b', '2026-10-10 21:00:00');
  db.close();
});

after(() => rmSync(dir, { recursive: true, force: true }));

describe('fixture DB (read-only)', () => {
  it('opens read-only: writes are refused', async () => {
    const db = await openReadOnly(dbPath);
    assert.throws(() => db.prepare("INSERT INTO sources VALUES ('npm', 1)").run(), /readonly/i);
    db.close();
  });

  it('counts items, turned-off rows and projects the steady state', async () => {
    const db = await openReadOnly(dbPath);
    const f = collectDbFootprint(db, dbPath, { retentionDays: 90, dbstat: false });
    db.close();
    assert.equal(f.items, 140);
    assert.equal(f.disabledItems, 100);
    assert.equal(f.sizeIsUpperBound, true, 'without dbstat the size is an upper bound');
    // 30 created in 30 days -> 1/day; 2 perennial rows; 90 days -> 92 items.
    assert.equal(f.projection.itemsLow, 92);
    assert.equal(f.projectionDefault.retentionDays, 30);
    assert.ok(f.projection.bytesHigh >= f.projection.bytesLow);
  });

  it('reads morning outcomes and briefing days', async () => {
    const db = await openReadOnly(dbPath);
    const b = readBrief(db, 600);
    db.close();
    assert.deepEqual(Object.keys(b.outcomes), ['2026-10-09', '2026-10-10', '2026-10-11']);
    assert.deepEqual(b.briefingDays, ['2026-10-11'], 'UTC 21:00 is the next local day at UTC+10');
    const g4 = evaluateG4({ ...b, today: '2026-10-12', capConstant: 6 });
    assert.equal(g4.status, 'pending');
    assert.equal(g4.details.streak, 2, 'the failed 10-09 ends the streak; today unrecorded counts from yesterday');
    assert.match(g4.summary, /version faults: not recorded/);
  });
});

describe('evaluators never pass on missing data', () => {
  it('G1-G6 and G9 are pending without inputs', () => {
    assert.equal(evaluateG1({}).status, 'pending');
    assert.equal(evaluateG2({}).status, 'pending');
    assert.equal(evaluateG3({}).status, 'pending');
    assert.equal(evaluateG4({ outcomes: {}, today: '2026-10-12' }).status, 'pending');
    assert.equal(evaluateG5({}).status, 'pending');
    assert.equal(evaluateG6({ db: null, idle: null, cold: null }).status, 'pending');
    assert.equal(evaluateG9({ today: '2026-10-12' }).status, 'pending');
  });

  it('a passing corpus without CI is pending, with CI it passes, a red CI fails', () => {
    const corpus = readCorpus();
    assert.equal(evaluateG1({ ...corpus, ci: { error: 'gh unavailable' } }).status, 'pending');
    assert.equal(evaluateG1({ ...corpus, ci: OK_CI }).status, 'pass');
    assert.equal(evaluateG1({ ...corpus, ci: { ...OK_CI, conclusion: 'failure' } }).status, 'fail');
  });
});

describe('G1 / G2 on the real corpus files', () => {
  it('reads >= 30 repository cases and >= 8 supported formats', () => {
    const { cases, formats } = readCorpus();
    assert.ok(cases.filter((c) => c.source === 'repository').length >= 30);
    assert.ok(formats.length >= 8);
  });

  it('G1 fails when a ratchet falls below the bar', () => {
    const corpus = readCorpus();
    const thresholds = structuredClone(corpus.thresholds);
    thresholds.ecosystems.npm.findings_recall = 0.98;
    assert.equal(evaluateG1({ ...corpus, thresholds, ci: OK_CI }).status, 'fail');
  });

  it('G2 counts oracle clears and fails on a still-vulnerable target', () => {
    const v = (id, clears, atTarget = []) => ({
      id,
      source: 'fix-path oracle 2026-10-10',
      target: '1.0.0',
      clears,
      evidence: { osv_ids_at_target: atTarget },
    });
    const good = Array.from({ length: 20 }, (_, i) => v(`ft-${i}`, true));
    assert.equal(evaluateG2({ fixTargets: good, skip: {}, ci: OK_CI }).status, 'pass');
    assert.equal(evaluateG2({ fixTargets: [...good, v('bad', true, ['GHSA-x'])], skip: {}, ci: OK_CI }).status, 'fail');
    const lowRate = [...good.slice(0, 10), v('a', false), v('b', false)];
    assert.equal(evaluateG2({ fixTargets: lowRate, skip: {}, ci: OK_CI }).status, 'fail');
    assert.equal(
      evaluateG2({ fixTargets: good, skip: { rust: { 'ft-1': 'todo' } }, ci: OK_CI }).status,
      'pending',
      'a skipped fix_target vector is not proven',
    );
  });
});

describe('G5 Wilson bound', () => {
  it('matches the textbook value and enforces n >= 80', () => {
    assert.ok(Math.abs(wilsonLower(49, 50) - 0.8950) < 0.001);
    assert.equal(evaluateG5({ lane1: { successes: 49, n: 50 } }).status, 'pending');
    assert.equal(evaluateG5({ lane1: { successes: 78, n: 80 } }).status, 'pass');
    assert.equal(evaluateG5({ lane1: { successes: 64, n: 80 } }).status, 'fail');
  });
});

describe('G6', () => {
  const LOG = [
    '2026-10-12T02:38:02.791546Z  INFO 4da::watchdog: Startup watchdog initialized prev_crashed=false',
    '2026-10-12T02:38:05.305775Z  INFO 4da::integrity: Verifying binary integrity path=D:\\4DA\\src-tauri\\target\\debug\\fourda.exe',
    '2026-10-12T02:38:10.260417Z  INFO 4da::watchdog: Startup segment within budget segment="pre_tauri" elapsed_ms=4321 budget_ms=8000',
    '2026-10-12T02:38:10.260754Z  INFO 4da::watchdog: Startup segment within budget segment="tauri_build" elapsed_ms=2952 budget_ms=12000',
    '2026-10-12T02:38:10.261123Z  INFO 4da::watchdog: Startup segment within budget segment="setup_app" elapsed_ms=195 budget_ms=3000',
    '2026-10-12T02:38:11.005020Z  INFO 4da::watchdog: Phase 0 (first-light) complete elapsed_ms=8212 budget_s=45',
  ].join('\n');

  it('parses the latest cold start and flags a debug build', () => {
    const c = parseColdStart(LOG, 'fixture');
    assert.equal(c.phase0.elapsedMs, 8212);
    assert.equal(c.phase0.budgetMs, 45000);
    assert.deepEqual(c.segments.map((s) => s.name), ['pre_tauri', 'tauri_build', 'setup_app']);
    assert.equal(c.debug, true);
    assert.equal(parseColdStart('nothing here'), null);
  });

  it('fails on idle CPU over 2% of a core, never passes on a debug cold start', () => {
    const cold = parseColdStart(LOG, 'fixture');
    const db = { fileBytes: 1, walBytes: 0, freelistBytes: 0, items: 1, disabledItems: 0, path: 'x' };
    const busy = evaluateG6({ db, cold, idle: { appPctCore: 5.2, enginePctCore: 0.1, wallSec: 600, gpuSmMean: 0 } });
    assert.equal(busy.status, 'fail');
    assert.equal(busy.details.idle, 'fail');
    const quiet = evaluateG6({ db, cold, idle: { appPctCore: 0.4, enginePctCore: 0.1, wallSec: 600, gpuSmMean: 0 } });
    assert.equal(quiet.details.idle, 'pass');
    assert.equal(quiet.details.coldStart, 'pending', 'release budgets are unmeasured on a debug build');
    assert.equal(quiet.status, 'pending');
  });

  it('projects steady state from the turned-on inflow', () => {
    const p = projectSteadyState({ retentionDays: 90, enabledCreated30: 1072, enabledSeen30: 1449, perennial: 60, bytesPerItem: 9000, fixedBytes: 190e6 });
    assert.equal(p.itemsLow, 3276);
    assert.equal(p.itemsHigh, 4407);
  });

  it('clamps retention like MonitoringConfig::retention_days', () => {
    const s = join(dir, 'settings.json');
    writeFileSync(s, JSON.stringify({ monitoring: { cleanup_max_age_days: 0 } }));
    assert.equal(readRetention(s).days, 7);
    writeFileSync(s, JSON.stringify({ monitoring: { cleanup_max_age_days: null } }));
    assert.equal(readRetention(s).days, 30);
    assert.equal(readRetention(join(dir, 'missing.json')).days, 30);
  });
});

describe('idle sampler classification', () => {
  const s = (t, anim, app) => ({ t: t * 1000, anim, app100ns: app * 1e7, engine100ns: 0, ollama100ns: 0, gpuSm: 0 });

  it('counts an interval idle only after 120 s without animations', () => {
    // 0-60 s animating (cycle), quiet from 70 s; idle intervals start at 190 s.
    const samples = [];
    let cpu = 0;
    for (let t = 0; t <= 400; t += 10) {
      cpu += t < 70 ? 5 : 0.1; // 50% in the cycle, 1% idle
      samples.push(s(t, t < 70 ? 3 : 0, cpu));
    }
    const r = summarise(annotateQuiet(samples), 120);
    assert.equal(r.idle.wallSec, 210);
    assert.ok(Math.abs(r.idle.appPctCore - 1) < 1e-9);
    assert.ok(r.cycle.appPctCore > 1);
  });

  it('a working fourda-engine is a cycle even with no animation', () => {
    const samples = [];
    let app = 0;
    let eng = 0;
    for (let t = 0; t <= 600; t += 10) {
      app += 0.1;
      eng += t >= 300 && t < 400 ? 3 : 0; // engine at 30% of a core for 100 s
      samples.push({ ...s(t, 0, app), engine100ns: eng * 1e7 });
    }
    const r = summarise(annotateQuiet(samples), 120);
    assert.equal(r.idle.enginePctCore, 0, 'engine work never lands in idle');
    // 30 engine-seconds land in the 350 s that are not idle.
    assert.ok(Math.abs(r.cycle.enginePctCore - (100 * 30) / 350) < 1e-9);
    // quiet from 0 -> idle intervals 120..290; engine busy 290..390; quiet from 400 -> idle 520..600.
    assert.equal(r.idle.wallSec, 170 + 80);
  });

  it('an unreachable bridge is unknown, not idle', () => {
    const r = summarise([s(0, null, 0), s(10, null, 1)], 0);
    assert.equal(r.idle.wallSec, 0);
    assert.equal(r.unknown.wallSec, 10);
  });

  it('walks the whole process tree', () => {
    const procs = [
      { pid: 1, ppid: 0 },
      { pid: 2, ppid: 1 },
      { pid: 3, ppid: 2 },
      { pid: 4, ppid: 9 },
    ];
    assert.deepEqual([...descendants(procs, 1)].sort(), [1, 2, 3]);
  });

  it('prevDay crosses month ends', () => {
    assert.equal(prevDay('2026-11-01'), '2026-10-31');
  });
});
