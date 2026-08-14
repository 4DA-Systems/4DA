#!/usr/bin/env node
/**
 * Dead-code expiry gate — every `#[allow(dead_code)]` must carry a
 * `REMOVE BY YYYY-MM-DD` marker on the same line or the line above, and that
 * date must not be in the past.
 *
 * Replaces the previous inline shell loop in .husky/pre-commit, which forked a
 * `grep` subprocess PER LINE of every staged file carrying an annotation. On
 * Windows/cygwin that is pathological: staging a change that touches
 * blind_spots.rs (6.5k lines) or db/migrations.rs (5.5k lines) made the hook
 * take tens of minutes. The hook's own comment recorded the symptom
 * ("cygheap fork failures ... observed repeatedly 2026-05-22") and narrowed the
 * file set, but the per-line fork remained. This does the same work in one
 * process.
 *
 * Scans STAGED content (`git show :<file>`) so it gates what is actually being
 * committed, matching the previous behaviour.
 *
 * Exit: 0 clean, 1 on any missing or expired marker.
 */

const { execFileSync } = require('node:child_process');

const MARKER = /REMOVE BY (\d{4})[-/](\d{2})[-/](\d{2})/;
const ALLOW = /#\[allow\(dead_code\)\]/;

function staged() {
  // execFileSync (no shell) so the regex and pathspec are passed to git verbatim —
  // cmd.exe on Windows mangles the quoting that a shell form would need.
  const out = execFileSync(
    'git',
    ['diff', '--cached', '-G', '#\\[allow\\(dead_code\\)\\]', '--name-only', '--', '*.rs'],
    { encoding: 'utf8', maxBuffer: 1 << 28 },
  );
  return out.split('\n').map((s) => s.trim()).filter(Boolean);
}

function main() {
  let files;
  try {
    files = staged();
  } catch {
    process.exit(0); // not a git context / nothing staged
  }
  if (files.length === 0) {
    console.log('Dead-code expiry: no staged files carry #[allow(dead_code)].');
    process.exit(0);
  }

  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const problems = [];
  let annotations = 0;

  for (const file of files) {
    let content;
    try {
      content = execFileSync("git", ["show", `:${file}`], { encoding: "utf8", maxBuffer: 1 << 28 });
    } catch {
      continue; // deleted in this commit
    }
    const lines = content.split('\n');
    for (let i = 0; i < lines.length; i++) {
      if (!ALLOW.test(lines[i])) continue;
      annotations++;
      const m = lines[i].match(MARKER) || (i > 0 ? lines[i - 1].match(MARKER) : null);
      if (!m) {
        problems.push(`  ERROR: ${file}:${i + 1} — #[allow(dead_code)] missing REMOVE BY YYYY-MM-DD`);
        continue;
      }
      const due = new Date(`${m[1]}-${m[2]}-${m[3]}T00:00:00`);
      if (due < today) {
        problems.push(`  ERROR: ${file}:${i + 1} — REMOVE BY ${m[1]}-${m[2]}-${m[3]} has expired`);
      }
    }
  }

  if (problems.length > 0) {
    console.error(problems.join('\n'));
    console.error('');
    console.error('Every #[allow(dead_code)] must have a comment with:');
    console.error('  // REMOVE BY YYYY-MM-DD');
    console.error('on the same line or the line above.');
    console.error('');
    console.error('Expired deadlines must be resolved: remove the dead code');
    console.error('or update the deadline with justification.');
    process.exit(1);
  }

  console.log(
    `Dead-code expiry: clean — ${annotations} annotation(s) across ${files.length} staged file(s), all dated and current.`,
  );
}

main();
