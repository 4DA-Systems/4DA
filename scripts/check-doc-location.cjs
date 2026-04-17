#!/usr/bin/env node
/**
 * Doc-location + PII gate — pre-commit hook.
 *
 * Three checks, all run on the staged file set:
 *
 *   1. Root-level block: rejects any *.md at repo root whose filename matches
 *      an internal-plan pattern AND is not on the root allowlist, unless the
 *      file contains a `<!-- public-ok: <reason> -->` marker.
 *
 *   2. Mixed-directory allowlist: for directories that legitimately contain a
 *      mix of public and internal docs (.ai/, docs/strategy/), only files on
 *      the per-directory allowlist may be tracked. Anything else is rejected.
 *      The allowlist lives in scripts/doc-allowlist.json.
 *
 *   3. PII sweep: rejects any staged file containing personal email addresses
 *      or similar PII that must not enter the public git history.
 *
 * Canonical homes for internal planning / strategy / audit docs:
 *   - .claude/plans/       (session-local, gitignored)
 *   - docs/strategy/<ALLOWLISTED>   (curated architecture)
 *   - docs/private/        (never tracked)
 *
 * Full framework: .claude/rules/document-hygiene.md
 */

const { execSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const GREEN = '\x1b[32m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const REPO_ROOT = execSync('git rev-parse --show-toplevel', { encoding: 'utf8' }).trim();
const ALLOWLIST_PATH = path.join(REPO_ROOT, 'scripts/doc-allowlist.json');

let ALLOWLIST;
try {
  ALLOWLIST = JSON.parse(fs.readFileSync(ALLOWLIST_PATH, 'utf8'));
} catch (err) {
  console.error(`${RED}doc-location gate: cannot read ${ALLOWLIST_PATH}: ${err.message}${RESET}`);
  process.exit(1);
}

// Root-level filename patterns that signal an internal planning doc.
const ROOT_BLOCK_PATTERNS = [
  /PLAN\.md$/i, /-PLAN\.md$/i, /^PLAN-/i,
  /STRATEGY\.md$/i, /-STRATEGY-/i,
  /AUDIT\.md$/i, /-AUDIT-/i,
  /CHECKLIST\.md$/i, /ROADMAP\.md$/i,
  /TRAJECTORY\.md$/i, /^TRAJECTORY-/i,
  /^VIRAL-/i, /^LAUNCH-/i, /^PRE-LAUNCH-/i, /^FIRST-CONTACT-/i,
  /^FORTIFICATION-/i, /^EXECUTION-/i, /^ASCENT-/i, /^BATTLE-/i,
  /^MASTER-/i, /^BARRIER-/i, /^IMPROVEMENTS-/i, /^CRITICAL-/i,
  /^DEVELOPER-OS-/i, /^NOTIFICATION-INTELLIGENCE-/i,
  /^INTELLIGENCE-CONSOLE-/i,
  /^whats-next\.md$/i, /^WHATS-NEXT\.md$/i, /^NEXT-STEPS\.md$/i,
  /^MISSION_/i, /^SHIP_/i,
];

// Directories with per-directory allowlists. Anything else in them = blocked.
const MIXED_DIRS = ['.ai/', 'docs/strategy/'];

// PII patterns. Matches must not appear in tracked content.
// Extend this list as new PII categories are identified.
const PII_PATTERNS = [
  {
    label: 'Personal Gmail (runyourempirehq)',
    regex: /runyourempirehq@gmail\.com/i,
  },
  {
    label: 'Personal Gmail (4dasystems — use a role alias like hello@4da.ai instead)',
    regex: /4dasystems@gmail\.com/i,
  },
];

// PII exclusions — files where the pattern is expected (e.g. this gate itself,
// the rule doc that documents the pattern, memory files). These paths are NOT
// blocked even if they match PII regex, because the content is meta-reference
// rather than a leak.
const PII_EXCLUDE_PATHS = [
  'scripts/check-doc-location.cjs',
  'scripts/public-readiness-audit.cjs',
  'scripts/doc-allowlist.json',
  '.claude/rules/document-hygiene.md',
  'CLAUDE.md', // references the gate behaviour
];

const PUBLIC_OK_MARKER = /<!--\s*public-ok\s*:\s*.+?-->/i;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isRootBlocked(filename) {
  const allow = new Set(ALLOWLIST.root.files);
  if (allow.has(filename)) return false;
  return ROOT_BLOCK_PATTERNS.some((re) => re.test(filename));
}

function isMixedDirBlocked(filepath) {
  for (const dir of MIXED_DIRS) {
    if (!filepath.startsWith(dir)) continue;
    const rel = filepath.slice(dir.length);
    // Only block direct children (not deeper paths — those are assumed private by convention)
    if (rel.includes('/')) return { dir, reason: 'nested-path' };
    const allowed = new Set((ALLOWLIST[dir] && ALLOWLIST[dir].files) || []);
    if (!allowed.has(rel)) return { dir, reason: 'not-on-allowlist' };
  }
  return null;
}

function hasPublicOkMarker(filepath) {
  try {
    const abs = path.join(REPO_ROOT, filepath);
    const head = fs.readFileSync(abs, 'utf8').split('\n').slice(0, 10).join('\n');
    return PUBLIC_OK_MARKER.test(head);
  } catch {
    return false;
  }
}

function scanPII(filepath) {
  if (PII_EXCLUDE_PATHS.includes(filepath)) return [];
  try {
    const abs = path.join(REPO_ROOT, filepath);
    // Skip binary files and large files (basic heuristic)
    const stat = fs.statSync(abs);
    if (stat.size > 1024 * 1024) return []; // > 1 MB: skip
    const content = fs.readFileSync(abs, 'utf8');
    // Basic binary detection
    if (content.includes('\u0000')) return [];
    const hits = [];
    for (const { label, regex } of PII_PATTERNS) {
      if (regex.test(content)) hits.push(label);
    }
    return hits;
  } catch {
    return [];
  }
}

function getStagedFiles() {
  const raw = execSync('git diff --cached --name-only --diff-filter=ACM', {
    encoding: 'utf8',
  });
  return raw.split('\n').filter(Boolean);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  let staged;
  try {
    staged = getStagedFiles();
  } catch (err) {
    console.error(`${YELLOW}doc-location gate: failed to list staged files (${err.message}). Skipping.${RESET}`);
    return 0;
  }

  const rootViolations = [];
  const mixedViolations = [];
  const piiViolations = [];

  for (const file of staged) {
    // 1. Root-level .md check
    if (!file.includes('/') && file.endsWith('.md')) {
      if (isRootBlocked(file) && !hasPublicOkMarker(file)) {
        rootViolations.push(file);
      }
    }

    // 2. Mixed-directory allowlist check
    const mixed = isMixedDirBlocked(file);
    if (mixed && !hasPublicOkMarker(file)) {
      mixedViolations.push({ file, ...mixed });
    }

    // 3. PII scan on all text files
    const piiHits = scanPII(file);
    if (piiHits.length > 0) {
      piiViolations.push({ file, hits: piiHits });
    }
  }

  const hasViolations =
    rootViolations.length > 0 ||
    mixedViolations.length > 0 ||
    piiViolations.length > 0;

  if (!hasViolations) return 0;

  console.error('');
  console.error(`${RED}${BOLD}✗ Doc-hygiene gate: violations detected${RESET}`);
  console.error('');

  if (rootViolations.length > 0) {
    console.error(`${BOLD}${RED}Internal planning doc(s) at repo root:${RESET}`);
    for (const f of rootViolations) console.error(`  ${RED}${f}${RESET}`);
    console.error('');
  }

  if (mixedViolations.length > 0) {
    console.error(`${BOLD}${RED}File(s) not on the directory allowlist:${RESET}`);
    for (const v of mixedViolations) {
      console.error(`  ${RED}${v.file}${RESET}  (in ${v.dir}, ${v.reason})`);
    }
    console.error('');
    console.error(`${YELLOW}The allowlist for mixed dirs lives in scripts/doc-allowlist.json.${RESET}`);
    console.error('');
  }

  if (piiViolations.length > 0) {
    console.error(`${BOLD}${RED}PII detected in staged file(s):${RESET}`);
    for (const v of piiViolations) {
      console.error(`  ${RED}${v.file}${RESET}`);
      for (const h of v.hits) console.error(`    - ${h}`);
    }
    console.error('');
    console.error(`${YELLOW}Replace personal identifiers with role-based aliases (hello@4da.ai, etc)${RESET}`);
    console.error(`${YELLOW}or move the file out of tracked content.${RESET}`);
    console.error('');
  }

  console.error(`${BOLD}Canonical homes for internal docs:${RESET}`);
  console.error(`  ${GREEN}.claude/plans/${RESET}       session-local, gitignored`);
  console.error(`  ${GREEN}docs/strategy/<ALLOW>${RESET} curated architecture only (see allowlist)`);
  console.error(`  ${GREEN}docs/private/${RESET}        never tracked`);
  console.error('');
  console.error(`${BOLD}Escape hatches:${RESET}`);
  console.error(`  - Add ${YELLOW}<!-- public-ok: <reason> -->${RESET} to the first 10 lines`);
  console.error(`    (allowed for root + mixed-dir checks, NOT for PII)`);
  console.error(`  - Add filename to scripts/doc-allowlist.json`);
  console.error('');
  console.error(`${BOLD}Full doctrine:${RESET} .claude/rules/document-hygiene.md`);
  console.error('');
  return 1;
}

process.exit(main());
