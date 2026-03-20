/**
 * Coverage Map — per-module test coverage scanner for the 4DA codebase.
 *
 * Scans Rust and TypeScript source files, counts test functions per module,
 * classifies coverage health, and cross-references with metabolism data.
 *
 * Usage:
 *   node scripts/coverage-map.cjs              # Full colored terminal report
 *   node scripts/coverage-map.cjs --json       # JSON output
 *   node scripts/coverage-map.cjs --save       # Persist to ops-state.json + data/health-reports/
 *   node scripts/coverage-map.cjs --brief      # One-line summary (for session briefing)
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const ROOT = path.resolve(__dirname, '..');
const RUST_SRC = path.join(ROOT, 'src-tauri', 'src');
const TS_SRC = path.join(ROOT, 'src');
const OPS_STATE = path.join(ROOT, '.claude', 'wisdom', 'ops-state.json');
const HEALTH_DIR = path.join(ROOT, 'data', 'health-reports');

const MAX_TREND = 30;

const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', '.git', '_future', '__tests__']);

const COLORS = {
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  green: '\x1b[32m',
  cyan: '\x1b[36m',
  dim: '\x1b[2m',
  bold: '\x1b[1m',
  reset: '\x1b[0m',
};

// ============================================================================
// CLI flags
// ============================================================================

const args = process.argv.slice(2);
const flagJson = args.includes('--json');
const flagSave = args.includes('--save');
const flagBrief = args.includes('--brief');

// ============================================================================
// File Walking
// ============================================================================

function walkDir(dir, results, extensions) {
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
  for (const entry of entries) {
    if (SKIP_DIRS.has(entry.name)) continue;
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) walkDir(fullPath, results, extensions);
    else if (entry.isFile()) {
      const ext = path.extname(entry.name).slice(1);
      if (extensions.includes(ext)) results.push(fullPath);
    }
  }
}

function normalizeSlashes(p) {
  return p.replace(/\\/g, '/');
}

function relPath(fullPath) {
  return normalizeSlashes(path.relative(ROOT, fullPath));
}

// ============================================================================
// Rust Scanning
// ============================================================================

function countRustSourceLines(filePath) {
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch { return 0; }
  const lines = content.split('\n');
  let count = 0;
  let inTestBlock = false;
  for (const line of lines) {
    if (line.trim() === '#[cfg(test)]') { inTestBlock = true; continue; }
    if (!inTestBlock) count++;
  }
  return count;
}

function countRustTests(filePath) {
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch { return 0; }
  return (content.match(/#\[test\]/g) || []).length;
}

function countRustInlineTests(filePath) {
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch { return 0; }
  const lines = content.split('\n');
  let inTestBlock = false;
  let count = 0;
  for (const line of lines) {
    if (line.trim() === '#[cfg(test)]') { inTestBlock = true; continue; }
    if (inTestBlock && line.trim() === '#[test]') count++;
  }
  return count;
}

function scanRustModules() {
  const files = [];
  walkDir(RUST_SRC, files, ['rs']);

  const modules = [];

  for (const filePath of files) {
    const basename = path.basename(filePath, '.rs');

    // Skip main.rs, test_utils.rs, files ending in _tests.rs
    if (basename === 'main') continue;
    if (basename === 'test_utils') continue;
    if (basename.endsWith('_tests')) continue;

    const sourceLines = countRustSourceLines(filePath);
    let testCount = 0;

    // Inline tests (after #[cfg(test)] in same file)
    testCount += countRustInlineTests(filePath);

    // Companion test file: <basename>_tests.rs in same directory
    const dir = path.dirname(filePath);
    const companionPath = path.join(dir, `${basename}_tests.rs`);
    if (fs.existsSync(companionPath)) {
      testCount += countRustTests(companionPath);
    }

    const rel = relPath(filePath);
    const classification = classify(testCount, sourceLines);

    modules.push({
      file: rel,
      sourceLines,
      testCount,
      classification,
      language: 'rust',
    });
  }

  return modules;
}

// ============================================================================
// TypeScript Scanning
// ============================================================================

function isTestFile(filePath) {
  const base = path.basename(filePath);
  if (base.endsWith('.test.tsx') || base.endsWith('.test.ts')) return true;
  if (base === 'smoke.test.tsx') return true;
  return false;
}

function countTsTests(filePath) {
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch { return 0; }
  // Count it( and test( occurrences — match function call pattern
  const itMatches = (content.match(/\bit\s*\(/g) || []).length;
  const testMatches = (content.match(/\btest\s*\(/g) || []).length;
  return itMatches + testMatches;
}

function countTsSourceLines(filePath) {
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch { return 0; }
  return content.split('\n').length;
}

function categorizeTsFile(rel) {
  if (/\.tsx$/.test(rel) && rel.includes('components/')) return 'component';
  if (/^src\/hooks\//.test(rel) || /\/use-[^/]+\.ts$/.test(rel)) return 'hook';
  if (/^src\/store\//.test(rel)) return 'store';
  return 'util';
}

function scanTsModules() {
  const files = [];
  walkDir(TS_SRC, files, ['ts', 'tsx']);

  // Separate source and test files
  const sourceFiles = [];
  const testFiles = [];
  for (const f of files) {
    if (isTestFile(f)) testFiles.push(f);
    else sourceFiles.push(f);
  }

  // Build a lookup of test files by their "source base"
  // e.g. Foo.test.tsx -> Foo, use-bar.test.ts -> use-bar
  const testLookup = new Map();
  for (const tf of testFiles) {
    const base = path.basename(tf)
      .replace(/\.test\.tsx$/, '')
      .replace(/\.test\.ts$/, '');
    const dir = path.dirname(tf);
    // Store keyed by dir + base for lookup
    const key = normalizeSlashes(path.join(dir, base));
    testLookup.set(key, tf);
  }

  const modules = [];

  for (const filePath of sourceFiles) {
    const rel = relPath(filePath);
    const sourceLines = countTsSourceLines(filePath);
    let testCount = 0;

    const base = path.basename(filePath).replace(/\.tsx$/, '').replace(/\.ts$/, '');
    const dir = path.dirname(filePath);

    // Check same-dir test file
    const sameDir = normalizeSlashes(path.join(dir, base));
    if (testLookup.has(sameDir)) {
      testCount += countTsTests(testLookup.get(sameDir));
    }

    // Check __tests__/<base>.test.tsx in same dir
    const testsDirKey = normalizeSlashes(path.join(dir, '__tests__', base));
    if (testLookup.has(testsDirKey)) {
      testCount += countTsTests(testLookup.get(testsDirKey));
    }

    const category = categorizeTsFile(rel);
    const classification = classify(testCount, sourceLines);

    modules.push({
      file: rel,
      sourceLines,
      testCount,
      classification,
      language: 'typescript',
      category,
    });
  }

  return modules;
}

// ============================================================================
// Classification
// ============================================================================

function classify(testCount, sourceLines) {
  if (testCount === 0 && sourceLines > 200) return 'CRITICAL';
  if (sourceLines === 0) return 'ADEQUATE';
  const ratio = testCount / sourceLines;
  if (ratio >= 1 / 50) return 'HEALTHY';
  if (testCount > 0 && ratio < 1 / 100) return 'WARNING';
  return 'ADEQUATE';
}

// ============================================================================
// Metabolism (Heat)
// ============================================================================

function getMetabolism() {
  try {
    const state = JSON.parse(fs.readFileSync(OPS_STATE, 'utf8'));
    return state.metabolism?.fileChangeFrequency || {};
  } catch { return {}; }
}

function heatLabel(changeCount) {
  if (changeCount >= 10) return 'HOT';
  if (changeCount >= 3) return 'WARM';
  if (changeCount >= 1) return 'COLD';
  return 'DEAD';
}

function attachHeat(modules) {
  const metabolism = getMetabolism();
  for (const mod of modules) {
    const changes = metabolism[mod.file] || 0;
    mod.changes = changes;
    mod.heat = heatLabel(changes);
  }
}

// ============================================================================
// Reporting — Terminal
// ============================================================================

function classColor(cls) {
  switch (cls) {
    case 'CRITICAL': return COLORS.red;
    case 'WARNING': return COLORS.yellow;
    case 'HEALTHY': return COLORS.green;
    default: return COLORS.dim;
  }
}

function heatIcon(heat) {
  switch (heat) {
    case 'HOT': return `${COLORS.red}HOT${COLORS.reset}`;
    case 'WARM': return `${COLORS.yellow}WARM${COLORS.reset}`;
    case 'COLD': return `${COLORS.dim}COLD${COLORS.reset}`;
    default: return `${COLORS.dim}DEAD${COLORS.reset}`;
  }
}

function printSection(title, modules, color) {
  if (modules.length === 0) return;
  console.log(`\n${color}${COLORS.bold}=== ${title} (${modules.length}) ===${COLORS.reset}`);

  const maxFileLen = Math.max(...modules.map(m => m.file.length), 4);
  const header = '  ' + 'File'.padEnd(maxFileLen) + '  Lines  Tests  Heat';
  console.log(`${COLORS.dim}${header}${COLORS.reset}`);
  console.log(`${COLORS.dim}  ${'-'.repeat(header.length - 2)}${COLORS.reset}`);

  for (const mod of modules) {
    const line = `  ${color}${mod.file.padEnd(maxFileLen)}${COLORS.reset}`
      + `  ${String(mod.sourceLines).padStart(5)}`
      + `  ${String(mod.testCount).padStart(5)}`
      + `  ${heatIcon(mod.heat)}`;
    console.log(line);
  }
}

function printTerminalReport(allModules) {
  const critical = allModules.filter(m => m.classification === 'CRITICAL')
    .sort((a, b) => b.sourceLines - a.sourceLines);
  const warning = allModules.filter(m => m.classification === 'WARNING')
    .sort((a, b) => b.sourceLines - a.sourceLines);
  const healthy = allModules.filter(m => m.classification === 'HEALTHY');
  const adequate = allModules.filter(m => m.classification === 'ADEQUATE');

  console.log(`\n${COLORS.bold}${COLORS.cyan}4DA Coverage Map${COLORS.reset}`);
  console.log(`${COLORS.dim}Scanned ${allModules.length} modules${COLORS.reset}`);

  printSection('CRITICAL MODULES — untested, >200 lines', critical, COLORS.red);
  printSection('WARNING MODULES — under-tested', warning, COLORS.yellow);

  // Summary stats
  const rustModules = allModules.filter(m => m.language === 'rust');
  const tsModules = allModules.filter(m => m.language === 'typescript');
  const totalTests = allModules.reduce((s, m) => s + m.testCount, 0);
  const totalLines = allModules.reduce((s, m) => s + m.sourceLines, 0);
  const ratio = totalLines > 0 ? totalTests / totalLines : 0;

  console.log(`\n${COLORS.bold}=== SUMMARY ===${COLORS.reset}`);
  console.log(`  Rust modules:       ${rustModules.length} (${rustModules.filter(m => m.classification === 'CRITICAL').length} critical, ${rustModules.filter(m => m.classification === 'WARNING').length} warning, ${rustModules.filter(m => m.classification === 'ADEQUATE').length} adequate, ${rustModules.filter(m => m.classification === 'HEALTHY').length} healthy)`);
  console.log(`  Frontend modules:   ${tsModules.length} (${tsModules.filter(m => m.classification === 'CRITICAL').length} critical, ${tsModules.filter(m => m.classification === 'WARNING').length} warning, ${tsModules.filter(m => m.classification === 'ADEQUATE').length} adequate, ${tsModules.filter(m => m.classification === 'HEALTHY').length} healthy)`);
  console.log(`  Total source lines: ${totalLines.toLocaleString()}`);
  console.log(`  Total test fns:     ${totalTests.toLocaleString()}`);
  console.log(`  Coverage ratio:     ${(ratio * 100).toFixed(1)}% (1 test per ${ratio > 0 ? Math.round(1 / ratio) : '---'} lines)`);
  console.log(`  Healthy threshold:  2.0% (1 test per 50 lines)`);

  // Hot + Untested — highest priority
  const hotUntested = allModules
    .filter(m => m.testCount === 0 && (m.heat === 'HOT' || m.heat === 'WARM'))
    .sort((a, b) => b.changes - a.changes);

  if (hotUntested.length > 0) {
    console.log(`\n${COLORS.red}${COLORS.bold}=== HOT + UNTESTED — highest priority (${hotUntested.length}) ===${COLORS.reset}`);
    const maxFileLen = Math.max(...hotUntested.map(m => m.file.length), 4);
    const header = '  ' + 'File'.padEnd(maxFileLen) + '  Lines  Changes';
    console.log(`${COLORS.dim}${header}${COLORS.reset}`);
    console.log(`${COLORS.dim}  ${'-'.repeat(header.length - 2)}${COLORS.reset}`);
    for (const mod of hotUntested) {
      const line = `  ${COLORS.red}${mod.file.padEnd(maxFileLen)}${COLORS.reset}`
        + `  ${String(mod.sourceLines).padStart(5)}`
        + `  ${String(mod.changes).padStart(7)}`;
      console.log(line);
    }
  }

  console.log('');
}

// ============================================================================
// Brief Output
// ============================================================================

function printBrief(allModules) {
  const totalTests = allModules.reduce((s, m) => s + m.testCount, 0);
  const totalLines = allModules.reduce((s, m) => s + m.sourceLines, 0);
  const ratio = totalLines > 0 ? (totalTests / totalLines * 100).toFixed(1) : '0.0';
  const hotUntested = allModules.filter(m => m.testCount === 0 && (m.heat === 'HOT' || m.heat === 'WARM'));
  console.log(`COVERAGE: ${ratio}% (target 25%) | ${hotUntested.length} hot+untested modules`);
}

// ============================================================================
// JSON Output
// ============================================================================

function buildJsonReport(allModules) {
  const rustModules = allModules.filter(m => m.language === 'rust');
  const tsModules = allModules.filter(m => m.language === 'typescript');
  const totalTests = allModules.reduce((s, m) => s + m.testCount, 0);
  const totalLines = allModules.reduce((s, m) => s + m.sourceLines, 0);
  const ratio = totalLines > 0 ? totalTests / totalLines : 0;
  const hotUntested = allModules
    .filter(m => m.testCount === 0 && (m.heat === 'HOT' || m.heat === 'WARM'))
    .sort((a, b) => b.changes - a.changes);

  return {
    timestamp: new Date().toISOString(),
    totalModules: allModules.length,
    totalSourceLines: totalLines,
    totalTestFunctions: totalTests,
    coverageRatio: Math.round(ratio * 10000) / 10000,
    rust: {
      modules: rustModules.length,
      critical: rustModules.filter(m => m.classification === 'CRITICAL').length,
      warning: rustModules.filter(m => m.classification === 'WARNING').length,
      adequate: rustModules.filter(m => m.classification === 'ADEQUATE').length,
      healthy: rustModules.filter(m => m.classification === 'HEALTHY').length,
    },
    frontend: {
      modules: tsModules.length,
      critical: tsModules.filter(m => m.classification === 'CRITICAL').length,
      warning: tsModules.filter(m => m.classification === 'WARNING').length,
      adequate: tsModules.filter(m => m.classification === 'ADEQUATE').length,
      healthy: tsModules.filter(m => m.classification === 'HEALTHY').length,
      byCategory: {
        component: tsModules.filter(m => m.category === 'component').length,
        hook: tsModules.filter(m => m.category === 'hook').length,
        store: tsModules.filter(m => m.category === 'store').length,
        util: tsModules.filter(m => m.category === 'util').length,
      },
    },
    hotUntested: hotUntested.map(m => m.file),
    hotUntestedCount: hotUntested.length,
    modules: allModules,
  };
}

// ============================================================================
// Save to ops-state.json + health-reports
// ============================================================================

function saveResults(allModules) {
  const rustModules = allModules.filter(m => m.language === 'rust');
  const tsModules = allModules.filter(m => m.language === 'typescript');
  const totalTests = allModules.reduce((s, m) => s + m.testCount, 0);
  const totalLines = allModules.reduce((s, m) => s + m.sourceLines, 0);
  const ratio = totalLines > 0 ? totalTests / totalLines : 0;
  const hotUntested = allModules
    .filter(m => m.testCount === 0 && (m.heat === 'HOT' || m.heat === 'WARM'))
    .sort((a, b) => b.changes - a.changes);

  const now = new Date().toISOString();

  // Update ops-state.json
  let state = {};
  try { state = JSON.parse(fs.readFileSync(OPS_STATE, 'utf8')); } catch { state = {}; }

  const coverageEntry = {
    lastRun: now,
    rustModules: {
      critical: rustModules.filter(m => m.classification === 'CRITICAL').length,
      warning: rustModules.filter(m => m.classification === 'WARNING').length,
      adequate: rustModules.filter(m => m.classification === 'ADEQUATE').length,
      healthy: rustModules.filter(m => m.classification === 'HEALTHY').length,
    },
    frontendComponents: {
      untested: tsModules.filter(m => m.testCount === 0).length,
      tested: tsModules.filter(m => m.testCount > 0).length,
    },
    coverageRatio: Math.round(ratio * 10000) / 10000,
    hotUntested: hotUntested.slice(0, 20).map(m => path.basename(m.file)),
    hotUntestedCount: hotUntested.length,
    trend: [],
  };

  // Preserve existing trend data
  if (state.coverage && Array.isArray(state.coverage.trend)) {
    coverageEntry.trend = state.coverage.trend;
  }

  // Append new trend entry
  coverageEntry.trend.push({
    date: now,
    ratio: coverageEntry.coverageRatio,
  });

  // Trim trend to last MAX_TREND entries
  if (coverageEntry.trend.length > MAX_TREND) {
    coverageEntry.trend = coverageEntry.trend.slice(-MAX_TREND);
  }

  state.coverage = coverageEntry;
  fs.writeFileSync(OPS_STATE, JSON.stringify(state, null, 2));
  console.log(`Saved coverage to ${relPath(OPS_STATE)}`);

  // Save snapshot to data/health-reports/
  try {
    if (!fs.existsSync(HEALTH_DIR)) fs.mkdirSync(HEALTH_DIR, { recursive: true });
    const timestamp = now.replace(/[:.]/g, '-').replace('T', '_').slice(0, 19);
    const snapshotPath = path.join(HEALTH_DIR, `coverage-${timestamp}.json`);
    const report = buildJsonReport(allModules);
    fs.writeFileSync(snapshotPath, JSON.stringify(report, null, 2));
    console.log(`Saved snapshot to ${relPath(snapshotPath)}`);
  } catch (e) {
    console.error(`Failed to save health report: ${e.message}`);
  }
}

// ============================================================================
// Main
// ============================================================================

function main() {
  // Scan both codebases
  const rustModules = scanRustModules();
  const tsModules = scanTsModules();
  const allModules = [...rustModules, ...tsModules];

  // Attach heat data
  attachHeat(allModules);

  // Output based on flags
  if (flagJson) {
    const report = buildJsonReport(allModules);
    console.log(JSON.stringify(report, null, 2));
  } else if (flagBrief) {
    printBrief(allModules);
  } else {
    printTerminalReport(allModules);
  }

  if (flagSave) {
    saveResults(allModules);
  }
}

main();
