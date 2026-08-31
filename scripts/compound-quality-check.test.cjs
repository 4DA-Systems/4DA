// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the compound-quality pre-push gate (scripts/compound-quality-check.cjs),
// specifically its "no new .unwrap() in production Rust" rule.
//
// Run: node --test scripts/compound-quality-check.test.cjs   (or `pnpm run test:scripts`)
//
// Why this exists: the rule used to classify test code by FILENAME (`*_tests.rs`)
// only. This repo overwhelmingly uses an inline `#[cfg(test)] mod tests` at the
// bottom of a production module instead — 383 files vs 71 — so the rule fired on
// almost every Rust change that added tests. A gate that cries wolf on nearly
// every PR is a gate reviewers learn to skip, so the false-positive direction is
// tested here at least as carefully as the true-positive one.

const { test } = require('node:test');
const assert = require('node:assert/strict');

const {
  cfgTestRanges,
  inTestRange,
  isProductionUnwrap,
  parseDiffGitPath,
} = require('./compound-quality-check.cjs');

// ---------------------------------------------------------------------------
// A representative module: production code first, inline test module last.
// ---------------------------------------------------------------------------
const MODULE = [
  /* 1 */ '//! A module.',
  /* 2 */ '',
  /* 3 */ 'pub fn parse(v: &str) -> Option<u32> {',
  /* 4 */ '    v.parse().ok()',
  /* 5 */ '}',
  /* 6 */ '',
  /* 7 */ '#[cfg(test)]',
  /* 8 */ 'mod tests {',
  /* 9 */ '    use super::*;',
  /* 10 */ '',
  /* 11 */ '    #[test]',
  /* 12 */ '    fn parses() {',
  /* 13 */ '        assert_eq!(parse("7").unwrap(), 7);',
  /* 14 */ '    }',
  /* 15 */ '}',
].join('\n');

// ---------------------------------------------------------------------------
// cfgTestRanges — where the test code actually is
// ---------------------------------------------------------------------------

test('finds the inline #[cfg(test)] module range', () => {
  assert.deepEqual(cfgTestRanges(MODULE), [[7, 15]]);
});

test('a file with no test module has no test ranges', () => {
  const src = ['pub fn f() {', '    let x = g().unwrap();', '}'].join('\n');
  assert.deepEqual(cfgTestRanges(src), []);
});

test('handles nested braces inside the test module', () => {
  const src = [
    /* 1 */ 'pub fn f() {}',
    /* 2 */ '#[cfg(test)]',
    /* 3 */ 'mod tests {',
    /* 4 */ '    fn helper() {',
    /* 5 */ '        if true { let _ = 1; }',
    /* 6 */ '    }',
    /* 7 */ '}',
    /* 8 */ 'pub fn after() {}',
  ].join('\n');
  assert.deepEqual(cfgTestRanges(src), [[2, 7]]);
});

test('finds multiple #[cfg(test)] items in one file', () => {
  const src = [
    /* 1 */ '#[cfg(test)]',
    /* 2 */ 'mod a { }',
    /* 3 */ 'pub fn mid() {}',
    /* 4 */ '#[cfg(test)]',
    /* 5 */ 'mod b { }',
  ].join('\n');
  assert.deepEqual(cfgTestRanges(src), [[1, 2], [4, 5]]);
});

test('an unterminated test module does not run off the end', () => {
  const src = ['#[cfg(test)]', 'mod tests {', '    fn x() {}'].join('\n');
  const ranges = cfgTestRanges(src);
  assert.equal(ranges.length, 1);
  assert.equal(ranges[0][1], 3, 'range is clamped to the last line');
});

test('#[cfg(test)] with no following brace yields no range', () => {
  assert.deepEqual(cfgTestRanges('#[cfg(test)]'), []);
});

// ---------------------------------------------------------------------------
// inTestRange — the classification itself
// ---------------------------------------------------------------------------

test('EXCUSES: an unwrap inside the inline test module', () => {
  const ranges = cfgTestRanges(MODULE);
  assert.ok(inTestRange(13, ranges), 'line 13 is the assert_eq! in mod tests');
});

test('CATCHES: an unwrap in production code above the test module', () => {
  const ranges = cfgTestRanges(MODULE);
  assert.ok(!inTestRange(4, ranges), 'line 4 is production code');
});

test('CATCHES: the boundary line just before the test module', () => {
  const ranges = cfgTestRanges(MODULE);
  assert.ok(!inTestRange(6, ranges));
});

test('EXCUSES: the #[cfg(test)] attribute line and the closing brace', () => {
  const ranges = cfgTestRanges(MODULE);
  assert.ok(inTestRange(7, ranges), 'the attribute itself');
  assert.ok(inTestRange(15, ranges), 'the closing brace');
});

test('an unlocated line (0) is never excused', () => {
  // n === 0 means the hunk header could not be parsed. The rule must stay
  // strict there rather than silently excusing an unknown line.
  assert.ok(!inTestRange(0, cfgTestRanges(MODULE)));
});

// ---------------------------------------------------------------------------
// isProductionUnwrap — the text-level filter
// ---------------------------------------------------------------------------

test('CATCHES: a plain unwrap', () => {
  assert.ok(isProductionUnwrap('    let v = thing().unwrap();'));
});

test('IGNORES: a line with no unwrap at all', () => {
  assert.ok(!isProductionUnwrap('    let v = thing()?;'));
});

test('IGNORES: an unwrap mentioned in a trailing comment', () => {
  assert.ok(!isProductionUnwrap('    let v = thing()?; // never .unwrap() here'));
});

test('CATCHES: unwrap_or is NOT excused by the comment heuristic', () => {
  // unwrap_or/unwrap_or_else are safe, but they do not match `.unwrap()`,
  // so they must not be reported at all.
  assert.ok(!isProductionUnwrap('    let v = thing().unwrap_or(0);'));
  assert.ok(!isProductionUnwrap('    let v = thing().unwrap_or_else(|_| 0);'));
});

// ---------------------------------------------------------------------------
// parseDiffGitPath — the header parse the whole rule depends on
// ---------------------------------------------------------------------------

test('parses an ordinary diff header', () => {
  assert.equal(
    parseDiffGitPath('diff --git a/src/lib.rs b/src/lib.rs'),
    'src/lib.rs',
  );
});

test('REGRESSION: a path containing "b/" is not truncated', () => {
  // `src-tauri/src/db/migrations.rs` contains `b/` inside `db/`. The old
  // `/b\/(.+)$/` matched there and produced a path that does not exist on
  // disk, so the #[cfg(test)] exclusion could never load the file and all 13
  // of that file's test-module unwraps were reported as production code.
  assert.equal(
    parseDiffGitPath(
      'diff --git a/src-tauri/src/db/migrations.rs b/src-tauri/src/db/migrations.rs',
    ),
    'src-tauri/src/db/migrations.rs',
  );
});

test('handles other b/-containing directory names', () => {
  for (const p of ['a/b/c.rs', 'src/lib/b/x.ts', 'web/b/deep/nested/file.tsx']) {
    assert.equal(parseDiffGitPath(`diff --git a/${p} b/${p}`), p);
  }
});

test('returns null for a line that is not a diff header', () => {
  assert.equal(parseDiffGitPath('+++ b/src/lib.rs'), null);
  assert.equal(parseDiffGitPath('index 1234567..89abcde 100644'), null);
  assert.equal(parseDiffGitPath(''), null);
});

test('a rename header yields the NEW path', () => {
  assert.equal(
    parseDiffGitPath('diff --git a/src/old/name.rs b/src/new/name.rs'),
    'src/new/name.rs',
  );
});

// ---------------------------------------------------------------------------
// Known blind spots — recorded so the gate is never mistaken for a proof
// ---------------------------------------------------------------------------

test('BLIND SPOT: a brace inside a string literal can end a range early', () => {
  const src = [
    /* 1 */ '#[cfg(test)]',
    /* 2 */ 'mod tests {',
    /* 3 */ '    fn x() { let s = "}"; }',
    /* 4 */ '    fn y() { let _ = f().unwrap(); }',
    /* 5 */ '}',
  ].join('\n');
  const ranges = cfgTestRanges(src);
  // The stray `}` in the string closes the range at line 3, so line 4 is
  // (wrongly) treated as production. That direction is SAFE — the gate stays
  // strict and warns; it never excuses real production code.
  assert.ok(!inTestRange(4, ranges), 'documents the conservative failure mode');
});
