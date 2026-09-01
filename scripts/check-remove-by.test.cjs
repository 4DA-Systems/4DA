'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { analyzeSources, resolveToday, isSkippedFile } = require('./check-remove-by.cjs');

const TODAY = '2026-08-12';

/** Convenience: analyze a single in-memory source. */
function analyze(src, allow = [], today = TODAY) {
  return analyzeSources([{ rel: 'fixture.rs', src }], today, allow);
}

test('flags a marker whose date has passed', () => {
  const { expired, upcoming } = analyze(`
    // REMOVE BY 2026-08-01
    fn temporary() {}
  `);
  assert.strictEqual(expired.length, 1);
  assert.strictEqual(upcoming.length, 0);
  assert.strictEqual(expired[0].date, '2026-08-01');
  assert.strictEqual(expired[0].lineNo, 2);
});

test('a future marker is upcoming, not expired', () => {
  const { expired, upcoming } = analyze(`
    // REMOVE BY 2026-12-31
    fn later() {}
  `);
  assert.strictEqual(expired.length, 0);
  assert.strictEqual(upcoming.length, 1);
  assert.strictEqual(upcoming[0].date, '2026-12-31');
});

test('the deadline day itself counts as expired (matches .husky/pre-commit)', () => {
  const { expired } = analyze('// REMOVE BY 2026-08-12');
  assert.strictEqual(expired.length, 1, 'EXPIRY <= TODAY is expired');
});

test('the day after the deadline is still valid (and now also flagged due-soon)', () => {
  const { expired, dueSoon, upcoming } = analyze('// REMOVE BY 2026-08-13');
  assert.strictEqual(expired.length, 0, 'tomorrow is not expired — nothing blocks');
  assert.strictEqual(dueSoon.length, 1, 'but it is inside the 14-day runway');
  assert.strictEqual(upcoming.length, 0);
});

test('a marker inside the runway warns but never blocks', () => {
  // The whole point: this must not reach `expired`, or the runway becomes the
  // outage it exists to prevent.
  const { expired, dueSoon } = analyze('// REMOVE BY 2026-08-20'); // today + 8
  assert.strictEqual(expired.length, 0);
  assert.strictEqual(dueSoon.length, 1);
  assert.strictEqual(dueSoon[0].date, '2026-08-20');
});

test('the runway boundary is inclusive at +14 days and clear at +15', () => {
  const inside = analyze('// REMOVE BY 2026-08-26'); // today + 14
  assert.strictEqual(inside.dueSoon.length, 1, '+14 days is inside the runway');
  assert.strictEqual(inside.upcoming.length, 0);

  const outside = analyze('// REMOVE BY 2026-08-27'); // today + 15
  assert.strictEqual(outside.dueSoon.length, 0, '+15 days is not yet warned about');
  assert.strictEqual(outside.upcoming.length, 1);
});

test('the runway crosses a month boundary without arithmetic drift', () => {
  // Date maths, in a gate that exists because of a date bug. today + 12.
  const { expired, dueSoon } = analyze('// REMOVE BY 2026-09-01', [], '2026-08-20');
  assert.strictEqual(expired.length, 0);
  assert.strictEqual(dueSoon.length, 1, 'Aug 20 + 12d = Sep 1, still inside 14 days');
});

test('accepts YYYY/MM/DD and normalizes it to YYYY-MM-DD', () => {
  const { expired } = analyze('// REMOVE BY 2026/08/01');
  assert.strictEqual(expired.length, 1);
  assert.strictEqual(expired[0].date, '2026-08-01', 'slashes normalized to dashes');
});

test('finds every marker across multiple lines and files', () => {
  const { expired } = analyzeSources(
    [
      { rel: 'a.rs', src: '// REMOVE BY 2026-01-01\n// REMOVE BY 2026-02-02\n' },
      { rel: 'b.ts', src: '// REMOVE BY 2026-03-03\n' },
    ],
    TODAY
  );
  assert.strictEqual(expired.length, 3);
  assert.deepStrictEqual(
    expired.map((e) => e.rel),
    ['a.rs', 'a.rs', 'b.ts']
  );
});

test('two markers on the SAME line are both reported', () => {
  const { expired } = analyze('// REMOVE BY 2026-01-01 and REMOVE BY 2026-02-02');
  assert.strictEqual(expired.length, 2);
});

test('CRLF line endings do not corrupt the reported text', () => {
  const { expired } = analyze('// REMOVE BY 2026-08-01\r\nfn f() {}\r\n');
  assert.strictEqual(expired.length, 1);
  assert.ok(!expired[0].text.includes('\r'), 'trailing CR stripped from reported text');
});

test('an allowlist entry suppresses a matching expired marker', () => {
  const allow = [{ file: 'fixture.rs', date: '2026-08-01', ticket: '#123' }];
  const { expired } = analyze('// REMOVE BY 2026-08-01', allow);
  assert.strictEqual(expired.length, 1, 'still reported...');
  assert.strictEqual(expired[0].allowlisted, true, '...but marked allowlisted (non-blocking)');
});

test('an allowlist entry for a DIFFERENT date does not suppress', () => {
  const allow = [{ file: 'fixture.rs', date: '2026-07-01', ticket: '#123' }];
  const { expired } = analyze('// REMOVE BY 2026-08-01', allow);
  assert.strictEqual(expired[0].allowlisted, false);
});

test('an allowlist entry for a DIFFERENT file does not suppress', () => {
  const allow = [{ file: 'other.rs', date: '2026-08-01', ticket: '#123' }];
  const { expired } = analyze('// REMOVE BY 2026-08-01', allow);
  assert.strictEqual(expired[0].allowlisted, false);
});

test('allowlist paths are compared with forward slashes', () => {
  const allow = [{ file: 'src\\thing.rs', date: '2026-08-01', ticket: '#1' }];
  const { expired } = analyzeSources(
    [{ rel: 'src/thing.rs', src: '// REMOVE BY 2026-08-01' }],
    TODAY,
    allow
  );
  assert.strictEqual(expired[0].allowlisted, true, 'backslashes normalized');
});

test('a malformed allowlist entry is ignored rather than throwing', () => {
  const allow = [null, {}, { file: 'fixture.rs' }, { date: '2026-08-01' }];
  const { expired } = analyze('// REMOVE BY 2026-08-01', allow);
  assert.strictEqual(expired[0].allowlisted, false);
});

test('a backtick-quoted marker is a citation, not a live deadline', () => {
  const { expired, upcoming } = analyze(
    '/// the `REMOVE BY 2026-08-01` marker was cleared when the shim was deleted'
  );
  assert.strictEqual(expired.length, 0);
  assert.strictEqual(upcoming.length, 0);
});

test('a real marker adjacent to prose is still caught', () => {
  const { expired } = analyze('// tidy this up. REMOVE BY 2026-08-01 (tracked in #99)');
  assert.strictEqual(expired.length, 1);
});

test('text without a valid date is not a marker', () => {
  const { expired, upcoming } = analyze(`
    // REMOVE BY soon
    // REMOVE BY 26-08-01
    // REMOVEBY 2026-08-01
  `);
  assert.strictEqual(expired.length, 0);
  assert.strictEqual(upcoming.length, 0);
});

test('resolveToday honours the REMOVE_BY_TODAY override', () => {
  assert.strictEqual(resolveToday({ REMOVE_BY_TODAY: '2020-01-01' }), '2020-01-01');
  assert.strictEqual(resolveToday({ REMOVE_BY_TODAY: 'garbage' }).length, 10, 'falls back to real today');
});

test('resolveToday returns a zero-padded ISO date', () => {
  assert.match(resolveToday({}), /^\d{4}-\d{2}-\d{2}$/);
});

test('test files are skipped so fixture markers cannot self-trip the gate', () => {
  // This very file is full of `REMOVE BY <date>` literals.
  assert.strictEqual(isSkippedFile('scripts/check-remove-by.test.cjs'), true);
  assert.strictEqual(isSkippedFile('src/components/Foo.test.tsx'), true);
  assert.strictEqual(isSkippedFile('src/lib/bar.spec.ts'), true);
  assert.strictEqual(isSkippedFile('src-tauri/src/scoring/pipeline_tests.rs'), true);
  assert.strictEqual(isSkippedFile('src/components/__tests__/Baz.tsx'), true);
  assert.strictEqual(isSkippedFile('src-tauri\\tests\\thing.rs'), true, 'backslash paths too');
});

test('production files are NOT skipped', () => {
  assert.strictEqual(isSkippedFile('src-tauri/src/suns/mod.rs'), false);
  assert.strictEqual(isSkippedFile('src/lib/commands.ts'), false);
  assert.strictEqual(isSkippedFile('scripts/check-file-sizes.cjs'), false);
  assert.strictEqual(isSkippedFile('src/latest/thing.ts'), false, '"latest" must not match /tests?/');
});
