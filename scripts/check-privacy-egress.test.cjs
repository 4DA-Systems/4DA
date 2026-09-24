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

// ---------------------------------------------------------------------------
// Rule 2: the titles_only privacy setting (2026-09-24)
// ---------------------------------------------------------------------------
const { scanLlmCallSites, productionPart } = require('./check-privacy-egress.cjs');

const UNROUTED = `
pub async fn summarize(provider: LLMProvider, body: &str) -> String {
    let client = LLMClient::with_purpose(provider, "summary");
    client.complete(SYSTEM, vec![msg(body)]).await
}
`;

test('a new LLM caller that ignores titles_only is caught', () => {
  const hits = scanLlmCallSites('src-tauri/src/some_new_feature.rs', UNROUTED);
  assert.strictEqual(hits.length, 1);
  assert.strictEqual(hits[0].token, 'titles_only');
});

test('routing through llm_egress satisfies the rule', () => {
  const routed = UNROUTED.replace(
    'let client',
    'let send_body = crate::llm_egress::body_allowed(&provider);\n    let client'
  );
  assert.deepStrictEqual(scanLlmCallSites('src-tauri/src/some_new_feature.rs', routed), []);
});

test('a declaration needs a reason, and a comment alone is not routing', () => {
  const declared = `// llm-egress: no-item-body sends only dependency names\n${UNROUTED}`;
  assert.deepStrictEqual(scanLlmCallSites('src-tauri/src/x.rs', declared), []);
  const bare = `// llm-egress: no-item-body\n${UNROUTED}`;
  assert.strictEqual(scanLlmCallSites('src-tauri/src/x.rs', bare).length, 1, 'no reason, no pass');
  const commented = `// crate::llm_egress::body_allowed(&p) would go here\n${UNROUTED}`;
  assert.strictEqual(
    scanLlmCallSites('src-tauri/src/x.rs', commented).length,
    1,
    'a comment mentioning llm_egress is not a call'
  );
});

test('a client built only inside a test module is not production egress', () => {
  const testOnly = `pub fn real() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() { let c = LLMClient::new(p()); }\n}\n`;
  assert.deepStrictEqual(scanLlmCallSites('src-tauri/src/llm.rs', testOnly), []);
});

test('production code AFTER a mid-file test module is still scanned', () => {
  // blind_spots.rs has a test module mid-file; cutting at the first one hid its
  // LLM call from the gate until 2026-09-24.
  const midFile = `#[cfg(test)]\nmod early_tests {\n    #[test]\n    fn a() { assert!(true); }\n}\n${UNROUTED}`;
  assert.ok(productionPart(midFile).includes('LLMClient::with_purpose'));
  assert.strictEqual(scanLlmCallSites('src-tauri/src/blind_spots_like.rs', midFile).length, 1);
});

test('an out-of-line test module declaration does not swallow the file', () => {
  const outOfLine = `#[cfg(test)]\n#[path = "x_tests.rs"]\nmod tests;\n${UNROUTED}`;
  assert.strictEqual(scanLlmCallSites('src-tauri/src/x.rs', outOfLine).length, 1);
});
