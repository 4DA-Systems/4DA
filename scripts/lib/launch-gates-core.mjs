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
/**
 * outcomes: { 'YYYY-MM-DD': {outcome, reason, path, at} } from kv_store
 * morning_brief_outcomes_v1; briefingDays: ['YYYY-MM-DD'] (local) with a briefing row;
 * today: local 'YYYY-MM-DD'; capConstant: MORNING_ACT_NOW_SHOWN if read from source.
 */
export function evaluateG4({ outcomes, briefingDays = [], today, capConstant }) {
  const name = 'G4 Brief';
  const delivered = (o) => ['narrated', 'facts', 'items_only'].includes(o?.outcome);
  const days = Object.keys(outcomes || {}).sort();
  const evidence = ['kv_store morning_brief_outcomes_v1', 'briefings table'];
  if (!days.length)
    return gate('G4', name, 'pending', 'no morning outcomes recorded yet (kv_store morning_brief_outcomes_v1 empty)', evidence);
  // Consecutive delivered days ending today (or yesterday when today has no record yet).
  let streak = 0;
  let cursor = outcomes[today] ? today : prevDay(today);
  while (outcomes[cursor] && delivered(outcomes[cursor])) {
    streak++;
    cursor = prevDay(cursor);
  }
  const deliveredCount = days.filter((d) => delivered(outcomes[d])).length;
  const rate = deliveredCount / days.length;
  const byOutcome = {};
  for (const d of days) byOutcome[outcomes[d].outcome] = (byOutcome[outcomes[d].outcome] || 0) + 1;
  const summary =
    `${streak}/14 consecutive delivered mornings; delivered ${deliveredCount}/${days.length} recorded days (${pct(rate)}; bar 95%) since ${days[0]}; ` +
    `outcomes ${JSON.stringify(byOutcome)}; ${briefingDays.length} local days with a briefing row; ` +
    `cap: ${capConstant != null ? `${capConstant} act-now facts listed per morning (code constant)` : 'not read'}; ` +
    'version faults: not recorded in the DB (needs the fault audit)';
  if (days.length >= 14 && rate < 0.95) return gate('G4', name, 'fail', summary, evidence);
  return gate('G4', name, 'pending', summary, evidence, { streak, deliveredCount, recordedDays: days.length });
}

export function prevDay(ymd) {
  const d = new Date(`${ymd}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return d.toISOString().slice(0, 10);
}

// --- G5 Lane 1 --------------------------------------------------------------
export function evaluateG5({ lane1 }) {
  const name = 'G5 Lane 1';
  if (!lane1)
    return gate('G5', name, 'pending', 'no weekly Lane 1 re-measure result is stored (pass --lane1 <result.json> with {successes, n, measured_at})', []);
  const lb = wilsonLower(lane1.successes, lane1.n);
  const summary = `${lane1.successes}/${lane1.n} correct (measured ${lane1.measured_at || '?'}); Wilson lower bound ${lb.toFixed(3)} (bar >= 0.80 at n >= 80)`;
  const evidence = [lane1.source || 'lane1 result file'];
  if (lane1.n < 80) return gate('G5', name, 'pending', `${summary}; n below 80`, evidence);
  return gate('G5', name, lb >= 0.8 ? 'pass' : 'fail', summary, evidence);
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
    evidence.push('idle sampler (Win32_Process CPU time, nvidia-smi pmon, getAnimations()==0 for >= 120 s)');
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

export function evaluateG9({ dogfoodStart, today }) {
  if (!dogfoodStart)
    return gate('G9', 'G9 7-day dogfood', 'pending', 'starts on the final build; no start date recorded (pass --dogfood-start YYYY-MM-DD)', []);
  const days = Math.floor((Date.parse(`${today}T00:00:00Z`) - Date.parse(`${dogfoodStart}T00:00:00Z`)) / 86400000);
  return gate(
    'G9',
    'G9 7-day dogfood',
    days >= 7 ? 'operator' : 'pending',
    `dogfood started ${dogfoodStart}: day ${days}/7${days >= 7 ? '; the founder confirms zero "this isn\'t right" issues' : ''}`,
    ['--dogfood-start'],
  );
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
