// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * A detector is only worth what its NEGATIVE test proves. This suite drives
 * `scanText` directly with synthetic content, so it never depends on the
 * repo's current state — the gate must still catch the regression on the day
 * someone reintroduces it.
 */
const test = require('node:test');
const assert = require('node:assert');
const { scanText } = require('./check-privacy-egress.cjs');

// The exact code that was live on 2026-08-28, reduced to its load-bearing line.
const REGRESSION = `
    if let Ok(mut stmt) = db.prepare(
        "SELECT commit_message FROM git_signals WHERE commit_message IS NOT NULL ORDER BY timestamp DESC LIMIT 5",
    ) {
        parts.push(format!("Recent commits:\n{}", commit_lines.join("\n")));
    }
`;

test('the exact regression is caught in an LLM-facing module', () => {
  const hits = scanText('src-tauri/src/analysis_rerank.rs', REGRESSION);
  assert.ok(hits.length > 0, 'reintroducing the commit-message SELECT must fail the gate');
  assert.strictEqual(hits[0].token, 'commit_message');
});

test('it is caught in a module nobody has thought of yet', () => {
  // The allowlist is a list of ALLOWED places, not a list of watched ones, so a
  // brand-new file is covered without anyone remembering to add it.
  const hits = scanText('src-tauri/src/some_new_prompt_builder.rs', REGRESSION);
  assert.ok(hits.length > 0);
});

test('the modules that mine and store the signal are not flagged', () => {
  for (const f of [
    'src-tauri/src/ace/git.rs',
    'src-tauri/src/ace/db.rs',
    'src-tauri/src/ace/context.rs',
  ]) {
    assert.deepStrictEqual(scanText(f, REGRESSION), [], `${f} must stay allowed`);
  }
});

test('documentation about the rule does not trip it', () => {
  // This very gate, and the comment left at the removal site, both name the
  // column. Neither performs egress.
  const doc = [
    '// Commit MESSAGES used to be included here — see check-privacy-egress.cjs.',
    '/// The `commit_message` column is mined locally and never sent.',
    ' * commit_message stays on the machine.',
  ].join('\n');
  assert.deepStrictEqual(scanText('src-tauri/src/analysis_rerank.rs', doc), []);
});

test('the escape hatch works and requires a reason on or above the line', () => {
  const onLine = 'let x = commit_message; // privacy-egress-ok: local digest, never leaves';
  assert.deepStrictEqual(scanText('src-tauri/src/whatever.rs', onLine), []);

  const above = [
    '// privacy-egress-ok: local digest, never leaves',
    'let x = commit_message;',
  ].join('\n');
  assert.deepStrictEqual(scanText('src-tauri/src/whatever.rs', above), []);

  // Without the marker the same line is still a finding — the hatch is opt-in.
  assert.ok(scanText('src-tauri/src/whatever.rs', 'let x = commit_message;').length > 0);
});

test('test files are exempt', () => {
  assert.deepStrictEqual(scanText('src-tauri/src/foo_tests.rs', REGRESSION), []);
});
