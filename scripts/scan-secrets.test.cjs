// The secret scanner must see every staged/tracked file under its REAL name.
//
// Two defects this pins (both CodeQL js/incomplete-sanitization #19):
//   1. The file list came from newline-split `git diff --cached --name-only`,
//      which C-quotes unusual paths (core.quotePath). `café.env` arrived as
//      `"caf\303\251.env"`, named no file, and was silently skipped — the commit
//      gate passed a secret it never read.
//   2. The staged blob was read with a shell string that escaped `"` only, so a
//      filename carrying `$(...)` or backticks was executed by /bin/sh, and on
//      Windows cmd.exe expanded `%PATH%` inside the name — the read failed and
//      that file was skipped as well.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const { listFiles, readStagedContent, scanContent } = require('./scan-secrets.cjs');

// Assembled at runtime so this test file does not itself trip the scanner.
const FAKE_TOKEN = 'gh' + 'p_' + 'Q7'.repeat(20);

// Every name here is legal on Windows, macOS and Linux.
const AWKWARD_NAMES = [
  'café-creds.env', // non-ASCII: quoted by core.quotePath
  '$(echo INJECTED).txt', // command substitution under /bin/sh
  'back`tick`.txt', // backtick substitution under /bin/sh
  "it's here.txt", // quote + space
  '100%PATH%.txt', // cmd.exe variable expansion
  'plain.txt',
];

function makeRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'scan-secrets-'));
  execFileSync('git', ['init', '-q'], { cwd: dir });
  for (const name of AWKWARD_NAMES) {
    fs.writeFileSync(path.join(dir, name), `token = "${FAKE_TOKEN}"\n`);
  }
  execFileSync('git', ['-c', 'core.autocrlf=false', 'add', '--', ...AWKWARD_NAMES], { cwd: dir, stdio: 'ignore' });
  return dir;
}

test('staged listing returns the real names, not git-quoted strings', (t) => {
  const dir = makeRepo();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  assert.deepEqual(listFiles(true, dir).sort(), [...AWKWARD_NAMES].sort());
});

test('tracked listing returns the real names too', (t) => {
  const dir = makeRepo();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  assert.deepEqual(listFiles(false, dir).sort(), [...AWKWARD_NAMES].sort());
});

test('every awkwardly named staged file is read and its secret found', (t) => {
  const dir = makeRepo();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  for (const name of listFiles(true, dir)) {
    const blob = readStagedContent(name, dir);
    assert.ok(blob, `${name} must be readable from the index`);
    const findings = scanContent(blob.toString('utf8'), name, { staged: true });
    assert.ok(findings.some((f) => f.pattern === 'github'), `${name}: the staged token must be reported`);
  }
});

test('the staged blob is read even after the working copy is deleted', (t) => {
  const dir = makeRepo();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  fs.rmSync(path.join(dir, '$(echo INJECTED).txt'));
  const blob = readStagedContent('$(echo INJECTED).txt', dir);
  assert.ok(blob && blob.toString('utf8').includes(FAKE_TOKEN));
});

test('a filename is never interpreted by a shell', (t) => {
  const dir = makeRepo();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  // Were the name shell-expanded, `$(echo INJECTED)` would become `INJECTED`
  // and git would be asked for a path that does not exist -> null.
  assert.notEqual(readStagedContent('$(echo INJECTED).txt', dir), null);
  assert.equal(readStagedContent('INJECTED.txt', dir), null, 'the expanded name is not what was read');
});
