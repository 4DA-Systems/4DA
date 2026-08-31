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
  const out = execSync('git ls-files "src-tauri/src/**/*.rs"', {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  return out.split('\n').map((s) => s.trim()).filter(Boolean);
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

function main() {
  const findings = [];
  for (const file of trackedRustFiles()) {
    const full = path.join(ROOT, file);
    if (!fs.existsSync(full)) continue;
    findings.push(...scanText(file, fs.readFileSync(full, 'utf8')));
  }

  if (findings.length === 0) {
    console.log(
      '[check-privacy-egress] OK — raw local content is confined to the modules that mine and store it.'
    );
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
  return 1;
}

if (require.main === module) process.exit(main());

module.exports = { scanText, RAW_CONTENT_TOKENS, ALLOWLIST };
