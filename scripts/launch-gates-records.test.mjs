// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Fixture tests for the stored-record gates of scripts/launch-gates.mjs:
// G4 (morning outcomes + version audit + caps), G5 (weekly Lane 1 results),
// G9 (dogfood window + dogfood-defect issues).
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, it } from 'node:test';

import { readDogfood, readLane1 } from './launch-gates.mjs';
import { evaluateG4, evaluateG5, evaluateG9, g4Day, poolLane1, prevDay } from './lib/launch-gates-core.mjs';

// ---- G4 -------------------------------------------------------------------------
const clean = (outcome = 'facts') => ({
  outcome,
  path: 'scheduled',
  at: '',
  versions_shown: ['hono 4.13.5'],
  version_faults: [],
  items_shown: 2,
  cap: 6,
});

/** n consecutive days ending at `end`, each from `make(day, i)` (i = 0 is `end`). */
function days(end, n, make) {
  const out = {};
  for (let i = 0, d = end; i < n; i++, d = prevDay(d)) out[d] = make(d, i);
  return out;
}

const DAILY_CAP = 3;

describe('G4 per-day rule', () => {
  it('a delivered, audited, capped morning qualifies; narrated or facts only', () => {
    assert.equal(g4Day('d', clean('facts'), { dailyCap: DAILY_CAP }).qualifies, true);
    assert.equal(g4Day('d', clean('narrated'), { dailyCap: DAILY_CAP }).qualifies, true);
    const items = g4Day('d', clean('items_only'), { dailyCap: DAILY_CAP });
    assert.equal(items.qualifies, false);
    assert.equal(items.delivered, false, 'feed items without a Brief are not a delivered Brief');
    assert.equal(g4Day('d', undefined, { dailyCap: DAILY_CAP }).why, 'no morning recorded');
  });

  it('a record without the audit fields is unknown, never a pass', () => {
    const old = { outcome: 'facts', path: 'scheduled', at: '' };
    const r = g4Day('d', old, { dailyCap: DAILY_CAP });
    assert.equal(r.qualifies, false);
    assert.equal(r.unknown, true);
    assert.equal(r.delivered, true, 'still counts toward the delivery rate');
    const failedAudit = g4Day('d', { ...old, audit_error: 'database unavailable' }, { dailyCap: DAILY_CAP });
    assert.match(failedAudit.why, /audit failed: database unavailable/);
    const noCap = g4Day('d', { ...clean(), items_shown: undefined }, { dailyCap: DAILY_CAP });
    assert.equal(noCap.unknown, true);
    assert.equal(g4Day('d', clean(), { dailyCap: null }).unknown, true, 'cap not read from source');
  });

  it('a shown version fault or a cap breach disqualifies the day', () => {
    const fault = g4Day('d', { ...clean(), version_faults: ['fix: hono 4.7.5'] }, { dailyCap: DAILY_CAP });
    assert.equal(fault.fault, true);
    assert.match(fault.why, /hono 4\.7\.5/);
    const actNow = g4Day('d', { ...clean(), items_shown: 7, cap: 6 }, { dailyCap: DAILY_CAP });
    assert.equal(actNow.capBreach, true);
    const auto = g4Day('d', clean(), { writes: { auto: 4, manual: 0 }, dailyCap: DAILY_CAP });
    assert.equal(auto.capBreach, true);
    assert.match(auto.why, /auto briefs 4 > 3/);
  });

  it('manual regenerations never count toward the cap; without a per-trigger count every row does', () => {
    assert.equal(g4Day('d', clean(), { writes: { auto: 3, manual: 5 }, briefingRows: 8, dailyCap: DAILY_CAP }).qualifies, true);
    const rows = g4Day('d', clean(), { briefingRows: 4, dailyCap: DAILY_CAP });
    assert.equal(rows.capBreach, true);
    assert.match(rows.why, /briefings rows/);
  });
});

describe('G4 streak, window and status', () => {
  const today = '2026-10-27';

  it('14 consecutive qualifying mornings pass', () => {
    const outcomes = days(today, 14, () => clean());
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'pass', g.summary);
    assert.equal(g.details.streak, 14);
  });

  it('counts from yesterday while today has no record yet', () => {
    const outcomes = days(prevDay(today), 14, () => clean());
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'pass');
    assert.equal(g.details.end, prevDay(today));
  });

  it('13 days is pending, with the streak and where it stops', () => {
    const outcomes = days(today, 13, () => clean());
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'pending');
    assert.equal(g.details.streak, 13);
    assert.match(g.summary, /13\/14 consecutive/);
    assert.match(g.summary, /streak stops at 2026-10-14: no morning recorded/);
  });

  it('an unknown day breaks the streak but is not a failure', () => {
    const outcomes = days(today, 14, (d, i) => (i === 3 ? { outcome: 'narrated', path: 's', at: '' } : clean()));
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'pending');
    assert.equal(g.details.streak, 3);
    assert.match(g.summary, /unknown days 1/);
  });

  it('a fault day inside the window fails', () => {
    const outcomes = days(today, 20, (d, i) => (i === 5 ? { ...clean(), version_faults: ['summary: hono 4.7.5'] } : clean()));
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'fail');
    assert.equal(g.details.streak, 5);
    assert.match(g.summary, /version-fault days 1 \(2026-10-22: version faults: summary: hono 4\.7\.5\)/);
  });

  it('a cap breach inside the window fails; one that has rolled out of it does not', () => {
    const breach = (at) =>
      evaluateG4({
        outcomes: days(today, 30, () => clean()),
        briefWrites: { [at]: { auto: 5, manual: 0 } },
        today,
        dailyCap: DAILY_CAP,
      });
    assert.equal(breach('2026-10-20').status, 'fail');
    const old = breach('2026-10-01');
    assert.equal(old.status, 'pass', old.summary);
  });

  it('the delivery rate fails a full window below 95%', () => {
    // Day 7 failed: 13/14 = 92.9% delivered, and the streak is 6.
    const outcomes = days(today, 14, (d, i) => (i === 6 ? { outcome: 'failed', reason: 'db', path: 's', at: '' } : clean()));
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'fail');
    assert.match(g.summary, /delivered 13\/14 days .*92\.9%/);
  });

  it('a short window below 95% is still pending (recording only just began)', () => {
    const outcomes = days(today, 3, (d, i) => (i === 2 ? { outcome: 'nothing_to_say', path: 's', at: '' } : clean()));
    const g = evaluateG4({ outcomes, today, dailyCap: DAILY_CAP });
    assert.equal(g.status, 'pending');
    assert.match(g.summary, /delivered 2\/3 days in the window 2026-10-25\.\.2026-10-27/);
  });
});

// ---- G5 -------------------------------------------------------------------------
const item = (key, useful = true) => ({ key, useful });
const weekly = (measured_at, items) => ({
  schema: 1,
  measured_at,
  method: 'panel+adjudicator',
  n: items.length,
  useful: items.filter((i) => i.useful).length,
  items,
});

describe('G5 weekly Lane 1', () => {
  it('pools independent items across weeks, latest verdict wins', () => {
    const w1 = weekly('2026-10-05T08:00Z', [item('a'), item('b', false), item('c')]);
    const w2 = weekly('2026-10-12T08:00Z', [item('b'), item('d')]);
    const p = poolLane1([w2, w1]);
    assert.equal(p.n, 4, 'b counted once');
    assert.equal(p.useful, 4, 'b re-labelled useful in the later week');
    assert.equal(p.latest.measured_at, '2026-10-12T08:00Z');
    assert.equal(p.runs, 2);
  });

  it('n below 80 is pending even at 100%', () => {
    const w = weekly('2026-10-12T23:19Z', Array.from({ length: 62 }, (_, i) => item(`k${i}`)));
    const g = evaluateG5({ lane1Records: [w], today: '2026-10-13' });
    assert.equal(g.status, 'pending');
    assert.match(g.summary, /62\/62 independent items/);
    assert.match(g.summary, /n 62 below 80/);
  });

  it('passes at LB >= 0.80 over >= 80 pooled items, fails below', () => {
    const w1 = weekly('2026-10-05', Array.from({ length: 50 }, (_, i) => item(`a${i}`)));
    const w2 = weekly('2026-10-12', Array.from({ length: 40 }, (_, i) => item(`b${i}`, i >= 2)));
    assert.equal(evaluateG5({ lane1Records: [w1, w2], today: '2026-10-13' }).status, 'pass');
    const bad = weekly('2026-10-12', Array.from({ length: 90 }, (_, i) => item(`c${i}`, i >= 20)));
    assert.equal(evaluateG5({ lane1Records: [bad], today: '2026-10-13' }).status, 'fail');
  });

  it('a stale latest result is pending, whatever it says', () => {
    const w = weekly('2026-09-01', Array.from({ length: 90 }, (_, i) => item(`k${i}`)));
    const g = evaluateG5({ lane1Records: [w], today: '2026-10-13' });
    assert.equal(g.status, 'pending');
    assert.match(g.summary, /42 days old/);
  });

  it('nothing stored is pending', () => {
    assert.equal(evaluateG5({ lane1Records: [], today: '2026-10-13' }).status, 'pending');
  });

  it('reads the harness jsonl and a legacy --lane1 json', () => {
    const dir = mkdtempSync(join(tmpdir(), 'lane1-'));
    try {
      const jl = join(dir, 'lane1-weekly.jsonl');
      writeFileSync(jl, `${JSON.stringify(weekly('2026-10-05', [item('a')]))}\n${JSON.stringify(weekly('2026-10-12', [item('b')]))}\n`);
      assert.equal(readLane1(jl).length, 2);
      const legacy = join(dir, 'r.json');
      writeFileSync(legacy, JSON.stringify({ successes: 78, n: 80, measured_at: '2026-10-12' }));
      const recs = readLane1(legacy);
      assert.equal(recs[0].useful, 78);
      assert.equal(evaluateG5({ lane1Records: recs, today: '2026-10-13' }).status, 'pass');
      assert.deepEqual(readLane1(join(dir, 'missing.jsonl')), []);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

// ---- G9 -------------------------------------------------------------------------
const dogfood = {
  label: 'dogfood-defect',
  windows: [{ started: '2026-10-13', activated_at: '2026-10-13T02:31:00+10:00', build: '322edca52', branch: 'main' }],
};
const START = Date.parse('2026-10-13T02:31:00+10:00');
const DAY = 86400000;
const issue = (number, createdAt, state = 'OPEN') => ({ number, title: `defect ${number}`, state, createdAt });

describe('G9 dogfood window', () => {
  it('is pending inside the 7 days with no issues, and passes after them', () => {
    const mid = evaluateG9({ dogfood, issues: [], now: START + 3.5 * DAY });
    assert.equal(mid.status, 'pending');
    assert.match(mid.summary, /day 3\/7/);
    assert.match(mid.summary, /build 322edca52/);
    assert.equal(evaluateG9({ dogfood, issues: [], now: START + 7 * DAY - 1 }).status, 'pending');
    assert.equal(evaluateG9({ dogfood, issues: [], now: START + 7 * DAY }).status, 'pass');
  });

  it('an issue filed in the window fails it, open or closed', () => {
    const at = new Date(START + 2 * DAY).toISOString();
    const g = evaluateG9({ dogfood, issues: [issue(950, at, 'CLOSED')], now: START + 8 * DAY });
    assert.equal(g.status, 'fail');
    assert.match(g.summary, /#950 closed "defect 950"/);
  });

  it('issues from before the window or after it do not fail it', () => {
    const before = new Date(START - DAY).toISOString();
    const after = new Date(START + 9 * DAY).toISOString();
    const g = evaluateG9({ dogfood, issues: [issue(1, before), issue(2, after)], now: START + 10 * DAY });
    assert.equal(g.status, 'pass');
    assert.match(g.summary, /1 filed after the window \(#2/);
  });

  it('the latest window is the current one', () => {
    const next = { ...dogfood, windows: [...dogfood.windows, { started: '2026-10-20', activated_at: '2026-10-20T09:00:00+10:00', build: 'abc' }] };
    const old = new Date(START + DAY).toISOString();
    const g = evaluateG9({ dogfood: next, issues: [issue(3, old)], now: Date.parse('2026-10-21T09:00:00+10:00') });
    assert.equal(g.status, 'pending', 'an issue from the previous window does not count');
    assert.match(g.summary, /build abc/);
  });

  it('unread issues or no window is pending, never pass', () => {
    assert.equal(evaluateG9({ dogfood, issues: { error: 'gh failed' }, now: START + 30 * DAY }).status, 'pending');
    assert.equal(evaluateG9({ dogfood: null, issues: [] }).status, 'pending');
  });

  it('the committed record starts 2026-10-13 on 322edca52', () => {
    const rec = JSON.parse(readFileSync(new URL('./launch-gates-dogfood.json', import.meta.url), 'utf8'));
    assert.equal(rec.label, 'dogfood-defect');
    assert.equal(rec.windows[0].started, '2026-10-13');
    assert.equal(rec.windows[0].build, '322edca52');
    assert.equal(Date.parse(rec.windows[0].activated_at), START);
    assert.equal(readDogfood(null, '2026-11-01').windows[0].started, '2026-11-01');
  });
});

