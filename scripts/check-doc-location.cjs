#!/usr/bin/env node
/**
 * Doc-location gate — pre-commit hook.
 *
 * Rejects any *.md staged at the repo root whose name matches an internal
 * planning-doc pattern (PLAN / STRATEGY / AUDIT / CHECKLIST / ROADMAP /
 * TRAJECTORY / VIRAL / LAUNCH / FORTIFICATION / whats-next / NEXT-STEPS /
 * etc), unless:
 *   1. The filename is on the public allowlist below, OR
 *   2. The first 10 lines of the file contain `<!-- public-ok: <reason> -->`
 *
 * Canonical homes for internal planning docs:
 *   - .claude/plans/       (session-local, gitignored)
 *   - docs/strategy/       (curated, user-approved, may be tracked)
 *   - docs/private/        (never tracked)
 *
 * This is Layer 3 of the Document Hygiene framework. See:
 *   .claude/rules/document-hygiene.md
 */

const { execSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const GREEN = '\x1b[32m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

// Files at repo root that are legitimately public-facing markdown.
// Anything NOT on this list + matching a block pattern is rejected.
const PUBLIC_ALLOWLIST = new Set([
  'README.md',
  'CHANGELOG.md',
  'LICENSE.md',
  'CONTRIBUTING.md',
  'CODE_OF_CONDUCT.md',
  'SECURITY.md',
  'CLAUDE.md',
  'AGENTS.md',
  'CONVENTIONS.md',
  'TRADEMARKS.md',
  'CLA.md',
  'LINUX.md',
  'NETWORK.md',
]);

// Filename patterns (case-insensitive) that signal an internal planning doc.
// If the filename matches ANY of these AND it is at repo root AND not on the
// allowlist, the commit is rejected.
const BLOCK_PATTERNS = [
  /PLAN\.md$/i,
  /-PLAN\.md$/i,
  /^PLAN-/i,
  /STRATEGY\.md$/i,
  /-STRATEGY-/i,
  /AUDIT\.md$/i,
  /-AUDIT-/i,
  /CHECKLIST\.md$/i,
  /ROADMAP\.md$/i,
  /TRAJECTORY\.md$/i,
  /^TRAJECTORY-/i,
  /^VIRAL-/i,
  /^LAUNCH-/i,
  /^PRE-LAUNCH-/i,
  /^FIRST-CONTACT-/i,
  /^FORTIFICATION-/i,
  /^EXECUTION-/i,
  /^ASCENT-/i,
  /^BATTLE-/i,
  /^MASTER-/i,
  /^BARRIER-/i,
  /^IMPROVEMENTS-/i,
  /^CRITICAL-/i,
  /^DEVELOPER-OS-/i,
  /^NOTIFICATION-INTELLIGENCE-/i,
  /^INTELLIGENCE-CONSOLE-/i,
  /^whats-next\.md$/i,
  /^WHATS-NEXT\.md$/i,
  /^NEXT-STEPS\.md$/i,
  /^MISSION_/i,
  /^SHIP_/i,
];

const PUBLIC_OK_MARKER = /<!--\s*public-ok\s*:\s*.+?-->/i;

function isBlockedName(filename) {
  if (PUBLIC_ALLOWLIST.has(filename)) return false;
  return BLOCK_PATTERNS.some((re) => re.test(filename));
}

function hasPublicOkMarker(filepath) {
  try {
    const head = fs.readFileSync(filepath, 'utf8').split('\n').slice(0, 10).join('\n');
    return PUBLIC_OK_MARKER.test(head);
  } catch {
    return false;
  }
}

function getStagedFiles() {
  const raw = execSync('git diff --cached --name-only --diff-filter=ACM', {
    encoding: 'utf8',
  });
  return raw.split('\n').filter(Boolean);
}

function main() {
  let staged;
  try {
    staged = getStagedFiles();
  } catch (err) {
    console.error(`${YELLOW}doc-location gate: failed to list staged files (${err.message}). Skipping.${RESET}`);
    return 0;
  }

  const violations = [];
  for (const file of staged) {
    // Only care about files directly at repo root (no slash in path).
    if (file.includes('/')) continue;
    if (!file.endsWith('.md')) continue;
    if (!isBlockedName(file)) continue;
    if (hasPublicOkMarker(file)) continue;
    violations.push(file);
  }

  if (violations.length === 0) {
    return 0;
  }

  console.error('');
  console.error(`${RED}${BOLD}✗ Doc-location gate: internal planning doc(s) at repo root${RESET}`);
  console.error('');
  for (const file of violations) {
    console.error(`  ${RED}${file}${RESET}`);
  }
  console.error('');
  console.error(`${BOLD}Internal planning / strategy / audit / checklist / roadmap docs${RESET}`);
  console.error('must not live at the repo root. Canonical homes:');
  console.error('');
  console.error(`  ${GREEN}.claude/plans/${RESET}       session-local, gitignored (default home)`);
  console.error(`  ${GREEN}docs/strategy/${RESET}       curated, reviewed, may be tracked`);
  console.error(`  ${GREEN}docs/private/${RESET}        never tracked`);
  console.error('');
  console.error(`${BOLD}How to fix:${RESET}`);
  console.error('  1. Move the file to one of the locations above, OR');
  console.error(`  2. If this IS a public-facing doc, add to the first 10 lines:`);
  console.error(`       ${YELLOW}<!-- public-ok: <one-line reason> -->${RESET}`);
  console.error('     and re-stage. The gate will allow it.');
  console.error('');
  console.error(`${BOLD}Why this exists:${RESET}`);
  console.error('  Internal plans leaking to repo root is a recurring pattern. See');
  console.error('  .claude/rules/document-hygiene.md for the full framework.');
  console.error('');
  return 1;
}

process.exit(main());
