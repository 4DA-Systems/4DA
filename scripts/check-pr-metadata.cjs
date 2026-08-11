#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * check-pr-metadata.cjs — close the squash-merge leak path.
 *
 * WHY THIS EXISTS
 * `.husky/commit-msg` guards locally authored commit messages and
 * `.husky/pre-push` guards outgoing commits. Neither sees a **squash merge**:
 * GitHub composes that commit message server-side, from the PR title and body
 * (and, depending on repo settings, the list of commit messages). No local hook
 * runs at that moment.
 *
 * That is not hypothetical. Two commits on public `main` — `d5df9e34` (#409)
 * and `053e5813` (#362) — carry the private external-verifier name in their
 * MESSAGES. A scan of all 1,948 tracked files at `origin/main` found zero
 * flagged files, so the 2026-07-13 content scrub held; only this path
 * regressed. Removing those messages now needs a public-history rewrite.
 *
 * This runs in CI on the pull_request event, where the title/body are known
 * BEFORE the merge button is pressed, and fails the required gate if any of
 * them would carry the name into a squash commit.
 *
 * FAIL-CLOSED, deliberately. `.husky/pre-push` fails OPEN on tool error so a
 * broken guard can never brick the fleet's ability to push. CI is different: a
 * merge is blocked, not a developer's local loop, the failure is visible on the
 * PR, and re-running is cheap. A guard that silently cannot run is not a guard
 * — and silent-failure is the exact bug class this whole check exists to stop.
 *
 * Detection is delegated to scripts/private-asset-guard.cjs, which keeps the
 * name hashed; no literal appears here or in the tests.
 */
'use strict';

/**
 * Scan the parts of a pull request that can end up inside a squash-merge
 * commit message.
 *
 * Pure and injectable so tests never need the real (private) pattern.
 *
 * @param {{title?: string, body?: string, commitMessages?: string[]}} pr
 * @param {(text: string) => boolean} scan returns true when text is flagged
 * @returns {string[]} human-readable locations that were flagged (empty = clean)
 */
function scanPullRequestMetadata(pr, scan) {
  const findings = [];
  const title = typeof pr.title === 'string' ? pr.title : '';
  const body = typeof pr.body === 'string' ? pr.body : '';
  const commits = Array.isArray(pr.commitMessages) ? pr.commitMessages : [];

  if (title && scan(title)) findings.push('PR title');
  if (body && scan(body)) findings.push('PR body');

  commits.forEach((message, i) => {
    if (typeof message === 'string' && message && scan(message)) {
      const subject = message.split('\n', 1)[0].slice(0, 60);
      findings.push(`commit message #${i + 1} ("${subject}")`);
    }
  });

  return findings;
}

module.exports = { scanPullRequestMetadata };

// ── CLI ────────────────────────────────────────────────────────────────────
if (require.main === module) {
  const { execFileSync } = require('node:child_process');
  const path = require('node:path');

  // Title and body arrive via the ENVIRONMENT, never interpolated into a shell
  // command — a PR title is attacker-controlled text and `${{ }}` inlined into
  // a `run:` block is a script-injection primitive.
  const title = process.env.PR_TITLE || '';
  const body = process.env.PR_BODY || '';
  const base = process.env.PR_BASE_SHA || '';
  const head = process.env.PR_HEAD_SHA || '';

  let commitMessages = [];
  if (base && head) {
    try {
      const out = execFileSync(
        'git',
        ['log', '--format=%B%x00', `${base}..${head}`],
        { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }
      );
      commitMessages = out.split('\0').map((s) => s.trim()).filter(Boolean);
    } catch (err) {
      console.error(`check-pr-metadata: could not read commit range: ${err.message}`);
      process.exit(1); // fail-closed
    }
  }

  let scanText;
  try {
    ({ scanText } = require(path.join(__dirname, 'private-asset-guard.cjs')));
    if (typeof scanText !== 'function') throw new Error('scanText is not a function');
  } catch (err) {
    console.error(
      `check-pr-metadata: cannot load the private-asset guard (${err.message}). ` +
        'Refusing to pass a check that did not actually run.'
    );
    process.exit(1); // fail-closed
  }

  let findings;
  try {
    findings = scanPullRequestMetadata({ title, body, commitMessages }, scanText);
  } catch (err) {
    console.error(`check-pr-metadata: scan failed (${err.message}).`);
    process.exit(1); // fail-closed
  }

  if (findings.length > 0) {
    console.error('');
    console.error('============================================================');
    console.error('  MERGE BLOCKED — private reference in PR metadata');
    console.error('============================================================');
    console.error('  A squash merge builds its commit message from the PR title');
    console.error('  and body, so this would land in PUBLIC history where no');
    console.error('  local hook can catch it (and removing it later needs a');
    console.error('  history rewrite).');
    console.error('');
    console.error('  Flagged:');
    for (const f of findings) console.error(`    - ${f}`);
    console.error('');
    console.error('  Edit the PR title/body (or reword the commit) and re-run.');
    console.error('');
    process.exit(1);
  }

  console.log(
    `check-pr-metadata: clean (title, body, ${commitMessages.length} commit message(s)).`
  );
}
