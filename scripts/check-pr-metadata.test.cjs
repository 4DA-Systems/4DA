// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const test = require('node:test');

const { scanPullRequestMetadata } = require('./check-pr-metadata.cjs');

// The real detector keeps the name hashed. Tests inject their own predicate so
// no literal private string ever lands in a tracked test file.
const flags = (needle) => (text) => text.includes(needle);
const never = () => false;

test('clean PR produces no findings', () => {
  const findings = scanPullRequestMetadata(
    { title: 'fix(scoring): tighten the gate', body: 'Nothing to see.', commitMessages: ['fix: a', 'fix: b'] },
    never
  );
  assert.deepEqual(findings, []);
});

test('flags the PR title — the field a squash merge uses as its subject', () => {
  const findings = scanPullRequestMetadata(
    { title: 'chore: sync with SECRETNAME', body: 'clean', commitMessages: [] },
    flags('SECRETNAME')
  );
  assert.deepEqual(findings, ['PR title']);
});

test('flags the PR body — squash merges fold it into the commit body', () => {
  const findings = scanPullRequestMetadata(
    { title: 'clean', body: 'ported from SECRETNAME', commitMessages: [] },
    flags('SECRETNAME')
  );
  assert.deepEqual(findings, ['PR body']);
});

test('flags an individual commit message and names its subject', () => {
  const findings = scanPullRequestMetadata(
    {
      title: 'clean',
      body: 'clean',
      commitMessages: ['fix: fine', 'chore: bump SECRETNAME\n\nbody text'],
    },
    flags('SECRETNAME')
  );
  assert.equal(findings.length, 1);
  assert.match(findings[0], /^commit message #2 \("chore: bump SECRETNAME"\)$/);
});

test('reports every flagged location, not just the first', () => {
  const findings = scanPullRequestMetadata(
    { title: 'SECRETNAME', body: 'SECRETNAME', commitMessages: ['SECRETNAME'] },
    flags('SECRETNAME')
  );
  assert.equal(findings.length, 3);
});

test('missing / non-string fields are tolerated, not crashed on', () => {
  assert.deepEqual(scanPullRequestMetadata({}, never), []);
  assert.deepEqual(
    scanPullRequestMetadata({ title: null, body: undefined, commitMessages: null }, never),
    []
  );
  assert.deepEqual(
    scanPullRequestMetadata({ commitMessages: [null, 42, ''] }, flags('x')),
    []
  );
});

test('empty strings are not scanned (an empty body is not a finding)', () => {
  // A predicate that flags everything would report empty fields if they were
  // scanned; they must be skipped.
  const findings = scanPullRequestMetadata(
    { title: '', body: '', commitMessages: [''] },
    () => true
  );
  assert.deepEqual(findings, []);
});
