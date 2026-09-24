#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * check-privacy-egress.cjs — keep raw local content out of LLM-bound text.
 *
 * NETWORK.md states: "Project files, source code, file contents, and git history
 * never leave your machine." On 2026-08-28 that was false. `analysis_rerank.rs`
 * put the five most recent git COMMIT MESSAGES into `JudgeRequest.context_summary`,
 * which is sent verbatim to whichever cloud LLM the user configured — and
 * `RerankConfig::enabled` defaults to true, so one saved API key switched it on.
 * The `titles_only` privacy setting did not cover it; it applies to article
 * content, not to the context summary.
 *
 * Nothing tested the promise, which is the actual defect. This gate does.
 *
 * RULE: a raw-content column may only be read in modules that MINE or STORE it.
 * Anywhere else — and every LLM prompt builder is anywhere else — is a finding.
 * Widening an allowlist is a deliberate, reviewable act; a new `SELECT` in a
 * prompt builder is not.
 *
 * Escape hatch: `privacy-egress-ok: <reason>` on the line or the line above.
 * Reserved for a reference that provably cannot reach a network call.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

/**
 * Columns holding text the user wrote or the machine holds locally. These are
 * the things NETWORK.md promises stay put.
 */
const RAW_CONTENT_TOKENS = [
  'commit_message',
  'file_content',
];
// Deliberately NOT included: `chunk_text`. It is the name of a text-splitting
// UTILITY as well as a column, so it fires on ordinary code — 22 hits, all
// benign, most of them the utility's own tests. A gate that cries wolf on
// normal source gets switched off, which is the same outcome as no gate.
// Distinctive column names only.

/**
 * Modules allowed to touch each token, with the reason. Paths are repo-relative
 * and matched as prefixes so a module can be a file or a directory.
 */
const ALLOWLIST = {
  commit_message: [
    ['src-tauri/src/ace/git.rs', 'the miner — reads git, extracts local topics'],
    ['src-tauri/src/ace/db.rs', 'storage layer for the mined signal'],
    ['src-tauri/src/ace/context.rs', 'local context assembly, never sent'],
  ],
  file_content: [
    ['src-tauri/src/ace/', 'ACE mints local topics from file content on-device'],
    ['src-tauri/src/db/', 'storage layer'],
    ['src-tauri/src/context_admission.rs', 'local admission classifier'],
    ['src-tauri/src/scoring/', 'on-device scoring reads the local corpus'],
  ],
};

const ESCAPE = /privacy-egress-ok:/;

function trackedRustFiles() {
  // Both globs: git's `**/` matches one-or-more directories, so
  // "src-tauri/src/**/*.rs" alone never matched the 299 top-level files
  // (monitoring.rs, data_export.rs, ...) — they were unscanned until 2026-09-05.
  const out = execSync('git ls-files "src-tauri/src/*.rs" "src-tauri/src/**/*.rs"', {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  return [...new Set(out.split('\n').map((s) => s.trim()).filter(Boolean))];
}

function isAllowed(token, file) {
  return (ALLOWLIST[token] || []).some(([prefix]) => file.startsWith(prefix));
}

/** Scan one file's text. Exported so the test can drive it without git. */
function scanText(file, text) {
  const findings = [];
  // Tests describe the guard; they do not perform egress.
  if (/_tests?\.rs$/.test(file) || file.includes('/tests/')) return findings;
  const lines = text.split('\n');
  for (const token of RAW_CONTENT_TOKENS) {
    if (isAllowed(token, file)) continue;
    lines.forEach((line, i) => {
      if (!line.includes(token)) return;
      if (ESCAPE.test(line) || (i > 0 && ESCAPE.test(lines[i - 1]))) return;
      // A comment mentioning the token is documentation, not egress.
      if (/^\s*(\/\/|\*|\/\*)/.test(line)) return;
      findings.push({ file, line: i + 1, token, text: line.trim().slice(0, 100) });
    });
  }
  return findings;
}

/*
 * RULE 2 (2026-09-24): the `titles_only` privacy setting.
 *
 * NETWORK.md promises that with `llm_content_level = "titles_only"` an
 * off-machine model gets item titles and no body text. Until 2026-09-24 one of
 * the dozen modules that call a model honoured it. Every module that builds an
 * `LLMClient` must now either route item text through `llm_egress::` or say, with
 * a reason, why it does not have to:
 *
 *   // llm-egress: no-item-body <what it sends instead>
 *   // llm-egress: exempt <why this caller may send bodies regardless>
 *
 * A new call site fails until someone makes that decision on purpose.
 */
const LLM_CLIENT_CTOR = /LLMClient::(new|with_purpose)\(/;
const EGRESS_ROUTED = /llm_egress::/;
// The reason must be on the same line: `\s` would let the newline and the next
// code line stand in for a reason.
const EGRESS_DECLARED = /llm-egress:[ \t]*(no-item-body|exempt)[ \t]+\S/;

/**
 * Temporarily not routed, with the reason. Every entry must be removed by the
 * change that routes the module; this list is meant to be empty.
 */
const EGRESS_PENDING = {
  'src-tauri/src/llm_judgments.rs':
    'claimed by the judge-capability-routing change; routed in its follow-up',
  'src-tauri/src/llm_judge_drain.rs':
    'claimed by the judge-capability-routing change; routed in its follow-up',
};

/**
 * A Rust file with every `#[cfg(test)] mod … { … }` body removed. Test modules
 * can sit mid-file with production code after them, so this removes only the
 * braced body (by brace depth), not everything below the first one. Braces in
 * strings or comments can skew the count; a skew only ever keeps MORE code, so the
 * gate errs toward flagging.
 */
function productionPart(text) {
  const lines = text.split('\n');
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    const isTestAttr = /^\s*#\[cfg\(test\)\]\s*$/.test(lines[i]);
    // The attribute may be followed by other attributes (e.g. #[path = "…"]).
    let j = i + 1;
    while (isTestAttr && j < lines.length && /^\s*#\[/.test(lines[j])) j++;
    if (!isTestAttr || j >= lines.length || !/^\s*(pub(\([^)]*\))?\s+)?mod\s/.test(lines[j])) {
      out.push(lines[i]);
      continue;
    }
    if (!lines[j].includes('{')) {
      i = j; // `mod tests;` — body lives in another file
      continue;
    }
    let depth = 0;
    let k = j;
    for (; k < lines.length; k++) {
      for (const ch of lines[k]) {
        if (ch === '{') depth++;
        else if (ch === '}') depth--;
      }
      if (depth <= 0 && k > j - 1 && lines[k].includes('}')) break;
    }
    i = k;
  }
  return out.join('\n');
}

/** Rule 2 for one file. Exported so the test can drive it without git. */
function scanLlmCallSites(file, text) {
  if (/_tests?\.rs$/.test(file) || file.includes('/tests/')) return [];
  if (EGRESS_PENDING[file]) return [];
  const code = productionPart(text)
    .split('\n')
    .filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l))
    .join('\n');
  if (!LLM_CLIENT_CTOR.test(code)) return [];
  if (EGRESS_ROUTED.test(code) || EGRESS_DECLARED.test(text)) return [];
  return [{ file, line: 0, token: 'titles_only', text: 'builds an LLMClient without llm_egress' }];
}

function main() {
  const findings = [];
  for (const file of trackedRustFiles()) {
    const full = path.join(ROOT, file);
    if (!fs.existsSync(full)) continue;
    const text = fs.readFileSync(full, 'utf8');
    findings.push(...scanText(file, text));
    findings.push(...scanLlmCallSites(file, text));
  }

  if (findings.length === 0) {
    const pending = Object.keys(EGRESS_PENDING);
    console.log(
      '[check-privacy-egress] OK — raw local content is confined to the modules that mine and store it, and every LLM caller outside the pending list honours titles_only.'
    );
    if (pending.length) {
      console.log(`[check-privacy-egress] titles_only still pending in: ${pending.join(', ')}`);
    }
    return 0;
  }

  console.error('[check-privacy-egress] raw local content referenced outside its allowlist:\n');
  for (const f of findings) {
    console.error(`  ${f.file}:${f.line}  [${f.token}]  ${f.text}`);
  }
  console.error(
    '\nIf this module cannot reach a network call, add `privacy-egress-ok: <reason>`.'
  );
  console.error('If it can, the content must not go in. NETWORK.md is the contract.');
  console.error(
    '[titles_only] findings: gate item body text with `crate::llm_egress::body_allowed(&provider)`, or add' +
      ' `// llm-egress: no-item-body <what it sends>` / `// llm-egress: exempt <why>`.'
  );
  return 1;
}

if (require.main === module) process.exit(main());

module.exports = {
  scanText,
  scanLlmCallSites,
  productionPart,
  RAW_CONTENT_TOKENS,
  ALLOWLIST,
  EGRESS_PENDING,
};
