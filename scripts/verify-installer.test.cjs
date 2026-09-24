// verify-installer.cjs must read the Authenticode status of the file it was
// given, whatever that file is called (CodeQL js/incomplete-sanitization #20).
//
// The old invocation spliced the path into a cmd.exe command line, escaping
// only quotes: `%VAR%` in the path was expanded by cmd.exe and `[` `]` were
// wildcards to -FilePath, so PowerShell looked for a different file and
// printed an EMPTY status — the real file's signature was never read.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { authenticodeStatus } = require('./verify-installer.cjs');

const onWindows = process.platform === 'win32';

test('reads the status of a path with cmd.exe and wildcard metacharacters', { skip: !onWindows && 'Authenticode is Windows-only' }, (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'verify-installer-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, "4DA [x64] 100%PATH% it's $env.exe");
  fs.writeFileSync(file, 'not a real PE, so certainly not signed\n');

  const status = authenticodeStatus(file);
  // Any real verdict proves PowerShell opened THIS file. A path that failed to
  // resolve would throw instead of returning a status.
  assert.match(status, /^(NotSigned|UnknownError|NotSupportedFileFormat|HashMismatch)$/);
  assert.notEqual(status, 'Valid');
});

test('a missing file throws instead of returning a status', { skip: !onWindows && 'Authenticode is Windows-only' }, () => {
  assert.throws(
    () => authenticodeStatus(path.join(os.tmpdir(), 'definitely-not-here-4da.exe')),
    /definitely-not-here-4da|not found|Cannot find/i,
  );
});
