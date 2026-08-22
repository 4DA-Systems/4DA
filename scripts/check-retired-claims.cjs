#!/usr/bin/env node
/**
 * check-retired-claims.cjs — AD-030 enforcement.
 *
 * AD-030 (.ai/DECISIONS.md) retired the "gets sharper every day" product
 * promise and its derivatives ("learns from how you engage", "compound
 * intelligence", "behavior learning" as a feature name). The claims were
 * unmeasurable (AD-029: "all risk, no demonstrated lift") and the mechanism
 * behind them lost scoring authority in v19. This gate stops the phrases
 * regenerating — from old copy, from translations, or from an LLM that read
 * a stale doc.
 *
 * What still PASSES:
 *   - "Yesterday's noise becomes tomorrow's signal" — true (corpus
 *     re-judging); deliberately NOT a banned pattern.
 *   - Code identifiers (compound_advantage, CompoundAdvantageScore,
 *     compound_score.rs, compound-quality-check.cjs, compound-five) — the
 *     MCP tool measures realized outcomes and is a published API name.
 *   - Historical-record files that quote the retired claims in order to
 *     document their retirement (path allowlist below).
 *
 * Escape hatch: `retired-ok: <reason>` in a comment on the line or the line
 * above. Reserved for quoting the claim as history — never for making it.
 *
 * Wired into test:scripts and validate. Exit 1 on any unjustified hit.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

// Retired claim phrases (AD-030). Case-insensitive.
const PATTERNS = [
  /gets?\s+sharper\s+every\s+day/i,
  /sharper\s+every\s+day/i,
  /learns?\s+from\s+how\s+you\s+engage/i,
  /compound\s+intelligence/i,
  /intelligence\s+(that\s+)?compounds/i,
  /behaviou?r(al)?\s+learning/i,
  /compounds?\s+over\s+time/i,
  /(scoring|model|system)\s+(gets|becomes)\s+(smarter|sharper|more\s+accurate)\s+(over\s+time|with\s+use|every)/i,
  // Interaction-learning promise family (2026-08-23, GPT adversarial audit
  // finding 5). Equivalent promises the original family missed: the mechanism
  // lost scoring authority in v19 (pipeline_v2.rs: feedback_boost = 0.0) and
  // implicit capture was removed in v20b (#488), so "learns from your
  // activity" / "train the system" are FALSE statements, not just retired
  // marketing. Caught live in commands.rs autopsy recommendations and
  // onboarding.projects.noTech across all 13 locales.
  /(learns?|learning)\s+from\s+your\s+(activity|activities|interactions?|behaviou?r)/i,
  /will\s+learn\s+from\s+your/i,
  /train(s|ing)?\s+the\s+system/i,
  /teach(es|ing)?\s+the\s+system/i,
  /system\s+(learns|is\s+learning)\s+from\s+you/i,
];

// Code identifiers that legitimately contain banned substrings.
const IDENTIFIER_OK =
  /compound_advantage|CompoundAdvantage|compound_score|compound-quality-check|compound-five|compound_learning_tests/;

// Historical-record files: they QUOTE the retired claims to document the
// retirement. Everything else must not carry them.
const ALLOWLIST = [
  /^\.ai\/DECISIONS\.md$/,
  /^\.ai\/FAILURE_MODES\.md$/,
  /^site\/src\/writing\/retiring-a-claim-we-could-not-measure\.njk$/,
  /^scripts\/check-retired-claims(\.test)?\.cjs$/,
];

// STREETS course content uses "compound" about income streams — unrelated.
const EXCLUDE = [
  /^streets-course\//,
  /^docs\/streets\//,
  /^site\/src\/streets\//,
  /^site\/_site\//,
  /[._]test\.(ts|tsx|cjs|js)$/,
  /_tests\.rs$/,
  /[\\/]__tests__[\\/]/,
];

function trackedFiles() {
  // src-tauri/src is in scope because backend .rs files carry user-facing
  // strings (autopsy recommendations, notifications) — the 2026-08-23 finding
  // was two false learning promises living in commands.rs, invisible to a
  // frontend-only scan. Test files are dropped by EXCLUDE.
  const out = execSync(
    'git ls-files "src/**" "src-tauri/src/**" "site/src/**" "site/scan/**" "docs/**" "*.md" "CLAUDE.md" "src-tauri/tauri.conf.json" "mcp-4da-server/README.md" "mcp-4da-server/src/**" "editors/**"',
    { cwd: ROOT, encoding: 'utf8' },
  );
  return out
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
    .filter((f) => /\.(md|njk|json|html|ts|tsx|js|cjs|rs|toml)$/.test(f))
    .filter((f) => !EXCLUDE.some((re) => re.test(f)))
    .filter((f) => !ALLOWLIST.some((re) => re.test(f)));
}

/** Pure decision (exported for tests): does this line make a retired claim? */
function isRetiredClaim(line) {
  if (IDENTIFIER_OK.test(line)) return false;
  return PATTERNS.some((re) => re.test(line));
}

/** Scan one file's text → array of {line, snippet} violations (escape hatch honoured). */
function scanText(text) {
  const out = [];
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (!isRetiredClaim(lines[i])) continue;
    const scope = lines.slice(Math.max(0, i - 1), i + 1).join('\n');
    if (/retired-ok:/.test(scope)) continue;
    out.push({ line: i + 1, snippet: lines[i].trim().slice(0, 140) });
  }
  return out;
}

function main() {
  const violations = [];
  for (const file of trackedFiles()) {
    let text;
    try {
      text = fs.readFileSync(path.join(ROOT, file), 'utf8');
    } catch {
      continue;
    }
    for (const v of scanText(text)) {
      violations.push({ file, ...v });
    }
  }

  if (violations.length > 0) {
    console.error('\n[check-retired-claims] AD-030 violation(s) — retired promise language found:\n');
    for (const v of violations) {
      console.error(`  ${v.file}:${v.line}  ${v.snippet}`);
    }
    console.error(
      '\nThese claims were retired by AD-030 (.ai/DECISIONS.md): the mechanism was removed (AD-029)\n' +
        'and the claim was never measurable. Rewrite as a present-tense verifiable statement, or —\n' +
        'ONLY when quoting the claim as history — add `retired-ok: <reason>` on the line above.\n',
    );
    process.exit(1);
  }
  console.log('[check-retired-claims] OK — no retired promise language on user-facing surfaces.');
}

module.exports = { PATTERNS, isRetiredClaim, scanText };

if (require.main === module) {
  main();
}
