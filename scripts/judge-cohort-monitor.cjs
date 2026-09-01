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
 * Thresholds and the current prompt version are READ FROM THE RUST SOURCE, not
 * copied here — a monitor with its own copy of a constant reports on a gate the
 * product no longer runs.
 */

const fs = require('node:fs');
const path = require('node:path');
const Database = require('better-sqlite3');

const repoRoot = path.resolve(__dirname, '..');
const dbPath = process.env.FOURDA_DB_PATH || path.join(repoRoot, 'data', '4da.db');
const judgeSrc = path.join(repoRoot, 'src-tauri', 'src', 'llm_judgments.rs');

// ── Read the shipped constants ────────────────────────────────────────────
function constFromRust(source, name, pattern) {
  const m = source.match(pattern);
  if (!m) {
    console.error(`Could not read ${name} from llm_judgments.rs.`);
    console.error('The monitor reads the shipped constants on purpose — it must not');
    console.error('fall back to a copy and report on a gate the product no longer runs.');
    process.exit(2);
  }
  return m[1];
}

const src = fs.readFileSync(judgeSrc, 'utf8');
const PROMPT_VERSION = constFromRust(
  src,
  'PROMPT_VERSION',
  /const PROMPT_VERSION: &str = "([^"]+)"/,
);
const RELEVANCE_BELOW = Number(
  constFromRust(src, 'DEMOTION_RELEVANCE_BELOW', /DEMOTION_RELEVANCE_BELOW: f64 = ([0-9.]+)/),
);
const CONFIDENCE_MIN = Number(
  constFromRust(src, 'DEMOTION_CONFIDENCE_MIN', /DEMOTION_CONFIDENCE_MIN: f64 = ([0-9.]+)/),
);

if (!fs.existsSync(dbPath)) {
  console.error(`No database at ${dbPath}. Set FOURDA_DB_PATH to point at one.`);
  process.exit(2);
}

const db = new Database(dbPath, { readonly: true, fileMustExist: true });
const findings = [];
const pct = (n, d) => (d === 0 ? 0 : (100 * n) / d);

console.log('judge cohort monitor');
console.log(`  db             : ${dbPath}`);
console.log(`  current prompt : ${PROMPT_VERSION}`);
console.log(`  shipped gate   : relevance < ${RELEVANCE_BELOW} AND confidence >= ${CONFIDENCE_MIN}\n`);

// ── Cohorts ───────────────────────────────────────────────────────────────
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

console.log('cohort            model                  n   avg_rel  avg_conf  omit%  reject%');
for (const c of cohorts) {
  const current = c.prompt_version === PROMPT_VERSION ? '*' : ' ';
  console.log(
    `${current} ${c.prompt_version.padEnd(15)} ${String(c.model).padEnd(20)} ${String(c.n).padStart(5)}` +
      `   ${String(c.avg_rel).padStart(6)}    ${String(c.avg_conf).padStart(6)}` +
      `  ${pct(c.omitted, c.n).toFixed(0).padStart(4)}%  ${pct(c.rejects, c.n).toFixed(0).padStart(5)}%`,
  );
}
console.log('  (* = the cohort the demotion gate reads)\n');

// ── 1 + 2: distribution shape, per cohort ────────────────────────────────
console.log('confidence distribution — top value per cohort');
// Only LIVE cohorts raise findings. v3 and v4 are quarantined history — they
// are displayed because a broken cohort sitting next to a healthy one is the
// clearest possible illustration of what the spike check looks for, but a
// monitor that fails every run on immutable history is a monitor nobody reads.
const isLive = (v) => v === PROMPT_VERSION || v === 'drain_v1';

for (const c of cohorts) {
  if (c.n < 20) continue;
  const live = isLive(c.prompt_version);
  const top = db
    .prepare(
      `SELECT ROUND(confidence,2) v, COUNT(*) n FROM llm_judgments
       WHERE prompt_version = ? AND model = ? GROUP BY 1 ORDER BY n DESC LIMIT 1`,
    )
    .get(c.prompt_version, c.model);
  const share = pct(top.n, c.n);
  const flag = share >= 40 ? (live ? '  <-- SPIKE' : '  <-- spike (retired cohort)') : '';
  console.log(
    `  ${c.prompt_version.padEnd(12)} value ${String(top.v).padStart(5)} holds ${share.toFixed(0).padStart(3)}% of ${c.n}${flag}`,
  );

  if (!live) continue;

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

// ── 4: can the gate still reach anything? ────────────────────────────────
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

console.log('\ngate reach on the CURATED feed (current cohort)');
console.log(`  curated + judged      : ${reach.curated_judged ?? 0}`);
console.log(`  qualify at the gate   : ${reach.at_gate ?? 0}`);
console.log(`  qualify one notch out : ${reach.at_probe ?? 0}`);
if ((reach.curated_judged ?? 0) === 0) {
  console.log('  -> the current cohort has not reached the curated feed yet (expected right');
  console.log('     after a PROMPT_VERSION bump; the gate reads only the current cohort)');
} else if ((reach.at_gate ?? 0) === 0 && (reach.at_probe ?? 0) > 0) {
  findings.push(
    `the gate matches nothing but ${reach.at_probe} curated item(s) qualify one notch looser — ` +
      `it may be calibrated past the judge's output distribution.`,
  );
}

// ── Demotion volume ───────────────────────────────────────────────────────
const demotions = db
  .prepare(
    `SELECT DATE(feed_verdict_at) d, COUNT(*) n FROM source_items
     WHERE feed_verdict_reason = 'llm_reject' AND feed_verdict_at >= datetime('now','-7 days')
     GROUP BY 1 ORDER BY d DESC`,
  )
  .all();
console.log('\ndemotions stamped llm_reject, last 7 days');
if (demotions.length === 0) {
  console.log('  none');
} else {
  for (const r of demotions) console.log(`  ${r.d}  ${String(r.n).padStart(4)}`);
}

db.close();

// ── Verdict ───────────────────────────────────────────────────────────────
console.log('');
if (findings.length === 0) {
  console.log('OK — no cohort anomaly detected.');
  process.exit(0);
}
console.log(`${findings.length} finding(s):`);
for (const f of findings) console.log(`  ! ${f}`);
process.exit(1);
