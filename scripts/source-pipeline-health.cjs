#!/usr/bin/env node
/*
 * source-pipeline-health.cjs — repeatable health read of the 20 content sources.
 *
 * WHY THIS EXISTS
 *   "sourcePipeline" is a sovereignty component, and it sat at a stale 45 for 17 days because
 *   NOTHING measured it routinely — the only path was the running app's MCP. The pipeline degrades
 *   silently: an adapter starts timing out or returning a bad body, its production drops to zero, and
 *   nobody notices until a human happens to look. This script makes that measurement a one-command,
 *   no-app-required loop, reading the same tables the app writes (`sources`, `source_health`,
 *   `source_items`).
 *
 * WHAT IT REPORTS  (per source, worst-first)
 *   - enabled, last_fetch age, total items, items in the last 7d / 30d, newest item timestamp
 *   - live status from `source_health` (status, consecutive_failures, last_error, items_fetched)
 *   - a classification: HEALTHY / STALE / DEAD / ERROR / CREDENTIALS? / DISABLED
 *
 * USAGE
 *   node scripts/source-pipeline-health.cjs            # human table
 *   node scripts/source-pipeline-health.cjs --json     # machine-readable summary
 *   FOURDA_DATA_DIR=/path node scripts/source-pipeline-health.cjs   # custom data dir
 *
 * Exit 0 always when the DB is readable (this is a *report*, not a gate). Exit 0 with a notice when
 * the DB is absent (fresh checkout / CI) so it never breaks a pipeline that has no data yet.
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const DATA_DIR = process.env.FOURDA_DATA_DIR || path.join(ROOT, 'data');
const DB_PATH = path.join(DATA_DIR, '4da.db');
const JSON_OUT = process.argv.includes('--json');

// Sources that produce nothing without user-supplied credentials/keys — a zero count for these is a
// config decision, not a code bug. Used only to annotate, never to hide a real problem.
const CREDENTIAL_GATED = new Set(['twitter', 'producthunt']);

if (!fs.existsSync(DB_PATH)) {
  console.log(`source-pipeline-health: no DB at ${DB_PATH} (fresh checkout?) — nothing to measure.`);
  process.exit(0);
}

let Database;
try {
  Database = require('better-sqlite3');
} catch {
  console.log('source-pipeline-health: better-sqlite3 not installed — run `pnpm install`. Skipping.');
  process.exit(0);
}

const db = new Database(DB_PATH, { readonly: true, fileMustExist: true });

const rows = db.prepare(`
  SELECT s.source_type AS type, s.name, s.enabled, s.last_fetch,
         CAST(julianday('now') - julianday(s.last_fetch) AS INT) AS fetch_age_days,
         (SELECT COUNT(*) FROM source_items i WHERE i.source_type = s.source_type) AS total,
         (SELECT COUNT(*) FROM source_items i WHERE i.source_type = s.source_type
            AND i.created_at > datetime('now','-7 days')) AS d7,
         (SELECT COUNT(*) FROM source_items i WHERE i.source_type = s.source_type
            AND i.created_at > datetime('now','-30 days')) AS d30,
         (SELECT MAX(i.created_at) FROM source_items i WHERE i.source_type = s.source_type) AS newest,
         h.status AS hstatus, h.consecutive_failures AS cfail, h.error_count AS errcount,
         h.items_fetched AS last_fetched, h.last_error AS last_error
  FROM sources s
  LEFT JOIN source_health h ON h.source_type = s.source_type
  ORDER BY d7 ASC, total ASC
`).all();
db.close();

function classify(r) {
  if (!r.enabled) return 'DISABLED';
  if (r.hstatus === 'error' || (r.cfail || 0) > 0) return 'ERROR';
  if (r.total === 0) return CREDENTIAL_GATED.has(r.type) ? 'CREDENTIALS?' : 'DEAD';
  if (r.d7 === 0) return 'STALE';
  return 'HEALTHY';
}

const classified = rows.map((r) => ({ ...r, klass: classify(r) }));
const counts = classified.reduce((a, r) => ((a[r.klass] = (a[r.klass] || 0) + 1), a), {});
const enabled = classified.filter((r) => r.enabled);
const producing = enabled.filter((r) => r.klass === 'HEALTHY').length;
const healthPct = enabled.length ? Math.round((producing / enabled.length) * 100) : 0;

if (JSON_OUT) {
  console.log(JSON.stringify({
    healthPct,
    enabled: enabled.length,
    producing,
    counts,
    problems: classified
      .filter((r) => ['ERROR', 'DEAD', 'STALE', 'CREDENTIALS?'].includes(r.klass))
      .map((r) => ({ type: r.type, klass: r.klass, total: r.total, d7: r.d7,
        fetch_age_days: r.fetch_age_days, last_error: r.last_error })),
  }, null, 2));
  process.exit(0);
}

const pad = (v, n) => String(v == null ? '-' : v).padEnd(n).slice(0, n);
console.log(`SOURCE PIPELINE HEALTH — ${producing}/${enabled.length} producing (${healthPct}%)  [${DB_PATH}]\n`);
console.log(pad('source', 15) + pad('class', 13) + pad('en', 3) + pad('age_d', 7) +
  pad('total', 8) + pad('7d', 6) + pad('30d', 7) + 'newest / last_error');
console.log('-'.repeat(96));
for (const r of classified) {
  const note = r.klass === 'ERROR' && r.last_error
    ? String(r.last_error).replace(/\s+/g, ' ').slice(0, 38)
    : r.newest || '-';
  console.log(pad(r.type, 15) + pad(r.klass, 13) + pad(r.enabled, 3) + pad(r.fetch_age_days, 7) +
    pad(r.total, 8) + pad(r.d7, 6) + pad(r.d30, 7) + note);
}
console.log('-'.repeat(96));
console.log('summary: ' + Object.entries(counts).map(([k, v]) => `${k}=${v}`).join('  '));
const probs = classified.filter((r) => ['ERROR', 'DEAD', 'STALE'].includes(r.klass));
if (probs.length) {
  console.log('\nattention (not credential-gated):');
  for (const r of probs) {
    console.log(`  • ${r.type} [${r.klass}] — total=${r.total}, 7d=${r.d7}` +
      (r.last_error ? `, last_error="${String(r.last_error).replace(/\s+/g, ' ').slice(0, 60)}"` : '') +
      (r.newest ? `, newest=${r.newest}` : ''));
  }
}
