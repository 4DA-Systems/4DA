#!/usr/bin/env node
/**
 * Judge cohort monitor — the free half of judge verification.
 *
 * `bench:judge` measures the judge against labels and costs money, so it runs
 * on demand. This runs on the judgments the app has ALREADY stored, costs
 * nothing, and answers the question a benchmark cannot: is the live judge
 * still behaving like the one that was measured?
 *
 *   pnpm run monitor:judge
 *
 * Exit codes: 0 = no anomaly · 1 = finding(s) below · 2 = could not run
 * (constants unreadable, no database).
 *
 * What it looks for, and why each check exists:
 *
 *   1. EXACT-VALUE SPIKE in confidence. This is the tell that cost a week in
 *      August 2026. The demotion gate reads `confidence >= 0.7`; the judge
 *      quietly stopped emitting the field, `unwrap_or(0.5)` invented a value,
 *      and 93% of the v3 cohort sat on EXACTLY 0.5 — just under the gate, so
 *      nothing was ever demoted and nothing logged it. Two audits read the
 *      MEAN (0.522) and concluded "the model is unsure", which is the wrong
 *      story. A distribution does not pile a third of its mass on one value.
 *      The frequency is the tell, never the average.
 *
 *   2. OMISSION RATE. Since v5 an absent confidence is stored as 0.0 rather
 *      than fabricated, so a rising share of exact zeros means the judge is
 *      answering less often — the same gate-killing failure, now visible.
 *
 *   3. DEGENERATE REJECT RATE. A judge that accepts everything or rejects
 *      everything carries no information whatever its confidence looks like.
 *
 *   4. GATE REACH. Whether the current cohort actually contains anything the
 *      shipped demotion gate could act on, and how much more sits one notch
 *      looser. A gate matching nothing because the judge agrees with the feed
 *      and a gate calibrated past the judge's output distribution look
 *      identical from the outside; this separates them.
 *
 *   5. MERGED != RUNNING. The checks above run on the LIVE cohort — whatever
 *      prompt_version has the newest judged_at in the last 24h (plus the
 *      `drain_v1` drain cohort) — NOT on the version the Rust SOURCE declares.
 *      Until 2026-09-04 "live" meant "equals the source constant", so for the
 *      three days the built binary lagged the source (source said v6, every
 *      live row was v5) the running cohort was filed as "retired", every spike
 *      and omission check was skipped, and the monitor exited 0 on a judge it
 *      had not looked at. Now, when the source version has NO rows and the
 *      newest built binary is older than STALE_BINARY_HOURS, that is reported
 *      as its own finding: the merge happened, the deploy did not.
 *
 * Thresholds and the current prompt version are READ FROM THE RUST SOURCE, not
 * copied here — a monitor with its own copy of a constant reports on a gate the
 * product no longer runs.
 */

const fs = require('node:fs');
const path = require('node:path');
const Database = require('better-sqlite3');

const repoRoot = path.resolve(__dirname, '..');
const DEFAULT_DB_PATH = process.env.FOURDA_DB_PATH || path.join(repoRoot, 'data', '4da.db');
const JUDGE_SRC = path.join(repoRoot, 'src-tauri', 'src', 'llm_judgments.rs');
const DEFAULT_BIN_DIR = path.join(repoRoot, 'src-tauri', 'target', 'debug');

/** A built binary older than this with zero rows at the source version is a deploy gap, not a warm-up. */
const STALE_BINARY_HOURS = 6;
/** The window that defines the live cohort. */
const LIVE_WINDOW = '-1 day';
/** The always-live drain cohort (stale-score drain judgments carry this label). */
const DRAIN_COHORT = 'drain_v1';

// ── Read the shipped constants ────────────────────────────────────────────
class ConstantMissing extends Error {}

function constFromRust(source, name, pattern) {
  const m = source.match(pattern);
  if (!m) throw new ConstantMissing(name);
  return m[1];
}

/** Parse the shipped judge constants out of llm_judgments.rs source text. */
function parseShippedConstants(src) {
  return {
    promptVersion: constFromRust(src, 'PROMPT_VERSION', /const PROMPT_VERSION: &str = "([^"]+)"/),
    relevanceBelow: Number(
      constFromRust(src, 'DEMOTION_RELEVANCE_BELOW', /DEMOTION_RELEVANCE_BELOW: f64 = ([0-9.]+)/),
    ),
    confidenceMin: Number(
      constFromRust(src, 'DEMOTION_CONFIDENCE_MIN', /DEMOTION_CONFIDENCE_MIN: f64 = ([0-9.]+)/),
    ),
  };
}

// ── Binaries ──────────────────────────────────────────────────────────────
/**
 * The newest built app binary in `binDir` — `{ path, mtimeMs }` or null when
 * none exists. Both the GUI and the headless engine count: either one can be
 * the process writing judgments.
 */
function newestBinary(binDir) {
  const names = ['fourda.exe', 'fourda-engine.exe', 'fourda', 'fourda-engine'];
  let best = null;
  for (const name of names) {
    const p = path.join(binDir, name);
    let st;
    try {
      st = fs.statSync(p);
    } catch {
      continue;
    }
    if (!best || st.mtimeMs > best.mtimeMs) best = { path: p, mtimeMs: st.mtimeMs };
  }
  return best;
}

// ── Cohort classification ────────────────────────────────────────────────
/**
 * The versions the anomaly checks apply to: the prompt_version with the newest
 * judged_at inside LIVE_WINDOW, plus the drain cohort. Deliberately NOT the
 * source constant — see item 5 in the header.
 */
function liveCohortVersions(db) {
  const row = db
    .prepare(
      `SELECT prompt_version FROM llm_judgments
       WHERE judged_at >= datetime('now', ?) AND prompt_version != ?
       ORDER BY judged_at DESC LIMIT 1`,
    )
    .get(LIVE_WINDOW, DRAIN_COHORT);
  const live = new Set([DRAIN_COHORT]);
  if (row) live.add(row.prompt_version);
  return live;
}

/**
 * MERGED != RUNNING: the source declares `promptVersion`, but is anything
 * actually writing rows under it? Pure over its inputs so it can be tested.
 *
 * @returns {{ status: 'in_sync'|'warming'|'merged_not_running', sourceRows: number, message: string }}
 */
function deployDrift({ promptVersion, sourceRows, liveVersion, binary, now = Date.now() }) {
  if (sourceRows > 0) {
    return { status: 'in_sync', sourceRows, message: `${sourceRows} row(s) carry the source version ${promptVersion}` };
  }
  const liveLabel = liveVersion ?? 'none in 24h';
  if (!binary) {
    return {
      status: 'merged_not_running',
      sourceRows,
      message:
        `MERGED != RUNNING: source says ${promptVersion}, live rows are ${liveLabel}, ` +
        `no built binary found — nothing can be running the source version.`,
    };
  }
  const ageH = (now - binary.mtimeMs) / 3_600_000;
  const built = new Date(binary.mtimeMs).toISOString();
  if (ageH > STALE_BINARY_HOURS) {
    return {
      status: 'merged_not_running',
      sourceRows,
      message:
        `MERGED != RUNNING: source says ${promptVersion}, live rows are ${liveLabel}, ` +
        `binary built ${built} (${ageH.toFixed(1)}h ago, > ${STALE_BINARY_HOURS}h) — ` +
        `the running process is not the source you are reading.`,
    };
  }
  return {
    status: 'warming',
    sourceRows,
    message:
      `no rows at source version ${promptVersion} yet, but the binary was built ${built} ` +
      `(${ageH.toFixed(1)}h ago) — expected right after a rebuild; re-check after ${STALE_BINARY_HOURS}h.`,
  };
}

// ── The monitor ───────────────────────────────────────────────────────────
/**
 * Run every check against an open database.
 *
 * @param {object} opts
 * @param {import('better-sqlite3').Database} opts.db read-only handle
 * @param {{promptVersion: string, relevanceBelow: number, confidenceMin: number}} opts.constants
 * @param {{path: string, mtimeMs: number}|null} opts.binary newest built binary
 * @param {number} [opts.now]
 * @param {(line: string) => void} [opts.log]
 * @returns {{ exitCode: number, findings: string[], live: Set<string>, drift: object }}
 */
function run({ db, constants, binary, now = Date.now(), log = console.log }) {
  const { promptVersion: PROMPT_VERSION, relevanceBelow: RELEVANCE_BELOW, confidenceMin: CONFIDENCE_MIN } =
    constants;
  const findings = [];
  const pct = (n, d) => (d === 0 ? 0 : (100 * n) / d);

  const live = liveCohortVersions(db);
  const liveVersion = [...live].find((v) => v !== DRAIN_COHORT) ?? null;

  log(`  current prompt : ${PROMPT_VERSION}  (source)`);
  log(`  live cohort    : ${liveVersion ?? 'none in 24h'} + ${DRAIN_COHORT}  (newest judged_at, last 24h)`);
  log(`  shipped gate   : relevance < ${RELEVANCE_BELOW} AND confidence >= ${CONFIDENCE_MIN}\n`);

  // ── Cohorts ─────────────────────────────────────────────────────────────
  const cohorts = db
    .prepare(
      `SELECT prompt_version, model, COUNT(*) n,
              ROUND(AVG(relevance_score),3) avg_rel,
              ROUND(AVG(confidence),3) avg_conf,
              SUM(CASE WHEN confidence = 0.0 THEN 1 ELSE 0 END) omitted,
              SUM(CASE WHEN relevance_score < ? THEN 1 ELSE 0 END) rejects,
              MIN(judged_at) first, MAX(judged_at) last
       FROM llm_judgments GROUP BY 1,2 ORDER BY MIN(judged_at)`,
    )
    .all(RELEVANCE_BELOW);

  log('cohort            model                  n   avg_rel  avg_conf  omit%  reject%');
  for (const c of cohorts) {
    const mark = c.prompt_version === PROMPT_VERSION ? '*' : live.has(c.prompt_version) ? '~' : ' ';
    log(
      `${mark} ${c.prompt_version.padEnd(15)} ${String(c.model).padEnd(20)} ${String(c.n).padStart(5)}` +
        `   ${String(c.avg_rel).padStart(6)}    ${String(c.avg_conf).padStart(6)}` +
        `  ${pct(c.omitted, c.n).toFixed(0).padStart(4)}%  ${pct(c.rejects, c.n).toFixed(0).padStart(5)}%`,
    );
  }
  log('  (* = source cohort the demotion gate reads · ~ = live cohort that is NOT the source one)\n');

  // ── 5: MERGED != RUNNING ────────────────────────────────────────────────
  const sourceRows = db
    .prepare('SELECT COUNT(*) n FROM llm_judgments WHERE prompt_version = ?')
    .get(PROMPT_VERSION).n;
  const drift = deployDrift({ promptVersion: PROMPT_VERSION, sourceRows, liveVersion, binary, now });
  log('deploy truth');
  log(`  ${drift.status.padEnd(19)}: ${drift.message}\n`);
  if (drift.status === 'merged_not_running') findings.push(drift.message);

  // ── 1 + 2: distribution shape, per cohort ──────────────────────────────
  log('confidence distribution — top value per cohort');
  // Only LIVE cohorts raise findings. Retired cohorts are displayed because a
  // broken cohort sitting next to a healthy one is the clearest possible
  // illustration of what the spike check looks for, but a monitor that fails
  // every run on immutable history is a monitor nobody reads.
  for (const c of cohorts) {
    if (c.n < 20) continue;
    const isLive = live.has(c.prompt_version);
    const top = db
      .prepare(
        `SELECT ROUND(confidence,2) v, COUNT(*) n FROM llm_judgments
         WHERE prompt_version = ? AND model = ? GROUP BY 1 ORDER BY n DESC LIMIT 1`,
      )
      .get(c.prompt_version, c.model);
    const share = pct(top.n, c.n);
    const flag = share >= 40 ? (isLive ? '  <-- SPIKE' : '  <-- spike (retired cohort)') : '';
    log(
      `  ${c.prompt_version.padEnd(12)} value ${String(top.v).padStart(5)} holds ${share.toFixed(0).padStart(3)}% of ${c.n}${flag}`,
    );

    if (!isLive) continue;

    if (share >= 40) {
      findings.push(
        `${c.prompt_version}: ${share.toFixed(0)}% of judgments hold the single value ${top.v}. ` +
          `A real distribution does not do this — suspect a fabricated default or a field the model stopped emitting.`,
      );
    }
    const omitShare = pct(c.omitted, c.n);
    if (omitShare >= 10) {
      findings.push(
        `${c.prompt_version}: confidence absent on ${omitShare.toFixed(0)}% of judgments — ` +
          `those can never satisfy the >= ${CONFIDENCE_MIN} gate, so the gate is partly disabled.`,
      );
    }
    const rejectShare = pct(c.rejects, c.n);
    if (rejectShare >= 95 || rejectShare <= 5) {
      findings.push(
        `${c.prompt_version}: reject rate ${rejectShare.toFixed(0)}% — a judge this one-sided ` +
          `carries almost no information, whatever its confidence looks like.`,
      );
    }
  }

  // ── 4: can the gate still reach anything? ──────────────────────────────
  const reach = db
    .prepare(
      `SELECT
         SUM(CASE WHEN lj.relevance_score < ? AND lj.confidence >= ? THEN 1 ELSE 0 END) at_gate,
         SUM(CASE WHEN lj.relevance_score < 0.40 AND lj.confidence >= 0.60 THEN 1 ELSE 0 END) at_probe,
         COUNT(*) curated_judged
       FROM source_items si
       JOIN llm_judgments lj ON lj.source_item_id = si.id AND lj.prompt_version = ?
       WHERE si.feed_relevant = 1 AND COALESCE(si.feed_verdict_source,'score') = 'score'`,
    )
    .get(RELEVANCE_BELOW, CONFIDENCE_MIN, PROMPT_VERSION);

  log('\ngate reach on the CURATED feed (current cohort)');
  log(`  curated + judged      : ${reach.curated_judged ?? 0}`);
  log(`  qualify at the gate   : ${reach.at_gate ?? 0}`);
  log(`  qualify one notch out : ${reach.at_probe ?? 0}`);
  if ((reach.curated_judged ?? 0) === 0) {
    log('  -> the current cohort has not reached the curated feed yet (expected right');
    log('     after a PROMPT_VERSION bump; the gate reads only the current cohort)');
  } else if ((reach.at_gate ?? 0) === 0 && (reach.at_probe ?? 0) > 0) {
    findings.push(
      `the gate matches nothing but ${reach.at_probe} curated item(s) qualify one notch looser — ` +
        `it may be calibrated past the judge's output distribution.`,
    );
  }

  // ── Demotion volume ────────────────────────────────────────────────────
  const demotions = db
    .prepare(
      `SELECT DATE(feed_verdict_at) d, COUNT(*) n FROM source_items
       WHERE feed_verdict_reason = 'llm_reject' AND feed_verdict_at >= datetime('now','-7 days')
       GROUP BY 1 ORDER BY d DESC`,
    )
    .all();
  log('\ndemotions stamped llm_reject, last 7 days');
  if (demotions.length === 0) {
    log('  none');
  } else {
    for (const r of demotions) log(`  ${r.d}  ${String(r.n).padStart(4)}`);
  }

  // ── Verdict ────────────────────────────────────────────────────────────
  log('');
  if (findings.length === 0) {
    log('OK — no cohort anomaly detected.');
  } else {
    log(`${findings.length} finding(s):`);
    for (const f of findings) log(`  ! ${f}`);
  }
  return { exitCode: findings.length === 0 ? 0 : 1, findings, live, drift };
}

function main() {
  let constants;
  try {
    constants = parseShippedConstants(fs.readFileSync(JUDGE_SRC, 'utf8'));
  } catch (e) {
    console.error(`Could not read ${e instanceof ConstantMissing ? e.message : 'the judge constants'} from llm_judgments.rs.`);
    console.error('The monitor reads the shipped constants on purpose — it must not');
    console.error('fall back to a copy and report on a gate the product no longer runs.');
    process.exit(2);
  }

  if (!fs.existsSync(DEFAULT_DB_PATH)) {
    console.error(`No database at ${DEFAULT_DB_PATH}. Set FOURDA_DB_PATH to point at one.`);
    process.exit(2);
  }

  const db = new Database(DEFAULT_DB_PATH, { readonly: true, fileMustExist: true });
  db.pragma('query_only = 1');
  console.log('judge cohort monitor');
  console.log(`  db             : ${DEFAULT_DB_PATH}`);
  const binary = newestBinary(process.env.FOURDA_BIN_DIR || DEFAULT_BIN_DIR);
  console.log(`  binary         : ${binary ? `${binary.path} (built ${new Date(binary.mtimeMs).toISOString()})` : 'none found'}`);
  let result;
  try {
    result = run({ db, constants, binary });
  } finally {
    db.close();
  }
  process.exit(result.exitCode);
}

if (require.main === module) {
  main();
}

module.exports = {
  parseShippedConstants,
  newestBinary,
  liveCohortVersions,
  deployDrift,
  run,
  STALE_BINARY_HOURS,
  DRAIN_COHORT,
};
