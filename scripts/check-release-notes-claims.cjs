#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * check-release-notes-claims.cjs — AD-030 enforcement for GitHub RELEASE BODIES.
 *
 * `check-retired-claims.cjs` scans `git ls-files`. A GitHub release body is not
 * a tracked file, so it sits outside that gate — and outside every other gate
 * in this repo.
 *
 * That gap was not theoretical. On 2026-08-28 the v1.0.0 release notes — the
 * page 4da.ai/download sends every visitor to, and the most-read public text
 * the project has — still opened with "gets sharper every day" and "It learns
 * from how you engage with what it shows you", four months after AD-030 retired
 * both. The repo was clean; the shopfront was not.
 *
 * Reuses the pattern list from `check-retired-claims.cjs` so the two can never
 * disagree about what is retired.
 *
 * Network access is required, so this SKIPS (exit 0, loudly) when no token is
 * available — a fork or an offline `pnpm run validate` must not fail on a check
 * it structurally cannot run. In CI the default GITHUB_TOKEN is enough.
 */

const { execFileSync } = require('node:child_process');
const { PATTERNS } = require('./check-retired-claims.cjs');

const OWNER = '4DA-Systems';
const REPO = '4DA';

/**
 * Scan a list of releases. Pure — no network — so the test can drive it.
 *
 * @param {Array<{tag_name?: string, name?: string, body?: string, draft?: boolean}>} releases
 * @returns {Array<{tag: string, field: string, phrase: string, excerpt: string}>}
 */
function scanReleases(releases) {
  const findings = [];
  for (const release of releases || []) {
    const tag = release.tag_name || release.name || '(untagged)';
    for (const field of ['name', 'body']) {
      const text = release[field];
      if (!text) continue;
      const hits = [];
      for (const pattern of PATTERNS) {
        // PATTERNS may carry /g from the sibling module; reset before reuse so
        // a lastIndex left over from a previous call cannot make a later scan
        // miss a hit. Pinned by "every release in a list is scanned".
        pattern.lastIndex = 0;
        const match = pattern.exec(text);
        if (match) hits.push({ index: match.index, text: match[0] });
      }
      // Several patterns deliberately overlap: "gets sharper every day" also
      // matches the looser "sharper every day". Report the widest match at each
      // position once, so two DISTINCT claims still read as two findings while
      // one claim does not read as two.
      const widest = hits.filter(
        (h) =>
          !hits.some(
            (o) =>
              o !== h &&
              o.index <= h.index &&
              o.index + o.text.length >= h.index + h.text.length &&
              o.text.length > h.text.length
          )
      );
      for (const hit of widest) {
        const at = Math.max(0, hit.index - 40);
        findings.push({
          tag,
          field,
          phrase: hit.text,
          excerpt: text.slice(at, hit.index + hit.text.length + 40).replace(/\s+/g, ' '),
        });
      }
    }
  }
  return findings;
}

function fetchReleases() {
  // `gh` is present in CI and on the maintainer's machine, and handles auth,
  // pagination and the API version. Falling back to it rather than hand-rolling
  // a fetch keeps one auth path instead of two.
  const out = execFileSync(
    'gh',
    ['api', `repos/${OWNER}/${REPO}/releases?per_page=100`, '--paginate'],
    { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }
  );
  // `--paginate` concatenates JSON arrays; normalise both shapes.
  const chunks = out.replace(/\]\s*\[/g, ',').trim();
  return JSON.parse(chunks || '[]');
}

function main() {
  let releases;
  try {
    releases = fetchReleases();
  } catch (err) {
    console.log(
      '[check-release-notes-claims] SKIPPED — could not reach the GitHub API ' +
        `(${String(err.message || err).split('\n')[0]}). This check needs network + auth; ` +
        'it runs for real in CI.'
    );
    return 0;
  }

  const findings = scanReleases(releases);
  if (findings.length === 0) {
    console.log(
      `[check-release-notes-claims] OK — ${releases.length} release(s) scanned, no retired promise language.`
    );
    return 0;
  }

  console.error('[check-release-notes-claims] RETIRED CLAIMS ARE LIVE ON PUBLISHED RELEASES:\n');
  for (const f of findings) {
    console.error(`  ${f.tag} (${f.field}): "${f.phrase}"`);
    console.error(`    ...${f.excerpt}...`);
  }
  console.error(
    '\nA release body is public copy. Fix it with:\n' +
      '  gh release edit <tag> --notes-file <corrected.md>\n' +
      'AD-030 is in .ai/DECISIONS.md; the phrase list is in check-retired-claims.cjs.'
  );
  return 1;
}

module.exports = { scanReleases };

if (require.main === module) process.exit(main());
