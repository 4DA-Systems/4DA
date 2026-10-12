// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Pure evaluators for the AD-054 launch gates (G1-G9), used by
// scripts/launch-gates.mjs. Every function takes plain data and returns
//   { id, name, status, summary, evidence: string[], details? }
// with status one of: pass | fail | pending | operator.
// Rule: missing or unreadable data is `pending` with the reason. Never `pass`.

export const STATUSES = ['pass', 'fail', 'pending', 'operator'];

const gate = (id, name, status, summary, evidence = [], details = undefined) => ({
  id,
  name,
  status,
  summary,
  evidence,
  ...(details ? { details } : {}),
});

const pct = (x, d = 1) => (x == null || !Number.isFinite(x) ? 'n/a' : `${(100 * x).toFixed(d)}%`);

/** Wilson score interval lower bound (95%). */
export function wilsonLower(successes, n, z = 1.96) {
  if (!n) return 0;
  const p = successes / n;
  const denom = 1 + (z * z) / n;
  const centre = p + (z * z) / (2 * n);
  const margin = z * Math.sqrt((p * (1 - p)) / n + (z * z) / (4 * n * n));
  return (centre - margin) / denom;
}

/** CI conclusion -> status contribution. */
function ciStatus(ci) {
  if (!ci || ci.error) return { ok: null, text: `CI not read (${ci?.error || 'gh unavailable'})` };
  if (ci.conclusion === 'success') return { ok: true, text: `CI ${ci.label}: success (${ci.when}, ${ci.sha})` };
  if (!ci.conclusion) return { ok: null, text: `CI ${ci.label}: still running (${ci.when})` };
  return { ok: false, text: `CI ${ci.label}: ${ci.conclusion} (${ci.when}, ${ci.sha})` };
}

// --- G1 conformance -------------------------------------------------------
export const G1_BAR = { precision: 0.995, recall: 0.99, silentDrops: 0, repos: 30, formats: 8 };

/**
 * thresholds: parsed thresholds.json; cases: [{source}] from case.json;
 * formats: distinct supported lockfile formats from expected/*.json; ci: rust job result.
 */
export function evaluateG1({ thresholds, cases, formats, ci }) {
  const name = 'G1 conformance';
  if (!thresholds) return gate('G1', name, 'pending', 'thresholds.json not found', []);
  const eco = Object.entries(thresholds.ecosystems || {});
  const below = eco.filter(
    ([, v]) => !(v.findings_precision >= G1_BAR.precision && v.findings_recall >= G1_BAR.recall),
  );
  const minP = Math.min(...eco.map(([, v]) => v.findings_precision));
  const minR = Math.min(...eco.map(([, v]) => v.findings_recall));
  const repos = (cases || []).filter((c) => c.source === 'repository').length;
  const nFormats = (formats || []).length;
  const drops = thresholds.max_silent_drops;
  const local = [
    `ratchet min findings precision ${pct(minP, 2)}, recall ${pct(minR, 2)} over ${eco.length} ecosystems (bar ${pct(G1_BAR.precision, 1)} / ${pct(G1_BAR.recall, 0)})`,
    `max_silent_drops ${drops} (bar 0)`,
    `${repos} repository cases (bar >= ${G1_BAR.repos}), ${(cases || []).length} total`,
    `${nFormats} supported lockfile formats (bar >= ${G1_BAR.formats})`,
  ];
  const localFail = below.length > 0 || drops !== 0 || repos < G1_BAR.repos || nFormats < G1_BAR.formats;
  const c = ciStatus(ci);
  const evidence = [
    'src-tauri/tests/conformance/thresholds.json',
    'src-tauri/tests/conformance/cases/*/case.json, expected/*.json',
    c.text,
  ];
  if (localFail) return gate('G1', name, 'fail', local.join('; '), evidence);
  if (c.ok === false) return gate('G1', name, 'fail', `${local.join('; ')}; but ${c.text}`, evidence);
  if (c.ok === null)
    return gate('G1', name, 'pending', `corpus meets the bar locally; ${c.text}`, evidence, { local });
  return gate('G1', name, 'pass', local.join('; '), evidence);
}

// --- G2 fix-path oracle ---------------------------------------------------
export function evaluateG2({ fixTargets, skip, ci }) {
  const name = 'G2 fix-path oracle';
  if (!fixTargets?.length) return gate('G2', name, 'pending', 'vectors/fix_target.jsonl not found or empty');
  const oracle = fixTargets.filter((v) => /fix-path oracle/.test(v.source || ''));
  const cleared = oracle.filter((v) => v.clears === true).length;
  const rate = oracle.length ? cleared / oracle.length : null;
  // A recommended target still affected by an advisory that HAS a fix.
  const stillVulnerable = fixTargets.filter(
    (v) => v.target && v.clears === true && (v.evidence?.osv_ids_at_target || []).length > 0,
  );
  const residualUnfixable = fixTargets.filter((v) => v.target && v.clears === false).map((v) => v.id);
  const skipped = ['rust', 'ts'].flatMap((eng) =>
    Object.keys(skip?.[eng] || {})
      .filter((id) => id.startsWith('ft-'))
      .map((id) => `${eng}:${id}`),
  );
  const c = ciStatus(ci);
  const summary =
    `${cleared}/${oracle.length} oracle actions clear with a minimal change (${pct(rate)}; bar 95%); ` +
    `${stillVulnerable.length} targets still affected by a fixable advisory (bar 0); ` +
    `${fixTargets.length} fix_target vectors, ${skipped.length} skipped` +
    (residualUnfixable.length ? `; ${residualUnfixable.length} targets keep an advisory with no fix (${residualUnfixable.join(', ')})` : '');
  const evidence = [
    'src-tauri/tests/conformance/vectors/fix_target.jsonl',
    'src-tauri/tests/conformance/vectors/engines_skip.json',
    c.text,
  ];
  if (rate == null) return gate('G2', name, 'pending', 'no fix-path oracle vectors', evidence);
  if (rate < 0.95 || stillVulnerable.length) return gate('G2', name, 'fail', summary, evidence);
  if (skipped.length) return gate('G2', name, 'pending', `${summary}; an engine skips fix_target vectors`, evidence);
  if (c.ok === false) return gate('G2', name, 'fail', `${summary}; ${c.text}`, evidence);
  if (c.ok === null) return gate('G2', name, 'pending', `${summary}; ${c.text}`, evidence);
  return gate('G2', name, 'pass', summary, evidence);
}

// --- G3 upgrade impact ----------------------------------------------------
export function evaluateG3({ mcp }) {
  const name = 'G3 upgrade impact';
  if (!mcp || !mcp.testFound)
    return gate('G3', name, 'pending', 'see MCP CI: the G3 panel test is not readable locally', [
      mcp?.where || 'MCP repo not found (set FOURDA_MCP_REPO)',
    ]);
  const bar = mcp.threshold;
  const evidence = [mcp.testPath, mcp.ci ? ciStatus(mcp.ci).text : 'MCP CI not read'];
  if (bar == null || bar < 0.9)
    return gate('G3', name, 'fail', `panel test asserts precision >= ${bar ?? '?'} (bar 0.90)`, evidence);
  const c = ciStatus(mcp.ci);
  const summary = `panel test asserts breaking-list precision >= ${bar} on all and held-out pairs`;
  if (c.ok === true) return gate('G3', name, 'pass', `${summary}; enforced by MCP CI`, evidence);
  if (c.ok === false) return gate('G3', name, 'fail', `${summary}; ${c.text}`, evidence);
  return gate('G3', name, 'pending', `${summary}; see MCP CI (${c.text})`, evidence);
}

// --- G4 Brief ---------------------------------------------------------------
export const G4_BAR = { days: 14, deliveredRate: 0.95 };

/**
 * One local day of G4. A day qualifies when a Brief went out (narrated summary or
 * facts view), the audit of what it showed recorded zero version faults, and both
 * caps held: the morning's act-now cap (items_shown <= cap, recorded) and the
 * daily auto-brief cap (auto briefs <= dailyCap). A field the record lacks is
 * unknown, and an unknown day does not qualify.
 *
 * o: the day's morning_brief_outcomes_v1 record; writes: its brief_writes_by_day_v1
 * entry {auto, manual} or undefined; briefingRows: briefings rows that local day.
 */
export function g4Day(day, o, { writes, briefingRows = 0, dailyCap } = {}) {
  const res = { day, delivered: false, qualifies: false, fault: false, capBreach: false, unknown: false, why: '' };
  if (!o) return { ...res, why: 'no morning recorded' };
  res.delivered = o.outcome === 'narrated' || o.outcome === 'facts';
  const autoBriefs = writes ? writes.auto ?? 0 : briefingRows;
  const autoSource = writes ? 'auto briefs' : 'briefings rows (auto and manual: no per-trigger count that day)';
  if (dailyCap != null && autoBriefs > dailyCap) {
    res.capBreach = true;
    res.why = `${autoSource} ${autoBriefs} > ${dailyCap}`;
  }
  if (Array.isArray(o.version_faults) && o.version_faults.length) {
    res.fault = true;
    res.why = [res.why, `version faults: ${o.version_faults.join('; ')}`].filter(Boolean).join('; ');
  }
  if (Number.isFinite(o.items_shown) && Number.isFinite(o.cap) && o.items_shown > o.cap) {
    res.capBreach = true;
    res.why = [res.why, `act-now named ${o.items_shown} > ${o.cap}`].filter(Boolean).join('; ');
  }
  if (res.fault || res.capBreach) return res;
  if (!res.delivered) return { ...res, why: `outcome ${o.outcome}${o.reason ? ` (${o.reason})` : ''}` };
  if (!Array.isArray(o.version_faults)) {
    return { ...res, unknown: true, why: o.audit_error ? `audit failed: ${o.audit_error}` : 'no version audit recorded (written before the audit existed)' };
  }
  if (!Number.isFinite(o.items_shown) || !Number.isFinite(o.cap)) return { ...res, unknown: true, why: 'no act-now cap recorded' };
  if (dailyCap == null) return { ...res, unknown: true, why: 'daily brief cap not read' };
  return { ...res, qualifies: true, why: 'qualifies' };
}

/**
 * outcomes: { 'YYYY-MM-DD': record } from kv_store morning_brief_outcomes_v1;
 * briefWrites: { day: {auto, manual} } from kv_store brief_writes_by_day_v1;
 * briefingRowsByDay: { day: n } briefings rows per local day; today: local 'YYYY-MM-DD';
 * dailyCap: DAILY_AUTO_BRIEF_CAP read from source.
 *
 * pass: the last 14 days (ending today, or yesterday while today has no record)
 * all qualify, and the delivered rate over that window is >= 95%.
 * fail: a version fault or a cap breach inside the window, or a full 14-day window
 * delivered below 95%. pending otherwise (with how far the streak has got).
 */
export function evaluateG4({ outcomes, briefWrites = {}, briefingRowsByDay = {}, today, dailyCap }) {
  const name = 'G4 Brief';
  const evidence = ['kv_store morning_brief_outcomes_v1', 'kv_store brief_writes_by_day_v1', 'briefings table'];
  const recorded = Object.keys(outcomes || {}).sort();
  if (!recorded.length)
    return gate('G4', name, 'pending', 'no morning outcomes recorded yet (kv_store morning_brief_outcomes_v1 empty)', evidence);
  const day = (d) => g4Day(d, outcomes[d], { writes: briefWrites[d], briefingRows: briefingRowsByDay[d] || 0, dailyCap });
  const end = outcomes[today] ? today : prevDay(today);

  let streak = 0;
  let breakDay = null;
  for (let d = end; ; d = prevDay(d)) {
    const r = day(d);
    if (!r.qualifies) {
      breakDay = r;
      break;
    }
    streak++;
  }
  // The window: the last 14 days to `end`, never before recording began.
  const window = [];
  for (let d = end; window.length < G4_BAR.days && d >= recorded[0]; d = prevDay(d)) window.unshift(day(d));
  const deliveredN = window.filter((r) => r.delivered).length;
  const rate = window.length ? deliveredN / window.length : null;
  const faults = window.filter((r) => r.fault);
  const breaches = window.filter((r) => r.capBreach);
  const unknown = window.filter((r) => r.unknown);

  const summary = [
    `${Math.min(streak, G4_BAR.days)}/${G4_BAR.days} consecutive qualifying mornings to ${end} (delivered as narrated or facts, 0 shown version faults, caps held)`,
    `delivered ${deliveredN}/${window.length} days in the window ${window[0]?.day}..${end} (${pct(rate)}; bar 95%)`,
    `version-fault days ${faults.length}${faults.length ? ` (${faults.map((r) => `${r.day}: ${r.why}`).join(' | ')})` : ''}`,
    `cap-breach days ${breaches.length}${breaches.length ? ` (${breaches.map((r) => `${r.day}: ${r.why}`).join(' | ')})` : ''}`,
    unknown.length ? `unknown days ${unknown.length} (${unknown.map((r) => r.day).join(', ')}: ${unknown[0].why})` : 'unknown days 0',
    `daily auto-brief cap ${dailyCap ?? 'not read'}`,
    breakDay && streak < G4_BAR.days ? `streak stops at ${breakDay.day}: ${breakDay.why}` : '',
  ]
    .filter(Boolean)
    .join('; ');
  const details = { streak, end, window };
  if (streak >= G4_BAR.days && rate >= G4_BAR.deliveredRate) return gate('G4', name, 'pass', summary, evidence, details);
  if (faults.length || breaches.length || (window.length >= G4_BAR.days && rate < G4_BAR.deliveredRate))
    return gate('G4', name, 'fail', summary, evidence, details);
  return gate('G4', name, 'pending', summary, evidence, details);
}

export function prevDay(ymd) {
  const d = new Date(`${ymd}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return d.toISOString().slice(0, 10);
}

// --- G5 Lane 1 --------------------------------------------------------------
export const G5_BAR = { lowerBound: 0.8, n: 80, staleDays: 14 };

/**
 * Pool weekly Lane 1 results into independent items: an item measured in several
 * weeks counts once, with its latest verdict. records: the weekly harness's
 * result lines ({measured_at, n, useful, items: [{key, useful}]}), any order.
 * A record without per-item keys cannot be pooled: the latest record stands alone.
 */
export function poolLane1(records) {
  const sorted = [...(records || [])].filter((r) => Number.isFinite(r?.n)).sort((a, b) => String(a.measured_at).localeCompare(String(b.measured_at)));
  const latest = sorted.at(-1) || null;
  if (!latest) return null;
  if (!sorted.every((r) => Array.isArray(r.items))) return { latest, n: latest.n, useful: latest.useful, runs: 1, pooled: false };
  const verdict = new Map();
  for (const r of sorted) for (const it of r.items) verdict.set(it.key, !!it.useful);
  return { latest, n: verdict.size, useful: [...verdict.values()].filter(Boolean).length, runs: sorted.length, pooled: true };
}

/**
 * lane1Records: parsed lines of data/launch-gates/lane1-weekly.jsonl (written by the
 * weekly re-measure) or a --lane1 file; source: where they came from; today: local date.
 * pass: Wilson 95% lower bound >= 0.80 over >= 80 independent items, latest week fresh.
 */
export function evaluateG5({ lane1Records, source, today }) {
  const name = 'G5 Lane 1';
  const evidence = [source || 'data/launch-gates/lane1-weekly.jsonl'];
  const pool = poolLane1(lane1Records);
  if (!pool)
    return gate('G5', name, 'pending', `no weekly Lane 1 re-measure result is stored (${evidence[0]}; the weekly harness writes it)`, evidence);
  const { latest } = pool;
  const lb = wilsonLower(pool.useful, pool.n);
  const latestLb = latest.wilson_lower ?? wilsonLower(latest.useful, latest.n);
  const ageDays = today && latest.measured_at ? Math.floor((Date.parse(`${today}T00:00:00Z`) - Date.parse(String(latest.measured_at).slice(0, 10) + 'T00:00:00Z')) / 86400000) : null;
  const summary =
    `latest week ${latest.useful}/${latest.n} useful (${latest.run || 'run'}, measured ${latest.measured_at}${latest.method ? '' : ', method not recorded'}; Wilson LB ${latestLb.toFixed(3)}); ` +
    `${pool.pooled ? `pooled over ${pool.runs} week(s): ${pool.useful}/${pool.n} independent items` : `${pool.n} items (latest week alone: no per-item keys to pool)`}, ` +
    `Wilson 95% lower bound ${lb.toFixed(3)} (bar >= ${G5_BAR.lowerBound} at n >= ${G5_BAR.n})`;
  if (ageDays != null && ageDays > G5_BAR.staleDays)
    return gate('G5', name, 'pending', `${summary}; latest result is ${ageDays} days old (the harness runs weekly)`, evidence);
  if (pool.n < G5_BAR.n) return gate('G5', name, 'pending', `${summary}; n ${pool.n} below ${G5_BAR.n}`, evidence);
  return gate('G5', name, lb >= G5_BAR.lowerBound ? 'pass' : 'fail', summary, evidence);
}

// --- G6 resources -----------------------------------------------------------
export const G6_IDLE_BAR_PCT = 2;

/**
 * db: output of collectDbFootprint (or null); idle: summary from the idle sampler
 * (or null); cold: parsed startup timings (or null).
 */
export function evaluateG6({ db, idle, cold }) {
  const name = 'G6 resources';
  const parts = [];
  const evidence = [];
  const sub = {};

  if (db) {
    sub.dbSize = 'pending';
    const proj = (p) =>
      `~${mb(p.bytesLow)}-${mb(p.bytesHigh)} at ${p.retentionDays} days (${p.itemsLow}-${p.itemsHigh} items)`;
    const projections = [db.projectionDefault, db.projection]
      .filter(Boolean)
      .filter((p, i, a) => a.findIndex((q) => q.retentionDays === p.retentionDays) === i);
    parts.push(
      `DB ${mb(db.fileBytes)} + WAL ${mb(db.walBytes)} (freelist ${mb(db.freelistBytes)}), ${db.items} items, ${db.disabledItems} from turned-off sources (pruned by last_seen retention, last seen ${db.disabledLastSeen || 'n/a'})` +
        (projections.length
          ? `; projected steady state ${projections.map(proj).join(', ')}${db.sizeIsUpperBound ? ' (upper bound: dbstat unavailable)' : ''}; no numeric size budget is defined`
          : '') +
        (db.vecChunks
          ? `; source_vec ${db.vecChunks.chunks} chunks (~3 MiB each), ${db.vecChunks.pinned} still hold a turned-on item inside retention (a chunk is freed only when empty)`
          : ''),
    );
    evidence.push(`${db.path} (read-only)`);
  } else {
    sub.dbSize = 'pending';
    parts.push('DB not read');
  }

  if (idle) {
    const appPct = idle.appPctCore;
    sub.idle = appPct == null ? 'pending' : appPct + (idle.enginePctCore || 0) < G6_IDLE_BAR_PCT ? 'pass' : 'fail';
    parts.push(
      `idle CPU ${appPct == null ? 'n/a (no idle window)' : `${appPct.toFixed(2)}%`} of a core for the GUI tree + ${(idle.enginePctCore ?? 0).toFixed(2)}% engine over ${idle.wallSec}s idle (bar < ${G6_IDLE_BAR_PCT}%); GPU SM ${idle.gpuSmMean == null ? 'n/a' : `${idle.gpuSmMean.toFixed(1)}%`}` +
        (idle.cycle?.wallSec
          ? `; in-cycle (reported apart) GUI ${(idle.cycle.appPctCore ?? 0).toFixed(1)}% + engine ${(idle.cycle.enginePctCore ?? 0).toFixed(1)}% over ${idle.cycle.wallSec}s`
          : ''),
    );
    evidence.push('idle sampler (Win32_Process CPU time of the GUI tree + fourda-engine, nvidia-smi pmon; idle = getAnimations()==0 and engine < 1% for >= 120 s)');
  } else {
    sub.idle = 'pending';
    parts.push('idle CPU not measured (run with --measure-idle)');
  }

  if (cold) {
    const over = cold.segments.filter((s) => s.elapsedMs >= s.budgetMs);
    const phase0Over = cold.phase0 && cold.phase0.elapsedMs >= cold.phase0.budgetMs;
    const releaseNote = cold.debug ? 'debug build: release budgets (Phase 0 5 s, pre_tauri 3 s, tauri_build 5 s) unmeasured' : '';
    sub.coldStart = over.length || phase0Over ? 'fail' : cold.debug ? 'pending' : 'pass';
    parts.push(
      `cold start ${cold.at}: Phase 0 ${cold.phase0 ? `${(cold.phase0.elapsedMs / 1000).toFixed(1)} s / ${cold.phase0.budgetMs / 1000} s` : 'n/a'}; ` +
        cold.segments.map((s) => `${s.name} ${(s.elapsedMs / 1000).toFixed(1)}/${s.budgetMs / 1000} s`).join(', ') +
        (releaseNote ? `; ${releaseNote}` : ''),
    );
    evidence.push(cold.source);
  } else {
    sub.coldStart = 'pending';
    parts.push('no GUI startup found in the logs');
  }

  const vals = Object.values(sub);
  const status = vals.includes('fail') ? 'fail' : vals.every((v) => v === 'pass') ? 'pass' : 'pending';
  return gate('G6', name, status, parts.join('; '), evidence, sub);
}

export function mb(bytes) {
  if (bytes == null) return 'n/a';
  return bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(2)} GiB` : `${(bytes / 1024 ** 2).toFixed(0)} MiB`;
}

/**
 * Steady-state projection at `retentionDays`: items still being written by
 * turned-ON sources over the window, at the corpus's measured bytes per item,
 * plus the non-item tables. Turned-off sources stop being re-seen, so the
 * last_seen retention prune removes their rows within `retentionDays`.
 */
export function projectSteadyState({ retentionDays, enabledCreated30, enabledSeen30, perennial, bytesPerItem, fixedBytes }) {
  const perDayLow = enabledCreated30 / 30;
  const perDayHigh = Math.max(enabledSeen30, enabledCreated30) / 30;
  const itemsLow = Math.round(perDayLow * retentionDays + perennial);
  const itemsHigh = Math.round(perDayHigh * retentionDays + perennial);
  return {
    retentionDays,
    itemsLow,
    itemsHigh,
    bytesLow: Math.round(fixedBytes + itemsLow * bytesPerItem),
    bytesHigh: Math.round(fixedBytes + itemsHigh * bytesPerItem),
  };
}

/** Parse the latest GUI cold start (watchdog lines) from log text. */
export function parseColdStart(logText, source = 'log') {
  const lines = logText.split(/\r?\n/).map((l) => l.replace(/\x1b\[[0-9;]*m/g, ''));
  let last = -1;
  lines.forEach((l, i) => {
    if (/Phase 0 \(first-light\) (complete|EXCEEDED budget)/.test(l)) last = i;
  });
  if (last < 0) return null;
  const p0 = lines[last];
  const at = p0.slice(0, 19);
  const phase0 = {
    elapsedMs: Number(/elapsed_ms=(\d+)/.exec(p0)?.[1]),
    budgetMs: Number(/budget_s=(\d+)/.exec(p0)?.[1]) * 1000,
  };
  const segments = [];
  for (let i = last; i >= Math.max(0, last - 400); i--) {
    const m = /Startup segment (?:within|exceeded) budget segment="(\w+)" elapsed_ms=(\d+) budget_ms=(\d+)/.exec(lines[i]);
    if (m && !segments.some((s) => s.name === m[1])) segments.unshift({ name: m[1], elapsedMs: +m[2], budgetMs: +m[3] });
    if (segments.length === 3) break;
  }
  const debug = phase0.budgetMs >= 45000 || lines.slice(Math.max(0, last - 400), last).some((l) => /target\\debug\\fourda\.exe|target\/debug\/fourda/.test(l));
  return { at, phase0, segments, debug, source };
}

// --- G7 / G8 / G9 -----------------------------------------------------------
export function evaluateG7() {
  return gate(
    'G7',
    'G7 agent eval',
    'pass',
    'measured 2026-10-12: 0/138 vulnerable-version picks on the @4da/mcp-server 6.2.0 release build (control 57.2%, 6.1.0 29.7%, p=3.6e-31); 138/138 completed',
    ['.claude/plans/PENDING-DECISION.md (6.2.0 publish decision, G7 line)'],
  );
}

export function evaluateG8({ signingConfigured }) {
  return gate(
    'G8',
    'G8 clean-VM installs, signed',
    'operator',
    `Windows installers ship unsigned (v1.0.3 unsigned waiver); signing ${signingConfigured ? 'step present in release.yml, secrets are the operator\'s' : 'not configured in release.yml'}; no clean-VM install record`,
    ['.github/workflows/release.yml', 'operator: Azure Artifact Signing account + secrets, clean-VM runs'],
  );
}

export const G9_DAYS = 7;

/**
 * Doctrine rule 10: seven consecutive days of founder use on the build, with zero
 * "this isn't right" issues filed (GitHub label `dogfood-defect`).
 * dogfood: scripts/launch-gates-dogfood.json ({label, windows: [{started, activated_at, build}]});
 * the last window is current. issues: [{number, title, state, createdAt}] carrying the
 * label, or {error}. now: epoch ms.
 * pass: 7 x 24 h since activation and no issue filed inside the window.
 * fail: an issue filed inside the window (a fixed build starts a new window).
 */
export function evaluateG9({ dogfood, issues, now = Date.now() }) {
  const name = 'G9 7-day dogfood';
  const w = dogfood?.windows?.at(-1);
  const label = dogfood?.label || 'dogfood-defect';
  const evidence = ['scripts/launch-gates-dogfood.json', `GitHub issues labelled ${label}`];
  if (!w) return gate('G9', name, 'pending', 'no dogfood window recorded (scripts/launch-gates-dogfood.json)', evidence);
  const start = Date.parse(w.activated_at || `${w.started}T00:00:00Z`);
  if (!Number.isFinite(start)) return gate('G9', name, 'pending', `dogfood window start unreadable (${w.activated_at || w.started})`, evidence);
  const end = start + G9_DAYS * 86400000;
  const days = Math.max(0, Math.floor((now - start) / 86400000));
  const head = `window ${w.started} on build ${w.build || '?'}${w.branch ? ` (${w.branch})` : ''}: day ${Math.min(days, G9_DAYS)}/${G9_DAYS}, ends ${new Date(end).toISOString().slice(0, 16)}Z`;
  if (!Array.isArray(issues))
    return gate('G9', name, 'pending', `${head}; ${label} issues not read (${issues?.error || 'gh unavailable'})`, evidence);
  const filed = issues.filter((i) => {
    const t = Date.parse(i.createdAt);
    return t >= start && t < end;
  });
  const after = issues.filter((i) => Date.parse(i.createdAt) >= end);
  const list = (xs) => xs.map((i) => `#${i.number} ${i.state.toLowerCase()} "${i.title}"`).join(', ');
  const tail = after.length ? `; ${after.length} filed after the window (${list(after)})` : '';
  if (filed.length)
    return gate('G9', name, 'fail', `${head}; ${filed.length} ${label} issue(s) filed in the window: ${list(filed)}; a fixed build starts a new window${tail}`, evidence);
  if (now < end) return gate('G9', name, 'pending', `${head}; 0 ${label} issues filed so far${tail}`, evidence);
  return gate('G9', name, 'pass', `${head}; 0 ${label} issues filed in the window${tail}`, evidence);
}

/** Render gates as an ASCII table (operator terminals render unicode as '?'). */
export function renderText(gates) {
  const out = [];
  for (const g of gates) {
    out.push(`${g.id.padEnd(3)} ${g.status.toUpperCase().padEnd(8)} ${g.name}`);
    out.push(`      ${g.summary}`);
    if (g.evidence.length) out.push(`      evidence: ${g.evidence.join(' | ')}`);
  }
  const count = (s) => gates.filter((g) => g.status === s).length;
  out.push('');
  out.push(`pass ${count('pass')}  fail ${count('fail')}  pending ${count('pending')}  operator ${count('operator')}`);
  return out.join('\n');
}
