#!/usr/bin/env node
/**
 * Audit EVERY lockfile git tracks, discovered — never enumerated.
 *
 * Why discovery: the two audit loops this replaces (validate.yml "Repo guards"
 * and nightly-audit.yml) both hardcoded the same four directories
 * (`.` `site` `paddle-webhook` `mcp-4da-server`). The repo tracks SIX
 * lockfiles. `editors/vscode/4da/package-lock.json` and
 * `mcp-memory-server/package-lock.json` are npm, not pnpm, so neither loop
 * ever saw them — and on 2026-09-09 the vscode extension was carrying
 * js-yaml 4.3.1 (GHSA-2883-xcg3-v3hh, high) and qs 6.15.2 unaudited. 4DA's
 * own Preemption tab is what surfaced them, not 4DA's CI.
 *
 * Enumerating the things to check is how a gate goes quietly out of date: a
 * new workspace, or a lockfile in another package manager's format, is
 * unaudited by default and nothing says so. Discovery inverts that — a new
 * tracked lockfile is audited from the commit that adds it.
 *
 * Tooling per format:
 *   pnpm-lock.yaml    -> `npx -y pnpm@<pin> --dir <d> audit [--prod]`
 *                        npm retired the legacy audit endpoint 2026-07-14
 *                        (HTTP 410), so pnpm <=10 cannot audit at all; only
 *                        pnpm 11+ speaks the bulk advisory endpoint. A one-shot
 *                        via npx, leaving the repo's 9.15.0 pin untouched.
 *   package-lock.json -> `npm audit` in that directory. Reads the lockfile
 *                        alone; no `node_modules` and no install required
 *                        (verified 2026-09-09 on a bare copy).
 *   yarn.lock         -> not present in this repo. Discovered ones are
 *                        reported UNSUPPORTED and fail, rather than skipped
 *                        silently — a gate that shrugs is the bug above.
 *
 * The ROOT pnpm lockfile keeps `--prod`, matching the step this replaces. The
 * root's dev tree is audited separately by the Frontend job (`pnpm audit`
 * without `--prod`), and that split is deliberate: a dev-only advisory should
 * not block the merge gate that guards shipped code.
 *
 * Usage:  node scripts/audit-lockfiles.cjs [--level=high] [--json]
 * Exit:   0 all clean · 1 advisories at/above the level · 2 harness failure
 */

const { execFileSync, spawnSync } = require('node:child_process');
const path = require('node:path');

const PNPM_AUDIT_PIN = process.env.AUDIT_PNPM_PIN || 'pnpm@11.13.0';
const REPO_ROOT = path.resolve(__dirname, '..');

const args = process.argv.slice(2);
const levelArg = args.find((a) => a.startsWith('--level='));
const LEVEL = levelArg ? levelArg.slice('--level='.length) : 'high';
const AS_JSON = args.includes('--json');

/** Lockfile basenames we know how to audit, mapped to their ecosystem. */
const KNOWN = new Map([
  ['pnpm-lock.yaml', 'pnpm'],
  ['package-lock.json', 'npm'],
  ['yarn.lock', 'yarn'],
]);

/**
 * Every lockfile git tracks, as repo-relative POSIX paths.
 *
 * `git ls-files` (not a filesystem walk) on purpose: it is the definition of
 * "in the repo", so a gitignored local lockfile never fails CI, and a tracked
 * one can never be missed.
 */
function discoverLockfiles() {
  const out = execFileSync('git', ['ls-files'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  return out
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => KNOWN.has(path.posix.basename(l)))
    .sort();
}

/** The audit command for one discovered lockfile. */
function commandFor(lockfile) {
  const dir = path.posix.dirname(lockfile) === '.' ? '.' : path.posix.dirname(lockfile);
  const ecosystem = KNOWN.get(path.posix.basename(lockfile));
  const isRoot = dir === '.';

  if (ecosystem === 'pnpm') {
    // Root: production closure only — see the header note on the split.
    const flags = isRoot ? ['--prod'] : [];
    return {
      dir,
      ecosystem,
      file: 'npx',
      args: ['-y', PNPM_AUDIT_PIN, '--dir', dir, 'audit', ...flags, `--audit-level=${LEVEL}`],
      // `--dir` is resolved against the CWD, so pnpm runs from the repo root —
      // passing both `cwd: dir` and `--dir dir` looks for `site/site`.
      cwd: '.',
      label: `pnpm audit ${dir}${isRoot ? ' (--prod)' : ''}`,
    };
  }
  if (ecosystem === 'npm') {
    return {
      dir,
      ecosystem,
      file: 'npm',
      args: ['audit', `--audit-level=${LEVEL}`],
      // npm has no `--dir`; it audits the lockfile in its CWD.
      cwd: dir,
      label: `npm audit ${dir}`,
    };
  }
  return { dir, ecosystem, unsupported: true, cwd: dir, label: `${ecosystem} audit ${dir}` };
}

function runAudit(cmd) {
  if (cmd.unsupported) {
    return {
      ...cmd,
      code: 1,
      output: `No audit tool wired for ${cmd.ecosystem} lockfiles. Add one to scripts/audit-lockfiles.cjs — do not skip it.`,
    };
  }
  const res = spawnSync(cmd.file, cmd.args, {
    cwd: path.join(REPO_ROOT, cmd.cwd),
    encoding: 'utf8',
    shell: process.platform === 'win32',
    env: { ...process.env, pnpm_config_pm_on_fail: 'ignore' },
    maxBuffer: 32 * 1024 * 1024,
  });
  const output = `${res.stdout || ''}${res.stderr || ''}`.trim();
  // A spawn that never ran (missing tool) is a harness failure, not a clean
  // audit — never let it read as "0 vulnerabilities".
  if (res.error) {
    return { ...cmd, code: 2, output: `failed to run ${cmd.file}: ${res.error.message}` };
  }
  const code = res.status === null ? 2 : res.status;
  // Both tools exit 1 for "found advisories" AND for "I could not run" — a
  // bad path, a missing lockfile, a usage error. Reporting the second as the
  // first sends the reader to bump a dependency that was never the problem
  // (seen while building this: a `--dir` passed twice printed
  // `ENOENT … 'site/site'` under the heading "advisories in site").
  if (code !== 0 && looksLikeHarnessError(output)) {
    return { ...cmd, code: 2, output };
  }
  return { ...cmd, code, output };
}

/**
 * Did the tool fail to AUDIT, rather than find something? An audit that ran
 * always says how many vulnerabilities it found; one that could not run says
 * ENOENT, "not found", or prints its own usage.
 */
function looksLikeHarnessError(output) {
  if (/vulnerabilit(y|ies)|No known vulnerabilities/i.test(output)) return false;
  return /ENOENT|ERR_|command not found|is not recognized|no such file|Usage:|help audit|Cannot find module/i.test(
    output,
  );
}

/**
 * The advisory lines worth putting in a GitHub annotation. Annotations are
 * single-line, so newlines fold into the literal `%0A` the Actions viewer
 * renders as a break — without this the annotation is silently truncated at
 * the first newline.
 */
function annotationBody(output) {
  const lines = output
    .split('\n')
    .filter((l) => /Severity:|vulnerabilities found|vulnerabilities \(|^\s*Package |Patched versions|https:\/\/github\.com\/advisories\//.test(l));
  return (lines.length ? lines : output.split('\n').slice(-12))
    .join('\n')
    .slice(-900)
    .replace(/\n/g, '%0A');
}

function main() {
  const lockfiles = discoverLockfiles();
  if (lockfiles.length === 0) {
    console.error('[audit-lockfiles] no tracked lockfiles found — refusing to report success.');
    process.exit(2);
  }

  const results = lockfiles.map((f) => runAudit(commandFor(f)));

  if (AS_JSON) {
    console.log(JSON.stringify({ level: LEVEL, results: results.map((r) => ({ dir: r.dir, ecosystem: r.ecosystem, code: r.code })) }, null, 2));
  }

  let failed = 0;
  let harness = 0;
  for (const r of results) {
    console.log(`\n=== ${r.label} — exit ${r.code} ===`);
    console.log(r.output);
    if (r.code === 2) harness += 1;
    if (r.code !== 0) {
      failed += 1;
      const kind = r.code === 2 ? 'audit harness failure' : 'advisories';
      console.log(`::error title=${kind} in ${r.dir}::${annotationBody(r.output)}`);
    }
  }

  const audited = results.map((r) => `${r.dir} (${r.ecosystem})`).join(', ');
  console.log(`\n[audit-lockfiles] ${results.length} tracked lockfile(s) audited at --audit-level=${LEVEL}: ${audited}`);

  if (harness > 0) {
    console.log('::error::An audit could not run. A gate that cannot execute is not a passing gate.');
    process.exit(2);
  }
  if (failed > 0) {
    console.log(
      '::error::Advisories at or above the level. For a pnpm lockfile raise that project\'s pnpm override floor and regenerate with the PINNED pnpm (9.15.0) — pnpm 11 is audit-only and its install drops the overrides block. For an npm lockfile run `npm audit fix --package-lock-only` in that directory.',
    );
    process.exit(1);
  }
  console.log(`[audit-lockfiles] all clean at --audit-level=${LEVEL}.`);
}

if (require.main === module) main();

module.exports = { discoverLockfiles, commandFor, annotationBody, looksLikeHarnessError, KNOWN };
