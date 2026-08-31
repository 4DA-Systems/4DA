// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the security-surface detectors in scripts/generate-knowledge.cjs,
// which decide what lands in `.claude/knowledge/security-surface.md` and, via
// sentinel-scan.cjs CHECK 3, what raises a Security alert.
//
// Run: node --test scripts/generate-knowledge.test.cjs   (or `pnpm run test:scripts`)
//
// WHY THESE EXIST (2026-08-25/26 audit):
//
// These detectors were wrong in BOTH directions, and neither failure was
// visible from the numbers alone:
//
//   * Too loose — all 23 "API key in logs" and all 30 "SQL string formatting"
//     CRITICAL findings were false positives. They matched TEXT, not meaning:
//     LLM token COUNTS, key NAMES in message strings, prose containing
//     "update to >=", Windows EdgeUpdate registry paths, and
//     `format!("{selection_pct:.1}")` matching "SELECT". Two CRITICAL gates
//     sat permanently red, which is how a gate stops being read.
//
//   * Too tight — the first attempt at a "precise" replacement MISSED
//     `format!("SELECT * FROM users WHERE name = '{user_input}'")`, the single
//     most common SQL injection shape, because it required `=` to be followed
//     immediately by `{` with no quote between. It reported a clean green.
//
// A count dropping to zero looks identical whether a detector was fixed or
// merely blinded. The only way to tell them apart is to feed it a real
// violation and assert that it fires. That is what this file does — and it is
// wired into `test:scripts`, so the CI "Repo guards" job runs it on every PR.

const { test } = require('node:test');
const assert = require('node:assert/strict');

const {
  isCommentLine,
  logsSecretValue,
  isSqlFormat,
  sqlInterpolatesValue,
} = require('./generate-knowledge.cjs');

const entry = (text) => ({ file: 'src-tauri/src/x.rs', line: 1, text });
const logs = (text) => logsSecretValue(entry(text));
const sqlValue = (text) => isSqlFormat(entry(text)) && sqlInterpolatesValue(entry(text));
const sqlIdent = (text) => isSqlFormat(entry(text)) && !sqlInterpolatesValue(entry(text));

// ---------------------------------------------------------------------------
// Secrets reaching a log sink — MUST fire
// ---------------------------------------------------------------------------

test('CATCHES: secret value interpolated into a log message', () => {
  assert.ok(logs('tracing::info!("configured with {api_key}");'));
});

test('CATCHES: secret as a tracing field value', () => {
  assert.ok(logs('tracing::warn!(api_key = %api_key, "auth failed");'));
});

test('CATCHES: bare tracing shorthand field carrying a secret', () => {
  assert.ok(logs('tracing::error!(target: "x", secret, "boom");'));
});

test('CATCHES: password interpolated via println', () => {
  assert.ok(logs('println!("pw={password}");'));
});

test('CATCHES: struct field access holding a token', () => {
  // Regression: a blanket `_tokens?\b` metering filter swallowed this, so
  // `bearer_token` was unreportable. Found by this file on its first run.
  assert.ok(logs('debug!("using {}", cfg.bearer_token);'));
});

test('CATCHES: access_token / auth_token / refresh_token are credentials', () => {
  assert.ok(logs('info!("t={access_token}");'), 'access_token');
  assert.ok(logs('info!("t={auth_token}");'), 'auth_token');
  assert.ok(logs('info!("t={refresh_token}");'), 'refresh_token');
  assert.ok(logs('info!("t={session_token}");'), 'session_token');
});

// ---------------------------------------------------------------------------
// Secrets NOT reaching a log sink — MUST NOT fire
// (every one of these was a real CRITICAL false positive before the fix)
// ---------------------------------------------------------------------------

test('ignores: LLM token COUNTS, not credentials', () => {
  assert.ok(!logs('info!(target: "4da::llm", input_tokens, output_tokens, cost_cents = cost, "ok");'));
});

test('ignores: usage metering args', () => {
  assert.ok(!logs('Self::format_limit_error(tokens_used, tokens_limit, cost_used, cost_limit).into(),'));
});

test('ignores: key NAME in message text, value never logged', () => {
  assert.ok(!logs('tracing::warn!(target: "4da::keystore", "Keychain unavailable for llm_api_key - plaintext fallback");'));
});

test('ignores: the log line announcing secrets WILL be zeroized', () => {
  assert.ok(!logs('tracing::info!(target: "4da::security", "Crash guard installed - secrets will be zeroized on panic");'));
});

test('ignores: webhook id plus prose about a secret', () => {
  assert.ok(!logs('info!(target: "4da::webhooks", webhook_id = %webhook_id, "Webhook secret stored in keychain");'));
});

test('ignores: an event NAME that contains "token"', () => {
  assert.ok(!logs('tracing::warn!("Failed to emit \'synthesis-token\': {e}");'));
});

test('ignores: `info` as a struct field, not a log macro at all', () => {
  assert.ok(!logs('let input_cost_per_token = info.input_cost_per_token?;'));
});

// ---------------------------------------------------------------------------
// SQL — value position (bindable) MUST be CRITICAL
// ---------------------------------------------------------------------------

test('CATCHES: quoted value interpolation - the textbook injection', () => {
  // Regression: the first "precise" rewrite missed this exact shape.
  assert.ok(sqlValue(`let _ = format!("SELECT * FROM users WHERE name = '{user_input}'");`));
});

test('CATCHES: unquoted value interpolation after =', () => {
  assert.ok(sqlValue('let _ = format!("SELECT * FROM t WHERE id = {id}");'));
});

test('CATCHES: raw list interpolated into IN ()', () => {
  assert.ok(sqlValue('let _ = format!("DELETE FROM items WHERE id IN ({raw_ids})");'));
});

test('CATCHES: LIKE pattern interpolation', () => {
  assert.ok(sqlValue(`let _ = format!("SELECT a FROM t WHERE n LIKE '{needle}'");`));
});

// ---------------------------------------------------------------------------
// SQL — identifier position is REVIEW, not CRITICAL
// (SQLite cannot bind a table/column name, so interpolation is unavoidable)
// ---------------------------------------------------------------------------

test('identifier interpolation is REVIEW, not CRITICAL', () => {
  assert.ok(sqlIdent('let _ = format!("SELECT DISTINCT {path_col} FROM {table}");'));
});

test('`IN ({placeholders})` is the CORRECT list binding, not a finding', () => {
  assert.ok(sqlIdent('let _ = format!("DELETE FROM {table} WHERE id IN ({placeholders})");'));
});

test('pragma_table_info identifier is REVIEW, not CRITICAL', () => {
  assert.ok(sqlIdent(`let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1");`));
});

test('bound parameter after = is not an interpolation', () => {
  assert.ok(sqlIdent('let _ = format!("DELETE FROM {table} WHERE rowid = ?1");'));
});

// ---------------------------------------------------------------------------
// SQL — prose that merely CONTAINS a SQL verb is not SQL
// ---------------------------------------------------------------------------

test('ignores: prose opening with a SQL verb', () => {
  // Regression: leading whitespace after the quote made this parse as UPDATE
  // with `>= {f}` in a value position — a CRITICAL false positive.
  assert.ok(!isSqlFormat(entry('.map(|f| format!(" Update to >= {f}."))')));
});

test('ignores: "Updated N facts" - UPDATE needs a word boundary', () => {
  assert.ok(!isSqlFormat(entry('message: format!("Updated {facts_found} hardware facts"),')));
});

test('ignores: prose containing "updates"', () => {
  assert.ok(!isSqlFormat(entry('_ => format!("{name} updates this week"),')));
});

test('ignores: a percentage that contains the letters "select"', () => {
  assert.ok(!isSqlFormat(entry('selection_pct = format!("{selection_pct:.1}"),')));
});

test('ignores: a Windows registry path containing EdgeUpdate', () => {
  assert.ok(!isSqlFormat(entry('format!("HKLM\\\\SOFTWARE\\\\Microsoft\\\\EdgeUpdate\\\\Clients\\\\{CLIENT_GUID}"),')));
});

test('ignores: a URL query containing "recent-updates"', () => {
  assert.ok(!isSqlFormat(entry('let url = format!("{API_BASE}/crates?sort=recent-updates&per_page={max}");')));
});

// ---------------------------------------------------------------------------
// Comment lines are documentation, not code
// ---------------------------------------------------------------------------

test('doc comment describing panic! is not a panic site', () => {
  assert.ok(isCommentLine('/// `panic!("literal")` boxes a `&\'static str` and `panic!("{fmt}", ..)` boxes a'));
});

test('plain line comment is a comment', () => {
  assert.ok(isCommentLine('    // panic!("nope")'));
});

test('real code is not a comment', () => {
  assert.ok(!isCommentLine('    _ => panic!("Unknown benchmark profile: {name}"),'));
});
