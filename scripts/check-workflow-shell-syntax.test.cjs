'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  run,
  extractRunBlocks,
  stripExpr,
  resolvePowerShell,
} = require('./check-workflow-shell-syntax.cjs');

// ubuntu runners have `pwsh` but no `powershell`; a bare container may have
// neither. Where no parser exists the gate reports the block inconclusive, so
// these cases would assert against a skip rather than a check. Skip them
// honestly instead of asserting something the environment cannot answer.
const NO_PS = resolvePowerShell() ? false : 'no pwsh or powershell on PATH';

/** Write a throwaway workflow dir containing the given files. */
function fixture(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wf-syntax-test-'));
  for (const [name, body] of Object.entries(files)) {
    fs.writeFileSync(path.join(dir, name), body);
  }
  return dir;
}

const PS_STEP = (body) => `name: T
on: [push]
jobs:
  win:
    runs-on: windows-latest
    steps:
      - name: Verify something
        shell: powershell
        run: |
${body
  .split('\n')
  .map((l) => (l ? `          ${l}` : ''))
  .join('\n')}
`;

const JOB_DEFAULT_PWSH = (body) => `name: T
on: [push]
jobs:
  win:
    runs-on: windows-latest
    defaults:
      run:
        shell: pwsh
    steps:
      - name: Inherits pwsh from job defaults
        run: |
${body
  .split('\n')
  .map((l) => (l ? `          ${l}` : ''))
  .join('\n')}
`;

test('accepts a well-formed PowerShell block', { skip: NO_PS }, () => {
  const dir = fixture({ 'a.yml': PS_STEP('if ($x -gt 0) {\n  Write-Host "ok"\n} else {\n  exit 1\n}') });
  const { checked, failures } = run(dir);
  assert.strictEqual(failures.length, 0, JSON.stringify(failures));
  assert.strictEqual(checked, 1);
});

test('catches the stray closing brace that broke the v1.0.1 release', { skip: NO_PS }, () => {
  // This is the exact shape of the defect: the if/else is already balanced,
  // then one extra `}` follows.
  const body = 'if ($failed -gt 0) {\n  Write-Host "warn"\n}\n}\nWrite-Host "done"';
  const dir = fixture({ 'release.yml': PS_STEP(body) });
  const { failures } = run(dir);
  assert.strictEqual(failures.length, 1);
  assert.match(failures[0].detail, /Unexpected token/i);
  assert.strictEqual(failures[0].step, 'Verify something');
});

test('catches an unterminated PowerShell block', { skip: NO_PS }, () => {
  const dir = fixture({ 'a.yml': PS_STEP('if ($x) {\n  Write-Host "no close"') });
  const { failures } = run(dir);
  assert.strictEqual(failures.length, 1);
});

test('honours job-level defaults.run.shell (pwsh is not parsed as bash)', { skip: NO_PS }, () => {
  // Without job-default resolution this PowerShell would be handed to
  // `bash -n` and reported as a syntax error that does not exist.
  const dir = fixture({ 'a.yml': JOB_DEFAULT_PWSH('if ($x -ne 0) {\n  Write-Output "fine"\n}') });
  const { failures, checked } = run(dir);
  assert.strictEqual(failures.length, 0, JSON.stringify(failures));
  assert.strictEqual(checked, 1);
});

test('a step-level shell overrides the job default', () => {
  const yml = `name: T
on: [push]
jobs:
  j:
    runs-on: ubuntu-latest
    defaults:
      run:
        shell: pwsh
    steps:
      - name: Actually bash
        shell: bash
        run: |
          if [ -n "$X" ]; then echo yes; fi
`;
  const dir = fixture({ 'a.yml': yml });
  const { failures } = run(dir);
  assert.strictEqual(failures.length, 0, JSON.stringify(failures));
});

test('job scope resets between jobs', { skip: NO_PS }, () => {
  const yml = `name: T
on: [push]
jobs:
  first:
    runs-on: windows-latest
    defaults:
      run:
        shell: pwsh
    steps:
      - name: ps step
        run: |
          Write-Host "hi"
  second:
    runs-on: ubuntu-latest
    steps:
      - name: bash step
        run: |
          if [ 1 -eq 1 ]; then echo ok; fi
`;
  const dir = fixture({ 'a.yml': yml });
  const { failures, checked } = run(dir);
  assert.strictEqual(failures.length, 0, JSON.stringify(failures));
  assert.strictEqual(checked, 2);
});

test('catches a bash syntax error', () => {
  const yml = `name: T
on: [push]
jobs:
  j:
    runs-on: ubuntu-latest
    steps:
      - name: broken bash
        run: |
          if [ -n "$X" ]; then
            echo missing fi
`;
  const dir = fixture({ 'a.yml': yml });
  const { failures } = run(dir);
  assert.strictEqual(failures.length, 1);
  assert.strictEqual(failures[0].step, 'broken bash');
});

test('GitHub expressions do not themselves cause a parse failure', { skip: NO_PS }, () => {
  const dir = fixture({
    'a.yml': PS_STEP('$dir = "src-tauri\\target\\${{ matrix.target }}\\release"\nWrite-Host $dir'),
  });
  const { failures } = run(dir);
  assert.strictEqual(failures.length, 0, JSON.stringify(failures));
});

test('stripExpr replaces every expression, not just the first', () => {
  assert.strictEqual(stripExpr('${{ a }} and ${{ b }}'), 'GHA_EXPR and GHA_EXPR');
});

test('a missing workflow directory is a failure, not a silent pass', () => {
  const { failures } = run(path.join(os.tmpdir(), 'definitely-not-a-real-dir-4da'));
  assert.strictEqual(failures.length, 1);
  assert.match(failures[0].detail, /missing/i);
});

test('an unhandled shell is skipped, never counted as clean', () => {
  const yml = `name: T
on: [push]
jobs:
  j:
    runs-on: ubuntu-latest
    steps:
      - name: python step
        shell: python
        run: |
          print("hello")
`;
  const dir = fixture({ 'a.yml': yml });
  const { checked, failures, skipped } = run(dir);
  assert.strictEqual(failures.length, 0);
  assert.strictEqual(checked, 0);
  assert.strictEqual(skipped.length, 1);
});

test('extractRunBlocks records where each shell decision came from', () => {
  const dir = fixture({ 'a.yml': JOB_DEFAULT_PWSH('Write-Host "x"') });
  const blocks = extractRunBlocks(path.join(dir, 'a.yml'));
  assert.strictEqual(blocks.length, 1);
  assert.strictEqual(blocks[0].shell, 'pwsh');
  assert.strictEqual(blocks[0].shellSource, 'job-default');
});

test('single-line `run:` values are ignored (only block scalars are parsed)', () => {
  const yml = `name: T
on: [push]
jobs:
  j:
    runs-on: ubuntu-latest
    steps:
      - name: one liner
        run: pnpm run build
`;
  const dir = fixture({ 'a.yml': yml });
  const { checked, failures } = run(dir);
  assert.strictEqual(failures.length, 0);
  assert.strictEqual(checked, 0);
});
