/**
 * 4DA Project Health Score — Composite metric across 8 dimensions.
 *
 * Produces a 0–100 score that tracks codebase quality over time.
 * Run: node scripts/project-health.cjs [--json] [--compare <file>]
 *
 * Dimensions (weights sum to 1.0):
 *   1. Test Health     (0.20) — test count, pass rate, coverage ratio
 *   2. Build Health    (0.15) — compiler errors, warnings, lint
 *   3. Size Discipline (0.15) — file size compliance, total LOC efficiency
 *   4. Safety          (0.15) — unwrap() density, panic paths
 *   5. Architecture    (0.10) — module count, max file size, coupling
 *   6. Freshness       (0.10) — dependency age, unused code
 *   7. Test Depth      (0.10) — test-to-code ratio, coverage breadth
 *   8. Hygiene         (0.05) — git cleanliness, TODO density
 */

'use strict';

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const RUST_SRC = path.join(ROOT, 'src-tauri', 'src');
const TS_SRC = path.join(ROOT, 'src');

// ============================================================================
// Helpers
// ============================================================================

function run(cmd, opts = {}) {
  try {
    return execSync(cmd, {
      cwd: opts.cwd || ROOT,
      encoding: 'utf-8',
      timeout: opts.timeout || 120000,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
  } catch (e) {
    return e.stdout ? e.stdout.trim() : '';
  }
}

function countFiles(dir, ext) {
  const result = run(`find "${dir}" -name "*.${ext}" -type f | wc -l`);
  return parseInt(result, 10) || 0;
}

function countLines(dir, ext) {
  const result = run(`find "${dir}" -name "*.${ext}" -type f -exec wc -l {} + | tail -1`);
  const match = result.match(/(\d+)\s+total/);
  return match ? parseInt(match[1], 10) : 0;
}

function countPattern(dir, pattern, ext, excludePatterns = []) {
  let cmd = `grep -rn "${pattern}" "${dir}" --include="*.${ext}"`;
  for (const ex of excludePatterns) {
    cmd += ` --exclude="${ex}"`;
  }
  cmd += ' | wc -l';
  const result = run(cmd);
  return parseInt(result, 10) || 0;
}

function clamp(val, min = 0, max = 100) {
  return Math.max(min, Math.min(max, val));
}

// Scoring function: maps a metric to 0-100 via configurable thresholds
// perfect = score of 100, terrible = score of 0
function scoreMetric(value, perfect, terrible) {
  if (perfect < terrible) {
    // Lower is better (e.g., error count)
    if (value <= perfect) return 100;
    if (value >= terrible) return 0;
    return 100 * (1 - (value - perfect) / (terrible - perfect));
  } else {
    // Higher is better (e.g., test count)
    if (value >= perfect) return 100;
    if (value <= terrible) return 0;
    return 100 * (value - terrible) / (perfect - terrible);
  }
}

// ============================================================================
// Dimension Collectors
// ============================================================================

function collectTestHealth() {
  // Rust tests
  const rustTestOutput = run('cargo test --lib 2>&1 | grep "test result:"', {
    cwd: path.join(ROOT, 'src-tauri'),
    timeout: 300000,
  });
  const rustMatch = rustTestOutput.match(/(\d+) passed.*?(\d+) failed/);
  const rustPassed = rustMatch ? parseInt(rustMatch[1], 10) : 0;
  const rustFailed = rustMatch ? parseInt(rustMatch[2], 10) : 0;

  // Frontend tests
  const tsTestOutput = run('pnpm run test -- --run 2>&1 | grep "Tests"', { timeout: 180000 });
  const tsMatch = tsTestOutput.match(/(\d+) passed/);
  const tsPassed = tsMatch ? parseInt(tsMatch[1], 10) : 0;

  const totalTests = rustPassed + tsPassed;
  const totalFailed = rustFailed;
  const passRate = totalTests > 0 ? (totalTests - totalFailed) / totalTests : 0;

  // Score: test count (more is better), pass rate (100% is perfect)
  const countScore = scoreMetric(totalTests, 3000, 500);
  const passScore = passRate * 100;

  return {
    name: 'Test Health',
    weight: 0.20,
    score: clamp(countScore * 0.4 + passScore * 0.6),
    metrics: {
      rust_tests_passed: rustPassed,
      rust_tests_failed: rustFailed,
      ts_tests_passed: tsPassed,
      total_tests: totalTests,
      pass_rate: `${(passRate * 100).toFixed(1)}%`,
    },
  };
}

function collectBuildHealth() {
  // Rust: check for errors and warnings
  const cargoOutput = run('cargo check 2>&1', {
    cwd: path.join(ROOT, 'src-tauri'),
    timeout: 180000,
  });
  const errors = (cargoOutput.match(/^error/gm) || []).length;
  const warnings = (cargoOutput.match(/warning\[/g) || []).length;

  // TypeScript: type check
  const tscOutput = run('npx tsc --noEmit 2>&1 | grep -c "error TS" || echo 0');
  const tsErrors = parseInt(tscOutput, 10) || 0;

  // ESLint errors
  const eslintOutput = run('npx eslint src/ --quiet 2>&1 | grep -c "error" || echo 0');
  const lintErrors = parseInt(eslintOutput, 10) || 0;

  const errorScore = scoreMetric(errors + tsErrors, 0, 10);
  const warningScore = scoreMetric(warnings + lintErrors, 0, 50);

  return {
    name: 'Build Health',
    weight: 0.15,
    score: clamp(errorScore * 0.7 + warningScore * 0.3),
    metrics: {
      rust_errors: errors,
      rust_warnings: warnings,
      ts_errors: tsErrors,
      lint_errors: lintErrors,
    },
  };
}

function collectSizeDiscipline() {
  // Run the file size checker
  const sizeOutput = run('node scripts/check-file-sizes.cjs 2>&1');
  const warnCount = (sizeOutput.match(/warn/g) || []).length;
  const errorCount = (sizeOutput.match(/\berror\b/g) || []).length;

  // Largest individual files (exclude 'total' line from wc -l)
  const rustLargest = run(`find "${RUST_SRC}" -name "*.rs" -exec wc -l {} + | grep -v total | sort -rn | head -1`);
  const largestMatch = rustLargest.match(/^\s*(\d+)/);
  const maxRustFile = largestMatch ? parseInt(largestMatch[1], 10) : 0;

  const tsLargest = run(`find "${TS_SRC}" \\( -name "*.ts" -o -name "*.tsx" \\) -exec wc -l {} + | grep -v total | sort -rn | head -1`);
  const tsLargestMatch = tsLargest.match(/^\s*(\d+)/);
  const maxTsFile = tsLargestMatch ? parseInt(tsLargestMatch[1], 10) : 0;

  const errorScore = scoreMetric(errorCount, 0, 5);
  const warnScore = scoreMetric(warnCount, 0, 30);
  const maxFileScore = scoreMetric(Math.max(maxRustFile, maxTsFile), 500, 2000);

  return {
    name: 'Size Discipline',
    weight: 0.15,
    score: clamp(errorScore * 0.5 + warnScore * 0.3 + maxFileScore * 0.2),
    metrics: {
      size_errors: errorCount,
      size_warnings: warnCount,
      max_rust_file_lines: maxRustFile,
      max_ts_file_lines: maxTsFile,
    },
  };
}

function collectSafety() {
  // Count unwrap() in non-test Rust code
  const totalUnwraps = countPattern(RUST_SRC, 'unwrap()', 'rs');
  // Count in test code
  const testUnwraps = parseInt(
    run(`grep -rn "unwrap()" "${RUST_SRC}" --include="*.rs" | grep -E "(#\\[test\\]|mod tests|_test\\.rs|benchmark|simulation)" | wc -l`),
    10,
  ) || 0;
  const prodUnwraps = totalUnwraps - testUnwraps;

  // Count panic!() in non-test code
  const totalPanics = countPattern(RUST_SRC, 'panic!', 'rs');

  // Count expect() — slightly better than unwrap but still panicking
  const totalExpects = countPattern(RUST_SRC, '\\.expect(', 'rs');

  // Lines of prod Rust code
  const rustLines = countLines(RUST_SRC, 'rs');
  const unwrapDensity = rustLines > 0 ? (prodUnwraps / rustLines) * 1000 : 0; // per 1000 lines

  // Score: unwrap density (lower is better)
  const densityScore = scoreMetric(unwrapDensity, 2, 15);
  const panicScore = scoreMetric(totalPanics, 0, 20);

  return {
    name: 'Safety',
    weight: 0.15,
    score: clamp(densityScore * 0.7 + panicScore * 0.3),
    metrics: {
      prod_unwraps: prodUnwraps,
      test_unwraps: testUnwraps,
      total_panics: totalPanics,
      total_expects: totalExpects,
      rust_lines: rustLines,
      unwrap_density_per_1k: parseFloat(unwrapDensity.toFixed(2)),
    },
  };
}

function collectArchitecture() {
  // Module count (Rust files)
  const rustFiles = countFiles(RUST_SRC, 'rs');
  const tsFiles = countFiles(TS_SRC, 'ts') + countFiles(TS_SRC, 'tsx');

  // Files over thresholds
  const rustOver1k = parseInt(
    run(`find "${RUST_SRC}" -name "*.rs" -exec wc -l {} + | grep -v total | awk '$1 > 1000' | wc -l`),
    10,
  ) || 0;
  const rustOver600 = parseInt(
    run(`find "${RUST_SRC}" -name "*.rs" -exec wc -l {} + | grep -v total | awk '$1 > 600' | wc -l`),
    10,
  ) || 0;

  // Average file size
  const rustLines = countLines(RUST_SRC, 'rs');
  const avgRustSize = rustFiles > 0 ? Math.round(rustLines / rustFiles) : 0;
  const tsLines = countLines(TS_SRC, 'ts') + countLines(TS_SRC, 'tsx');
  const avgTsSize = tsFiles > 0 ? Math.round(tsLines / tsFiles) : 0;

  const over1kScore = scoreMetric(rustOver1k, 0, 15);
  const avgSizeScore = scoreMetric(avgRustSize, 200, 500);

  return {
    name: 'Architecture',
    weight: 0.10,
    score: clamp(over1kScore * 0.5 + avgSizeScore * 0.5),
    metrics: {
      rust_files: rustFiles,
      ts_files: tsFiles,
      rust_over_1k_lines: rustOver1k,
      rust_over_600_lines: rustOver600,
      avg_rust_file_size: avgRustSize,
      avg_ts_file_size: avgTsSize,
      total_rust_lines: rustLines,
      total_ts_lines: tsLines,
      total_lines: rustLines + tsLines,
    },
  };
}

function collectTestDepth() {
  // Test files vs source files
  const rustTestFiles = parseInt(
    run(`grep -rl "#\\[cfg(test)\\]\\|#\\[test\\]" "${RUST_SRC}" --include="*.rs" | wc -l`),
    10,
  ) || 0;
  const tsTestFiles = parseInt(
    run(`find "${TS_SRC}" \\( -name "*.test.ts" -o -name "*.test.tsx" \\) | wc -l`),
    10,
  ) || 0;

  const rustFiles = countFiles(RUST_SRC, 'rs');
  const tsFiles = countFiles(TS_SRC, 'ts') + countFiles(TS_SRC, 'tsx');
  const tsSrcFiles = tsFiles - tsTestFiles;

  // Test file coverage ratio
  const rustTestRatio = rustFiles > 0 ? rustTestFiles / rustFiles : 0;
  const tsTestRatio = tsSrcFiles > 0 ? tsTestFiles / tsSrcFiles : 0;

  // Integration test count (separate test binaries in src-tauri/tests/)
  const integrationTests = parseInt(
    run(`ls "${path.join(ROOT, 'src-tauri', 'tests')}"/*.rs 2>/dev/null | wc -l`),
    10,
  ) || 0;

  const rustRatioScore = scoreMetric(rustTestRatio, 0.5, 0.1);
  const tsRatioScore = scoreMetric(tsTestRatio, 0.4, 0.05);

  return {
    name: 'Test Depth',
    weight: 0.10,
    score: clamp(rustRatioScore * 0.5 + tsRatioScore * 0.5),
    metrics: {
      rust_files_with_tests: rustTestFiles,
      ts_test_files: tsTestFiles,
      rust_test_coverage_ratio: parseFloat(rustTestRatio.toFixed(2)),
      ts_test_coverage_ratio: parseFloat(tsTestRatio.toFixed(2)),
      integration_test_files: integrationTests,
    },
  };
}

function collectFreshness() {
  // Check for deprecated dependencies warnings
  const auditOutput = run('pnpm audit 2>&1 | grep -ci "vulnerability" || echo 0');
  const vulnerabilities = parseInt(auditOutput, 10) || 0;

  // Unused imports in TypeScript (rough proxy)
  const unusedVars = parseInt(
    run(`cd "${ROOT}" && npx eslint src/ --rule '{"no-unused-vars":"warn"}' --quiet 2>&1 | grep -c "no-unused-vars" || echo 0`),
    10,
  ) || 0;

  // Dead Rust code (allow(dead_code) annotations)
  const deadCodeAllows = parseInt(
    run(`grep -rn "allow(dead_code)" "${RUST_SRC}" --include="*.rs" | wc -l`),
    10,
  ) || 0;

  const vulnScore = scoreMetric(vulnerabilities, 0, 20);
  const deadCodeScore = scoreMetric(deadCodeAllows, 5, 50);

  return {
    name: 'Freshness',
    weight: 0.10,
    score: clamp(vulnScore * 0.5 + deadCodeScore * 0.5),
    metrics: {
      npm_vulnerabilities: vulnerabilities,
      dead_code_allows: deadCodeAllows,
      unused_ts_vars: unusedVars,
    },
  };
}

function collectHygiene() {
  // Git cleanliness
  const gitStatus = run('git status --short');
  const uncommitted = gitStatus ? gitStatus.split('\n').filter(Boolean).length : 0;

  // TODO/FIXME/HACK count
  const todos = parseInt(
    run(`grep -rn "TODO\\|FIXME\\|HACK\\|XXX" "${RUST_SRC}" "${TS_SRC}" --include="*.rs" --include="*.ts" --include="*.tsx" | wc -l`),
    10,
  ) || 0;

  const gitScore = scoreMetric(uncommitted, 0, 30);
  const todoScore = scoreMetric(todos, 10, 100);

  return {
    name: 'Hygiene',
    weight: 0.05,
    score: clamp(gitScore * 0.5 + todoScore * 0.5),
    metrics: {
      uncommitted_files: uncommitted,
      todo_fixme_hack_count: todos,
    },
  };
}

// ============================================================================
// Main
// ============================================================================

function computeHealth() {
  const timestamp = new Date().toISOString();
  const commitHash = run('git rev-parse --short HEAD');
  const commitMsg = run('git log -1 --format="%s"');
  const branch = run('git rev-parse --abbrev-ref HEAD');

  console.log('Computing 4DA Project Health Score...\n');

  const dimensions = [
    collectTestHealth(),
    collectBuildHealth(),
    collectSizeDiscipline(),
    collectSafety(),
    collectArchitecture(),
    collectTestDepth(),
    collectFreshness(),
    collectHygiene(),
  ];

  // Composite score: weighted sum
  const composite = dimensions.reduce((sum, d) => sum + d.score * d.weight, 0);

  // Grade
  const grade =
    composite >= 90 ? 'A' :
    composite >= 80 ? 'B' :
    composite >= 70 ? 'C' :
    composite >= 60 ? 'D' : 'F';

  const report = {
    version: '1.0.0',
    timestamp,
    git: { commit: commitHash, message: commitMsg, branch },
    composite_score: parseFloat(composite.toFixed(1)),
    grade,
    dimensions: dimensions.map(d => ({
      name: d.name,
      weight: d.weight,
      score: parseFloat(d.score.toFixed(1)),
      weighted: parseFloat((d.score * d.weight).toFixed(2)),
      metrics: d.metrics,
    })),
  };

  return report;
}

function printReport(report) {
  const bar = (score) => {
    const filled = Math.round(score / 5);
    return '\u2588'.repeat(filled) + '\u2591'.repeat(20 - filled);
  };

  console.log('='.repeat(70));
  console.log('  4DA PROJECT HEALTH REPORT');
  console.log(`  ${report.git.branch} @ ${report.git.commit} — ${report.git.message}`);
  console.log(`  ${report.timestamp}`);
  console.log('='.repeat(70));
  console.log();

  for (const dim of report.dimensions) {
    const scoreStr = dim.score.toFixed(1).padStart(5);
    const weightStr = `(x${dim.weight.toFixed(2)})`;
    console.log(`  ${dim.name.padEnd(18)} ${bar(dim.score)} ${scoreStr}/100 ${weightStr} = ${dim.weighted.toFixed(2)}`);
    // Show key metrics inline
    const metricPairs = Object.entries(dim.metrics)
      .map(([k, v]) => `${k.replace(/_/g, ' ')}=${v}`)
      .join(', ');
    console.log(`  ${''.padEnd(18)} ${metricPairs}`);
    console.log();
  }

  console.log('-'.repeat(70));
  console.log(`  COMPOSITE SCORE:  ${report.composite_score.toFixed(1)} / 100  [Grade: ${report.grade}]`);
  console.log('-'.repeat(70));

  // Weakest dimensions
  const sorted = [...report.dimensions].sort((a, b) => a.score - b.score);
  console.log('\n  Weakest dimensions:');
  for (const dim of sorted.slice(0, 3)) {
    console.log(`    - ${dim.name}: ${dim.score.toFixed(1)}/100`);
  }
  console.log();
}

function compareReports(current, previousPath) {
  if (!fs.existsSync(previousPath)) {
    console.log(`  No previous report found at ${previousPath}`);
    return;
  }
  const previous = JSON.parse(fs.readFileSync(previousPath, 'utf-8'));
  const delta = current.composite_score - previous.composite_score;
  const arrow = delta > 0 ? '\u2191' : delta < 0 ? '\u2193' : '\u2192';
  console.log(`  Compared to ${previous.git.commit}: ${previous.composite_score.toFixed(1)} ${arrow} ${current.composite_score.toFixed(1)} (${delta > 0 ? '+' : ''}${delta.toFixed(1)})`);
  console.log();

  // Per-dimension deltas
  for (const dim of current.dimensions) {
    const prev = previous.dimensions.find(d => d.name === dim.name);
    if (prev) {
      const dd = dim.score - prev.score;
      if (Math.abs(dd) >= 1) {
        const dimArrow = dd > 0 ? '\u2191' : '\u2193';
        console.log(`    ${dim.name}: ${prev.score.toFixed(1)} ${dimArrow} ${dim.score.toFixed(1)} (${dd > 0 ? '+' : ''}${dd.toFixed(1)})`);
      }
    }
  }
}

// ============================================================================
// CLI
// ============================================================================

const args = process.argv.slice(2);
const jsonMode = args.includes('--json');
const saveFlag = args.includes('--save');
const compareIdx = args.indexOf('--compare');
const comparePath = compareIdx >= 0 ? args[compareIdx + 1] : null;

const report = computeHealth();

if (jsonMode) {
  console.log(JSON.stringify(report, null, 2));
} else {
  printReport(report);
  if (comparePath) {
    compareReports(report, comparePath);
  }
}

// Save report for future comparison
if (saveFlag) {
  const reportDir = path.join(ROOT, 'data', 'health-reports');
  if (!fs.existsSync(reportDir)) {
    fs.mkdirSync(reportDir, { recursive: true });
  }
  const filename = `health-${report.git.commit}-${Date.now()}.json`;
  const filepath = path.join(reportDir, filename);
  fs.writeFileSync(filepath, JSON.stringify(report, null, 2));
  console.log(`  Report saved to ${filepath}`);

  // Also save as "latest" for easy comparison
  const latestPath = path.join(reportDir, 'latest.json');
  fs.writeFileSync(latestPath, JSON.stringify(report, null, 2));
}
