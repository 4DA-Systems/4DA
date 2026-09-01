#!/usr/bin/env node
/**
 * Judge accuracy benchmark runner.
 *
 * Runs the LLM judge against the 87 human-labeled scenarios in
 * `src-tauri/src/scoring/benchmark_scenarios.json` and reports how well it
 * separates relevant from irrelevant — the only measurement in the codebase
 * that compares the judge to LABELS rather than to the scoring pipeline.
 *
 * Everything that decides an outcome (prompt, item rendering, parser,
 * thresholds) comes from the shipped judge code, never a copy. See
 * `src-tauri/src/scoring/judge_benchmark.rs` for the full rationale.
 *
 *   pnpm run bench:judge
 *
 * Costs roughly 6 cents per run on the cheap judge sibling (~11 API calls).
 * It runs in its own process, so it neither consumes nor is blocked by the
 * live app's daily cost cap — a starved day is exactly when you most need to
 * measure.
 *
 * Results append to `<data dir>/judge-benchmark-results.jsonl`, one row per
 * run, so drift across models and prompt versions stays visible. Run it:
 *   - after any change to the judge prompt or PROMPT_VERSION
 *   - after changing the judge model or the demotion thresholds
 *   - periodically, to catch silent provider-side model drift
 */

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const srcTauri = path.join(repoRoot, 'src-tauri');
const dbPath = process.env.FOURDA_DB_PATH || path.join(repoRoot, 'data', '4da.db');
const resultsPath = path.join(path.dirname(dbPath), 'judge-benchmark-results.jsonl');

const dataDir = path.dirname(dbPath);
if (!fs.existsSync(path.join(dataDir, 'settings.json'))) {
  console.error(`No settings.json in ${dataDir} — the benchmark needs a configured`);
  console.error('LLM provider to read an API key from. Set FOURDA_DB_PATH to a data');
  console.error('directory that has one.');
  process.exit(1);
}

console.log(`judge benchmark  ·  data dir: ${dataDir}`);
console.log('this makes ~11 real API calls (~6c)\n');

const result = spawnSync(
  'cargo',
  [
    'test',
    '--lib',
    'judge_benchmark::judge_accuracy_benchmark',
    '--',
    '--ignored',
    '--nocapture',
  ],
  {
    cwd: srcTauri,
    stdio: 'inherit',
    env: { ...process.env, FOURDA_JUDGE_BENCHMARK: '1', FOURDA_DB_PATH: dbPath },
  },
);

if (result.error) {
  console.error(`Failed to run cargo: ${result.error.message}`);
  process.exit(1);
}

// Show the trend, not just today's number. A single MCC is a reading; the
// sequence is what tells you the judge changed.
if (fs.existsSync(resultsPath)) {
  const rows = fs
    .readFileSync(resultsPath, 'utf8')
    .split('\n')
    .filter(Boolean)
    .slice(-10)
    .map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return null;
      }
    })
    .filter(Boolean);

  if (rows.length > 1) {
    console.log('\n--- recent runs ---');
    console.log(
      `  ${'when'.padEnd(20)} ${'model'.padEnd(20)} ${'prompt'.padEnd(8)} ${'MCC'.padStart(6)} ${'recall'.padStart(7)} ${'false_dem'.padStart(10)}`,
    );
    for (const r of rows) {
      const when = String(r.at || '').slice(0, 19).replace('T', ' ');
      console.log(
        `  ${when.padEnd(20)} ${String(r.model || '').padEnd(20)} ${String(r.prompt_version || '').padEnd(8)} ${Number(r.mcc).toFixed(3).padStart(6)} ${Number(r.recall).toFixed(3).padStart(7)} ${String(r.false_demotions).padStart(10)}`,
      );
    }
  }
  console.log(`\nfull history: ${resultsPath}`);
}

process.exit(result.status ?? 1);
