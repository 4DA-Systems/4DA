#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/*
 * verify-site-build.cjs — the pre-push "Verifying site build" step.
 *
 * WHY THIS EXISTS
 *   The hook used to run `(cd site && npx @11ty/eleventy --dryrun)`. In a fresh
 *   worktree (no site/node_modules) npx fetched whatever 3.x was newest from the
 *   registry instead of the locked version, and `--dryrun` always reports
 *   "Wrote 0 files" — so the step printed a success-shaped line that proved
 *   nothing, and could not tell a working build from one that produced no pages.
 *
 * WHAT IT DOES NOW
 *   - site deps installed            -> real build (locked Eleventy) into a temp dir,
 *                                       fail unless it wrote at least one file.
 *   - deps missing, site/ changed    -> `pnpm install --frozen-lockfile` in site/
 *                                       (~10 s), then the same real build.
 *   - deps missing, site/ unchanged  -> explicit "skipped: site deps not installed"
 *                                       line. Nothing this push touches can break it.
 *
 * Usage: node scripts/verify-site-build.cjs [--site-changed]
 * Exit 0 = built (or explicitly skipped), 1 = build failed or wrote 0 files.
 */
'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const SITE_DIR = path.resolve(__dirname, '..', 'site');

/** Pulls N out of Eleventy's summary line ("[11ty] Copied 22 Wrote 128 files in ..."). */
function parseWrittenCount(output) {
  const matches = [...String(output).matchAll(/Wrote (\d+) files?/g)];
  if (matches.length === 0) return null;
  return Number(matches[matches.length - 1][1]);
}

/** Decides what the step does: 'build', 'install-then-build' or 'skip'. */
function decide({ depsInstalled, siteChanged }) {
  if (depsInstalled) return 'build';
  return siteChanged ? 'install-then-build' : 'skip';
}

function eleventyBin(siteDir) {
  const name = process.platform === 'win32' ? 'eleventy.cmd' : 'eleventy';
  return path.join(siteDir, 'node_modules', '.bin', name);
}

function run(cmd, args, cwd) {
  return spawnSync(cmd, args, {
    cwd,
    encoding: 'utf8',
    windowsHide: true,
    // .cmd shims on Windows need a shell to launch.
    shell: process.platform === 'win32',
  });
}

function buildSite(siteDir) {
  const out = fs.mkdtempSync(path.join(os.tmpdir(), '4da-site-verify-'));
  try {
    const res = run(eleventyBin(siteDir), ['--quiet', `--output=${out}`], siteDir);
    const output = `${res.stdout || ''}${res.stderr || ''}`;
    if (res.status !== 0) {
      process.stdout.write(output);
      console.error('Site build failed. Fix site errors before pushing.');
      return 1;
    }
    const written = parseWrittenCount(output);
    if (!written) {
      process.stdout.write(output);
      console.error(
        `Site build wrote ${written === null ? 'an unknown number of' : '0'} files — ` +
          'expected the site pages. Check site/eleventy.config.js input/output dirs.',
      );
      return 1;
    }
    console.log(`Site build OK: wrote ${written} files.`);
    return 0;
  } finally {
    fs.rmSync(out, { recursive: true, force: true });
  }
}

function main(argv) {
  if (!fs.existsSync(path.join(SITE_DIR, 'package.json'))) {
    console.log('Site build: skipped (no site/package.json).');
    return 0;
  }
  const action = decide({
    depsInstalled: fs.existsSync(eleventyBin(SITE_DIR)),
    siteChanged: argv.includes('--site-changed'),
  });
  if (action === 'skip') {
    console.log(
      'Site build: skipped: site deps not installed and this push does not touch site/ ' +
        '(run `pnpm --dir site install --frozen-lockfile` to verify it anyway).',
    );
    return 0;
  }
  if (action === 'install-then-build') {
    console.log('Site build: installing site deps (frozen lockfile)...');
    const res = run('pnpm', ['install', '--frozen-lockfile', '--silent'], SITE_DIR);
    if (res.status !== 0) {
      process.stdout.write(`${res.stdout || ''}${res.stderr || ''}`);
      console.error('Site deps failed to install, so the site build cannot be verified.');
      return 1;
    }
  }
  return buildSite(SITE_DIR);
}

if (require.main === module) {
  process.exit(main(process.argv.slice(2)));
}

module.exports = { parseWrittenCount, decide };
