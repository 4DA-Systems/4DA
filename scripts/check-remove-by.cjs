#!/usr/bin/env node
/*
 * check-remove-by.cjs — build gate: no `REMOVE BY <date>` marker may be past its date.
 *
 * WHY THIS EXISTS
 *   The codebase uses `// REMOVE BY YYYY-MM-DD` to put an expiry on temporary code: dead-code
 *   allowances, compatibility shims, feature-flag scaffolding, "delete after the migration lands"
 *   helpers. .husky/pre-commit already enforces the marker for `#[allow(dead_code)]` — but ONLY on
 *   Rust files that are staged in that commit, and only for that one annotation. Every other marker
 *   in the tree was written on the honour system and nothing ever looked at it again.
 *
 *   The predictable result: at the time this gate was written the tree carried dozens of markers
 *   whose dates had already passed. A deadline nobody checks is a comment, not a deadline. This gate
 *   makes the convention enforced instead of remembered.
 *
 * WHAT IT CHECKS
 *   Scans SCAN_DIRS for `REMOVE BY YYYY-MM-DD` (also accepts YYYY/MM/DD, matching the pre-commit
 *   hook's regex) and fails if any marker's date is on or before today. "On or before" matches
 *   .husky/pre-commit, which treats `EXPIRY <= TODAY` as expired.
 *
 * ADOPTING THIS INCREMENTALLY
 *   This gate is expected to FAIL on a tree that has accumulated expired markers. That is the point.
 *   To adopt it without stopping the world, add already-ticketed markers to
 *   scripts/remove-by-allowlist.json. That file ships EMPTY and the intent is to DRIVE IT TO ZERO —
 *   it is a paydown ledger, not a parking lot. Every entry needs a ticket. Resolve the marker
 *   (delete the code, or move the date with a reason) and drop the entry.
 *
 * USAGE
 *   node scripts/check-remove-by.cjs             # local check
 *   node scripts/check-remove-by.cjs --verbose   # also list still-valid (future) markers
 *   node scripts/check-remove-by.cjs --ci        # emit GitHub Actions annotations
 *
 *   REMOVE_BY_TODAY=2026-01-01 node scripts/check-remove-by.cjs   # pin "today" (testing)
 *
 * EXIT CODES
 *   0 — no expired markers (allowlisted ones do not count)
 *   1 — at least one expired marker is not allowlisted
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

// Directories whose source is subject to the convention.
const SCAN_DIRS = ['src', 'src-tauri/src', 'scripts', 'mcp-4da-server/src'];

const SCAN_EXTENSIONS = new Set(['.rs', '.ts', '.tsx', '.js', '.jsx', '.cjs', '.mjs']);

const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', 'build', '.git', '_future']);

const ALLOWLIST_PATH = path.join(__dirname, 'remove-by-allowlist.json');

// Mirrors the marker regex in .husky/pre-commit so the two gates agree on what a marker is.
const MARKER_RE = /REMOVE BY (\d{4})[-/](\d{2})[-/](\d{2})/g;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Today as YYYY-MM-DD in LOCAL time, or the REMOVE_BY_TODAY override (for tests). */
function resolveToday(env = process.env) {
  const override = env.REMOVE_BY_TODAY;
  if (override && /^\d{4}-\d{2}-\d{2}$/.test(override.trim())) return override.trim();
  const now = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/*
 * Test files are skipped. They legitimately contain marker-LOOKING strings as fixtures — this
 * gate's own test asserts on dated marker literals — and flagging those would make the convention
 * impossible to test. Matches the test-skipping precedent in check-no-window-spawns.cjs and
 * check-file-sizes.cjs.
 */
function isSkippedFile(rel) {
  const norm = rel.replace(/\\/g, '/');
  if (/(^|\/)tests?\//.test(norm)) return true;
  if (/(^|\/)__tests__\//.test(norm)) return true;
  if (/_tests?\.rs$/.test(norm)) return true;
  if (/\.(test|spec)\.[cm]?[jt]sx?$/.test(norm)) return true;
  return false;
}

/** Recursively collect scannable files under a directory. */
function collectFiles(dir, out = []) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return out; // directory absent (e.g. a partial checkout) — not an error
  }
  for (const entry of entries) {
    if (SKIP_DIRS.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) collectFiles(full, out);
    else if (entry.isFile() && SCAN_EXTENSIONS.has(path.extname(entry.name))) out.push(full);
  }
  return out;
}

function loadAllowlist(allowlistPath = ALLOWLIST_PATH) {
  let raw;
  try {
    raw = fs.readFileSync(allowlistPath, 'utf8');
  } catch {
    return []; // no allowlist file == nothing allowlisted
  }
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    console.error(`remove-by gate: cannot parse ${allowlistPath}: ${err.message}`);
    process.exit(2);
  }
  return Array.isArray(parsed.allow) ? parsed.allow : [];
}

/**
 * Find every REMOVE BY marker in a set of { rel, src } sources and classify it against `today`.
 * Pure (no filesystem) so it can be unit-tested. Returns { expired, upcoming }.
 *
 * `allow` entries suppress an expired marker when file AND date both match.
 */
function analyzeSources(sources, today, allow = []) {
  const expired = [];
  const upcoming = [];

  const allowKeys = new Set(
    allow
      .filter((a) => a && a.file && a.date)
      .map((a) => `${String(a.file).replace(/\\/g, '/')}@${a.date}`)
  );

  for (const { rel, src } of sources) {
    // Split on \n and strip a trailing \r so CRLF files report clean line text.
    const lines = src.split('\n');
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].replace(/\r$/, '');
      MARKER_RE.lastIndex = 0;
      let m;
      while ((m = MARKER_RE.exec(line)) !== null) {
        // A marker wrapped in backticks is prose CITING a marker (e.g. a changelog note
        // "the `REMOVE BY 2026-08-01` marker was removed"), not a live deadline. Skipping
        // these keeps the gate from flagging the very comments that record a paydown.
        const before = line[m.index - 1];
        const after = line[m.index + m[0].length];
        if (before === '`' && after === '`') continue;

        const date = `${m[1]}-${m[2]}-${m[3]}`; // normalize YYYY/MM/DD -> YYYY-MM-DD
        const record = {
          rel,
          lineNo: i + 1,
          date,
          text: line.trim().slice(0, 120),
          allowlisted: allowKeys.has(`${rel}@${date}`),
        };
        // String compare is correct for zero-padded ISO dates.
        // `<=` matches .husky/pre-commit, which treats the deadline day itself as expired.
        if (date <= today) expired.push(record);
        else upcoming.push(record);
      }
    }
  }
  return { expired, upcoming };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/** CLI entry point. Returns the process exit code. */
function main(argv) {
  const ciMode = argv.includes('--ci');
  const verbose = argv.includes('--verbose');
  const today = resolveToday();
  const allow = loadAllowlist();

  const all = [];
  for (const dir of SCAN_DIRS) collectFiles(path.join(ROOT, dir), all);
  const files = all.filter((f) => !isSkippedFile(path.relative(ROOT, f)));

  const sources = files.map((f) => ({
    rel: path.relative(ROOT, f).replace(/\\/g, '/'),
    src: fs.readFileSync(f, 'utf8'),
  }));

  const { expired, upcoming } = analyzeSources(sources, today, allow);
  const blocking = expired.filter((e) => !e.allowlisted);
  const excused = expired.filter((e) => e.allowlisted);

  console.log(
    `remove-by gate: scanned ${files.length} files, ` +
      `${expired.length + upcoming.length} REMOVE BY marker(s) ` +
      `(${upcoming.length} upcoming, ${excused.length} allowlisted, ${blocking.length} expired) ` +
      `as of ${today}.`
  );

  if (verbose) {
    for (const u of [...upcoming].sort((a, b) => a.date.localeCompare(b.date))) {
      console.log(`  ok   ${u.rel}:${u.lineNo}  due ${u.date}`);
    }
    for (const e of excused) {
      console.log(`  allow ${e.rel}:${e.lineNo}  due ${e.date} (allowlisted)`);
    }
  }

  if (!blocking.length) return 0;

  blocking.sort((a, b) => a.date.localeCompare(b.date) || a.rel.localeCompare(b.rel));

  console.error(`\n${blocking.length} REMOVE BY deadline(s) have passed:\n`);
  for (const v of blocking) {
    console.error(`  ${v.rel}:${v.lineNo}  due ${v.date}`);
    console.error(`      ${v.text}`);
    if (ciMode) {
      console.log(
        `::error file=${v.rel},line=${v.lineNo}::REMOVE BY ${v.date} has passed — ` +
          `delete the code, or move the deadline with a written reason.`
      );
    }
  }

  console.error('\nFix each by ONE of:');
  console.error('  - delete the code the marker is attached to (the intended outcome), OR');
  console.error('  - move the date AND say why in the same comment, OR');
  console.error('  - if it is already ticketed, add it to scripts/remove-by-allowlist.json');
  console.error('    ({ "file", "date", "ticket", "note" }) — that list is a paydown ledger');
  console.error('    and is meant to reach zero, not to grow.\n');

  return 1;
}

module.exports = { analyzeSources, resolveToday, collectFiles, isSkippedFile, SCAN_DIRS };

if (require.main === module) {
  process.exit(main(process.argv));
}
