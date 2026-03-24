#!/usr/bin/env node
'use strict';

/**
 * Coverage Gate — ensures test coverage doesn't regress.
 *
 * Reads the coverage summary from vitest and enforces minimum thresholds.
 * Run: node scripts/coverage-gate.cjs
 *
 * Note: Must run `pnpm test -- --coverage` first to generate the summary.
 */

const fs = require('fs');
const path = require('path');

const COVERAGE_FILE = path.join(__dirname, '..', 'coverage', 'coverage-summary.json');

const THRESHOLDS = {
  statements: 40,
  branches: 25,
  functions: 35,
  lines: 40,
};

function main() {
  if (!fs.existsSync(COVERAGE_FILE)) {
    console.error('No coverage data found. Run: pnpm test -- --coverage');
    process.exit(1);
  }

  const data = JSON.parse(fs.readFileSync(COVERAGE_FILE, 'utf8'));
  const total = data.total;
  let failed = false;

  console.log('\n=== Coverage Gate ===\n');

  for (const [metric, threshold] of Object.entries(THRESHOLDS)) {
    const actual = total[metric]?.pct ?? 0;
    const status = actual >= threshold ? 'PASS' : 'FAIL';
    const icon = actual >= threshold ? '[PASS]' : '[FAIL]';
    console.log(`  ${icon} ${metric}: ${actual.toFixed(1)}% (threshold: ${threshold}%)`);
    if (actual < threshold) failed = true;
  }

  console.log('');

  if (failed) {
    console.error('Coverage below thresholds. Add tests before pushing.');
    process.exit(1);
  }

  console.log('Coverage gate passed.\n');
}

main();
