#!/usr/bin/env node
/**
 * check-llm-gate-honesty.cjs — antibody enforcement for "proxy-derived state claims".
 *
 * Background: a recurring false-state class (antibody 2026-06-02-proxy-derived-state.md)
 * where "is an LLM provider configured?" was re-derived inline from key-presence
 * (`!api_key.is_empty()` / `has_api_key`) OR a single-provider OR-shortcut
 * (`provider == "ollama"`). A stray/env key with provider "none" flips such a check
 * true (false-positive "configured"), and the OR-shortcut silently drops "builtin"
 * (false-negative). Ten such sites were fixed by routing through the single source of
 * truth: `content_personalization::context::compute_has_llm(provider, api_key)`.
 *
 * This gate fails the commit if a NEW inline proxy construct appears. Capability is a
 * property of the SELECTED PROVIDER, not of key presence — route through compute_has_llm.
 *
 * Escape hatch (rare, must be justified): put `llm-gate-ok: <reason>` in a comment on
 * the offending line or the line above it.
 *
 * Wired into .husky/pre-commit. Exit 1 on any unjustified violation.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

// The forbidden shapes (applied to a 3-line whitespace-collapsed window).
const PATTERNS = [
  // Rust: api_key.is_empty() || provider == "ollama"|"builtin"   (either order)
  /api_key\s*\.\s*is_empty\s*\(\s*\)\s*\|\|\s*[\w.]*provider\s*==\s*"(?:ollama|builtin)"/,
  /[\w.]*provider\s*==\s*"(?:ollama|builtin)"\s*\|\|\s*!?\s*[\w.]*api_key\s*\.\s*is_empty\s*\(\s*\)/,
  // Rust: !matches!(provider, "ollama"|"builtin") && api_key.is_empty()  (missing "none")
  /!\s*matches!\s*\([^)]*"(?:ollama|builtin)"[^)]*\)\s*&&\s*[\w.]*api_key\s*\.\s*is_empty\s*\(\s*\)/,
  // TS: has_api_key || provider === 'ollama'|'builtin'   (either order)
  /has_api_key\s*\|\|\s*[\w.]*provider\s*===\s*['"](?:ollama|builtin)/,
  /provider\s*===\s*['"](?:ollama|builtin)['"]\s*\|\|\s*[\w.]*has_api_key/,
];

// Files we never scan: tests, the canonical helper's own module, and this script.
const EXCLUDE = [
  /[._]test\./,
  /_tests\.rs$/,
  /\/tests\//,
  /content_personalization[\\/]context\.rs$/, // defines compute_has_llm (match arms, not a proxy)
  /scripts[\\/]check-llm-gate-honesty\.cjs$/,
];

function trackedSourceFiles() {
  const out = execSync('git ls-files "src-tauri/src/*.rs" "src/*.ts" "src/*.tsx"', {
    cwd: ROOT,
    encoding: 'utf8',
  });
  return out
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
    .filter((f) => !EXCLUDE.some((re) => re.test(f)));
}

function collapse(s) {
  return s.replace(/\s+/g, ' ');
}

const violations = [];

for (const rel of trackedSourceFiles()) {
  const abs = path.join(ROOT, rel);
  let text;
  try {
    text = fs.readFileSync(abs, 'utf8');
  } catch {
    continue;
  }
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    // 3-line window catches rustfmt/prettier line-splits of one expression.
    const windowRaw = lines.slice(i, i + 3).join(' ');
    const windowText = collapse(windowRaw);
    const hit = PATTERNS.find((re) => re.test(windowText));
    if (!hit) continue;
    // Escape hatch: marker anywhere in the window or the line directly above it.
    const marker = /llm-gate-ok:/;
    const markerScope = lines.slice(Math.max(0, i - 1), i + 3).join('\n');
    if (marker.test(markerScope)) {
      i += 2;
      continue;
    }
    // Report the most relevant line in the window (the key-presence test), not just
    // the window's first line.
    const offset = lines.slice(i, i + 3).findIndex((l) => /api_key|has_api_key/.test(l));
    const at = i + (offset >= 0 ? offset : 0);
    violations.push({ file: rel, line: at + 1, snippet: collapse(lines[at].trim()).slice(0, 100) });
    // Skip the rest of this window so a split match isn't reported 3×.
    i += 2;
  }
}

if (violations.length === 0) {
  console.log('LLM-gate honesty: clean — all availability checks route through compute_has_llm.');
  process.exit(0);
}

console.error('\nLLM-GATE HONESTY VIOLATION (antibody 2026-06-02-proxy-derived-state)\n');
console.error(
  'An LLM "configured/available" check is derived from key-presence or a single-provider',
);
console.error('OR-shortcut. A stray/env key with provider "none" makes it falsely true, and the');
console.error('shortcut drops "builtin". Route through the single source of truth instead:\n');
console.error('    crate::content_personalization::context::compute_has_llm(&provider, &api_key)\n');
for (const v of violations) {
  console.error(`  ${v.file}:${v.line}`);
  console.error(`    ${v.snippet}`);
}
console.error(
  '\nIf this is a genuine, justified exception, add `llm-gate-ok: <reason>` in a comment',
);
console.error('on that line or the line above. Do not bypass the gate without a reason.\n');
process.exit(1);
