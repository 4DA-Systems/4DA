/**
 * Build Guardian — comprehensive pre-deploy verification.
 *
 * Runs all critical checks that must pass before any deployment:
 *   1. TypeScript compilation (fresh, no cache)
 *   2. Vite build (actual bundling, not just type check)
 *   3. Site build verification (Eleventy, if site/ exists)
 *   4. Rust compilation (cargo check)
 *   5. Frontend tests (Vitest)
 *   6. File size limits
 *   7. IPC contract validation
 *
 * Usage:
 *   node scripts/build-guardian.cjs          # full check (7 gates)
 *   node scripts/build-guardian.cjs --quick  # skip tests + Rust (4 gates)
 *   pnpm run guardian                        # via package.json
 *
 * Exit: 0 if all checks pass, 1 if any check fails.
 */

'use strict';

const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const QUICK = process.argv.includes('--quick');

// ============================================================================
// Helpers
// ============================================================================

const PASS = '[PASS]';
const FAIL = '[FAIL]';
const SKIP = '[SKIP]';

/** Run a command, return { ok, output, duration }. */
function run(label, command, options = {}) {
  const cwd = options.cwd || ROOT;
  const start = Date.now();
  try {
    const output = execSync(command, {
      cwd,
      stdio: 'pipe',
      encoding: 'utf8',
      timeout: options.timeout || 120_000,
      env: { ...process.env, ...(options.env || {}) },
    });
    const duration = ((Date.now() - start) / 1000).toFixed(1);
    return { ok: true, output: output.trim(), duration };
  } catch (err) {
    const duration = ((Date.now() - start) / 1000).toFixed(1);
    const output = (err.stdout || '') + '\n' + (err.stderr || '');
    return { ok: false, output: output.trim(), duration };
  }
}

/** Print a section header. */
function header(text) {
  console.log('');
  console.log('------------------------------------------------------------');
  console.log(`  ${text}`);
  console.log('------------------------------------------------------------');
}

// ============================================================================
// Checks
// ============================================================================

const results = [];

function record(name, passed, duration, detail) {
  const status = passed ? PASS : FAIL;
  results.push({ name, passed, duration, detail });
  console.log(`  ${status} ${name} (${duration}s)`);
  if (!passed && detail) {
    // Show first 30 lines of error output
    const lines = detail.split('\n').slice(0, 30);
    for (const line of lines) {
      console.log(`       ${line}`);
    }
    if (detail.split('\n').length > 30) {
      console.log(`       ... (${detail.split('\n').length - 30} more lines)`);
    }
  }
}

// ============================================================================
// Main
// ============================================================================

console.log('');
console.log('============================================================');
console.log('  BUILD GUARDIAN — Pre-deploy Verification');
console.log('============================================================');

// --- 1. TypeScript compilation (fresh, no incremental cache) ---
header('TypeScript Compilation');
{
  const res = run(
    'tsc',
    'node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit',
    { timeout: 180_000 }
  );
  record('TypeScript (tsc --noEmit)', res.ok, res.duration, res.ok ? null : res.output);
}

// --- 2. Vite build (actual bundling — catches what tsc alone misses) ---
header('Vite Build');
{
  const res = run(
    'vite',
    'npx vite build',
    { timeout: 180_000, env: { NODE_OPTIONS: '--max-old-space-size=8192' } }
  );
  record('Vite build', res.ok, res.duration, res.ok ? null : res.output);
}

// --- 3. Site build verification ---
header('Site Build Verification');
{
  const siteDir = path.join(ROOT, 'site');
  const fs = require('fs');
  if (fs.existsSync(siteDir) && fs.existsSync(path.join(siteDir, 'package.json'))) {
    const res = run(
      'eleventy',
      'npx @11ty/eleventy',
      { cwd: siteDir, timeout: 60_000 }
    );
    record('Site build (Eleventy)', res.ok, res.duration, res.ok ? null : res.output);
  } else {
    console.log(`  ${SKIP} Site build (site/ directory not found)`);
    results.push({ name: 'Site build', passed: true, duration: '0.0', detail: null });
  }
}

// --- 4. Rust compilation (cargo check is faster than cargo build) ---
if (!QUICK) {
  header('Rust Compilation');
  {
    const tauriDir = path.join(ROOT, 'src-tauri');
    const fs = require('fs');
    if (fs.existsSync(path.join(tauriDir, 'Cargo.toml'))) {
      const res = run(
        'cargo',
        'cargo check',
        { cwd: tauriDir, timeout: 300_000 }
      );
      record('Rust (cargo check)', res.ok, res.duration, res.ok ? null : res.output);
    } else {
      console.log(`  ${SKIP} Rust check (src-tauri/ not found)`);
      results.push({ name: 'Rust check', passed: true, duration: '0.0', detail: null });
    }
  }
}

// --- 5. Frontend tests ---
if (!QUICK) {
  header('Frontend Tests');
  {
    const res = run(
      'vitest',
      'npx vitest run',
      { timeout: 300_000, env: { NODE_OPTIONS: '--max-old-space-size=4096' } }
    );
    record('Frontend tests (Vitest)', res.ok, res.duration, res.ok ? null : res.output);
  }
}

// --- 6. File size limits ---
header('File Size Limits');
{
  const res = run('sizes', 'node scripts/check-file-sizes.cjs');
  record('File size limits', res.ok, res.duration, res.ok ? null : res.output);
}

// --- 7. IPC contract validation ---
header('IPC Contract Validation');
{
  const res = run('ipc', 'node scripts/validate-commands.cjs');
  // validate-commands.cjs always exits 0, so check output for "MISMATCH" or "ghost"
  const hasIssues = res.output && /ghost|MISMATCH|UNREGISTERED/i.test(res.output);
  const passed = res.ok && !hasIssues;
  record('IPC contracts', passed, res.duration, passed ? null : res.output);
}

// ============================================================================
// Verdict
// ============================================================================

header('Verdict');

const failed = results.filter((r) => !r.passed);
const totalDuration = results.reduce((sum, r) => sum + parseFloat(r.duration), 0).toFixed(1);

if (failed.length === 0) {
  console.log(`  ALL CHECKS PASSED (${results.length}/${results.length}) in ${totalDuration}s`);
  console.log('  Safe to deploy.');
  console.log('');
  process.exit(0);
} else {
  console.log(`  ${failed.length} CHECK(S) FAILED (${results.length - failed.length}/${results.length} passed) in ${totalDuration}s`);
  console.log('');
  console.log('  Failed:');
  for (const f of failed) {
    console.log(`    - ${f.name}`);
  }
  console.log('');
  console.log('  Fix the failures above before deploying.');
  console.log('');
  process.exit(1);
}
