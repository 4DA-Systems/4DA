#!/usr/bin/env node
/**
 * check-vanity-metrics.cjs — Intelligence Doctrine rule 3 enforcement.
 *
 * Rule 3 (.claude/rules/intelligence-doctrine.md): "No vanity metrics." Every number
 * shown to the user must pass "what action does this inform?". The doctrine names
 * specific banned counters:
 *   - "Items monitored"      - "Sources producing"
 *   - "Validated principles: 0"  - "Decisions tracked: 0"
 *
 * This gate enforces the *detectable* part: those phrases rendered AS A COUNTER —
 * i.e. adjacent to a number or an i18n {{count}} interpolation. It deliberately does
 * NOT flag the phrases in prose (e.g. a feature bullet "decisions tracked with
 * evidence"), which is legitimate. The semantic rules (coverage gauges whose
 * denominator ~= numerator, single-point sparklines, %s that default to 0/100 on a
 * zero denominator) are not syntactically detectable and remain a PR-review check.
 *
 * Escape hatch: `vanity-ok: <reason>` in a comment on the line or the line above.
 *
 * Wired into .husky/pre-commit. Exit 1 on any unjustified violation.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

// The banned vanity-counter labels (doctrine rule 3).
const PHRASE = '(items monitored|sources producing|validated principles|decisions tracked)';

// A counter = the phrase adjacent to a number or a {{count}}-style interpolation.
const PATTERNS = [
  // "Decisions tracked: 0" / "Items monitored = 12"
  new RegExp(`${PHRASE}\\s*[:=]\\s*(\\{\\{|\\d)`, 'i'),
  // "{{count}} items monitored"  OR  "decisions tracked {{count}}"
  new RegExp(`\\{\\{[^}]*\\}\\}\\s*${PHRASE}`, 'i'),
  new RegExp(`${PHRASE}\\s*\\{\\{`, 'i'),
];

const EXCLUDE = [
  /[._]test\./,
  /_tests\.rs$/,
  /\/tests\//,
  /[\\/]__tests__[\\/]/,
  /scripts[\\/]check-vanity-metrics\.cjs$/,
];

function trackedFiles() {
  const out = execSync('git ls-files "src/*.ts" "src/*.tsx" "src/locales/*.json"', {
    cwd: ROOT,
    encoding: 'utf8',
  });
  return out
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
    .filter((f) => !EXCLUDE.some((re) => re.test(f)));
}

const violations = [];

for (const rel of trackedFiles()) {
  let text;
  try {
    text = fs.readFileSync(path.join(ROOT, rel), 'utf8');
  } catch {
    continue;
  }
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    // 2-line window: in JSX a label and its count can be on adjacent lines.
    const windowText = lines.slice(i, i + 2).join(' ').replace(/\s+/g, ' ');
    if (!PATTERNS.some((re) => re.test(windowText))) continue;
    const marker = /vanity-ok:/;
    const scope = lines.slice(Math.max(0, i - 1), i + 2).join('\n');
    if (marker.test(scope)) {
      i += 1;
      continue;
    }
    violations.push({ file: rel, line: i + 1, snippet: lines[i].trim().slice(0, 100) });
    i += 1;
  }
}

if (violations.length === 0) {
  console.log('Vanity-metrics: clean — no banned counters (doctrine rule 3).');
  process.exit(0);
}

console.error('\nVANITY-METRIC VIOLATION (intelligence-doctrine rule 3 — "no vanity metrics")\n');
console.error('A banned counter is being rendered. Every number shown to the user must answer');
console.error('"what action does this inform?" — if the answer is "none / it looks busy", cut it.\n');
for (const v of violations) {
  console.error(`  ${v.file}:${v.line}`);
  console.error(`    ${v.snippet}`);
}
console.error(
  '\nIf this is genuinely actionable (rare), add `vanity-ok: <reason>` in a comment on',
);
console.error('that line or the line above. See .claude/rules/intelligence-doctrine.md.\n');
process.exit(1);
