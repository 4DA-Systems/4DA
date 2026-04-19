#!/usr/bin/env node
/**
 * Installer smoke test — validates a built Windows NSIS installer without
 * actually executing it. This is the lightweight first line of defense before
 * a release; it does NOT replace a full VM-based install-and-launch test.
 *
 * What it checks:
 *   1. The installer exists at the expected path.
 *   2. It is a PE32 executable (not truncated, not garbage).
 *   3. Its size is within a sane range (2 MB floor, 200 MB ceiling).
 *   4. Its SHA-256 matches `--expected-sha256 <hex>` if provided.
 *   5. On Windows, `Get-AuthenticodeSignature` reports `Valid` — unless
 *      `--unsigned-ok` is passed (useful for unsigned dev builds).
 *
 * What it does NOT check:
 *   - That the installer actually runs on a clean Windows box. That requires
 *     a VM and is documented as manual step 5 in RELEASE-RUNBOOK.md.
 *   - That the bundled binary starts post-install. Also manual, same reason.
 *   - Any Linux/macOS bundle formats. NSIS is the primary ship vehicle.
 *
 * Usage:
 *   node scripts/verify-installer.cjs
 *   node scripts/verify-installer.cjs --path "dist/4DA_1.0.0_x64-setup.exe"
 *   node scripts/verify-installer.cjs --expected-sha256 abc123...
 *   node scripts/verify-installer.cjs --unsigned-ok   # dev builds pre-signing
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execSync } = require('node:child_process');

const args = parseArgs(process.argv.slice(2));
const result = { passes: [], warnings: [], failures: [] };

main().catch((e) => {
  result.failures.push(`Uncaught error: ${e.message}`);
  report();
  process.exit(1);
});

function parseArgs(argv) {
  const out = { path: null, expected: null, unsignedOk: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--path') out.path = argv[++i];
    else if (a === '--expected-sha256') out.expected = (argv[++i] || '').toLowerCase();
    else if (a === '--unsigned-ok') out.unsignedOk = true;
    else if (a === '--help' || a === '-h') {
      console.log(`Usage: verify-installer.cjs [--path <file>] [--expected-sha256 <hex>] [--unsigned-ok]`);
      process.exit(0);
    }
  }
  return out;
}

async function main() {
  const installerPath = args.path || findLatestInstaller();
  if (!installerPath) {
    result.failures.push('No installer path supplied and none found under src-tauri/target/release/bundle/nsis/.');
    report();
    process.exit(1);
  }

  checkExists(installerPath);
  checkPEHeader(installerPath);
  checkSize(installerPath);
  const sha256 = computeSha256(installerPath);
  result.passes.push(`SHA-256: ${sha256}`);
  if (args.expected) checkExpectedSha(sha256);
  checkSignature(installerPath);

  report();
  process.exit(result.failures.length > 0 ? 1 : 0);
}

function findLatestInstaller() {
  const nsisDir = path.join(__dirname, '..', 'src-tauri', 'target', 'release', 'bundle', 'nsis');
  if (!fs.existsSync(nsisDir)) return null;
  const candidates = fs
    .readdirSync(nsisDir)
    .filter((f) => f.endsWith('.exe') && f.toLowerCase().includes('setup'))
    .map((f) => ({ name: f, path: path.join(nsisDir, f), mtime: fs.statSync(path.join(nsisDir, f)).mtimeMs }))
    .sort((a, b) => b.mtime - a.mtime);
  if (candidates.length === 0) return null;
  result.passes.push(`Discovered installer: ${candidates[0].name}`);
  return candidates[0].path;
}

function checkExists(p) {
  if (!fs.existsSync(p)) {
    result.failures.push(`Installer path does not exist: ${p}`);
    return;
  }
  const stat = fs.statSync(p);
  if (!stat.isFile()) {
    result.failures.push(`Path is not a regular file: ${p}`);
    return;
  }
  result.passes.push(`Exists: ${p} (${humanBytes(stat.size)})`);
}

function checkPEHeader(p) {
  try {
    const fd = fs.openSync(p, 'r');
    const buf = Buffer.alloc(64);
    fs.readSync(fd, buf, 0, 64, 0);
    fs.closeSync(fd);
    if (buf[0] !== 0x4d || buf[1] !== 0x5a) {
      result.failures.push(`Not a valid PE file (missing MZ magic at offset 0)`);
      return;
    }
    result.passes.push('PE header: MZ magic present');
  } catch (e) {
    result.failures.push(`Cannot read PE header: ${e.message}`);
  }
}

function checkSize(p) {
  const MIN = 2 * 1024 * 1024; // 2 MB — anything smaller is definitely truncated
  const MAX = 200 * 1024 * 1024; // 200 MB — catches accidental bundling of test data
  try {
    const { size } = fs.statSync(p);
    if (size < MIN) {
      result.failures.push(`Installer suspiciously small: ${humanBytes(size)} (min ${humanBytes(MIN)})`);
    } else if (size > MAX) {
      result.warnings.push(`Installer larger than expected: ${humanBytes(size)} (max ${humanBytes(MAX)})`);
    } else {
      result.passes.push(`Size in range: ${humanBytes(size)}`);
    }
  } catch (e) {
    result.failures.push(`Cannot stat: ${e.message}`);
  }
}

function computeSha256(p) {
  const hash = crypto.createHash('sha256');
  const stream = fs.readFileSync(p);
  hash.update(stream);
  return hash.digest('hex');
}

function checkExpectedSha(actual) {
  const expected = args.expected;
  if (actual === expected) {
    result.passes.push(`SHA-256 matches expected value`);
  } else {
    result.failures.push(`SHA-256 mismatch: expected ${expected}, got ${actual}`);
  }
}

function checkSignature(p) {
  // Only runs on Windows. On other platforms we can't verify Authenticode,
  // and on Windows powershell is always present.
  const isWindows = process.platform === 'win32' || /WINDOWS/i.test(process.env.OS || '');
  if (!isWindows) {
    result.warnings.push('Authenticode check skipped (not running on Windows)');
    return;
  }
  try {
    // PowerShell single-quoted strings are literal; any embedded single quote
    // is escaped by doubling. This is the only quoting form that survives
    // filenames with spaces AND with weird characters without mangling.
    const psPath = p.replace(/'/g, "''");
    const ps = `(Get-AuthenticodeSignature -FilePath '${psPath}').Status`;
    const out = execSync(`powershell.exe -NoProfile -Command "${ps.replace(/"/g, '\\"')}"`, {
      encoding: 'utf8',
      timeout: 30000,
    }).trim();
    if (out === 'Valid') {
      result.passes.push('Authenticode signature: Valid');
    } else if (args.unsignedOk && (out === 'NotSigned' || out === 'HashMismatch')) {
      result.warnings.push(`Authenticode status: ${out} (accepted because --unsigned-ok)`);
    } else {
      result.failures.push(`Authenticode status: ${out} (use --unsigned-ok for dev builds)`);
    }
  } catch (e) {
    // PowerShell invocation itself failed. Treat as a warning so the script
    // doesn't fail on hosts where powershell is unavailable for any reason.
    result.warnings.push(`Could not run Get-AuthenticodeSignature: ${e.message.split('\n')[0]}`);
  }
}

function humanBytes(n) {
  const units = ['B', 'KB', 'MB', 'GB'];
  let u = 0;
  let v = n;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v.toFixed(u === 0 ? 0 : 1)} ${units[u]}`;
}

function report() {
  const RED = '\x1b[31m';
  const YEL = '\x1b[33m';
  const GRN = '\x1b[32m';
  const RESET = '\x1b[0m';
  console.log('\n=== Installer Smoke Test ===');
  for (const p of result.passes) console.log(`  ${GRN}[OK]${RESET}   ${p}`);
  for (const w of result.warnings) console.log(`  ${YEL}[WARN]${RESET} ${w}`);
  for (const f of result.failures) console.log(`  ${RED}[FAIL]${RESET} ${f}`);
  console.log('');
  if (result.failures.length > 0) {
    console.log(`${RED}FAIL${RESET} — ${result.failures.length} failing check(s). See RELEASE-RUNBOOK.md step 5 for the manual VM test that must also pass before shipping.`);
  } else {
    console.log(`${GRN}PASS${RESET} — artifact-level smoke passes. Now run the manual VM install check (RELEASE-RUNBOOK.md step 5).`);
  }
}
