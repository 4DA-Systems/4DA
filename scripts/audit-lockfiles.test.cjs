// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The guard's whole value is that it DISCOVERS lockfiles instead of listing
 * them. A green audit run proves the four the old loop already covered; only
 * these tests prove the two it did not, and that nobody can quietly narrow it
 * back to a list.
 */
const test = require('node:test');
const assert = require('node:assert');
const {
  discoverLockfiles,
  commandFor,
  looksLikeHarnessError,
  annotationBody,
  KNOWN,
} = require('./audit-lockfiles.cjs');

/**
 * The four the replaced CI loops hardcoded
 * (`for entry in ".:--prod" "site:" "paddle-webhook:" "mcp-4da-server:"`).
 */
const OLD_HARDCODED_DIRS = ['.', 'site', 'paddle-webhook', 'mcp-4da-server'];

test('discovery finds every tracked lockfile, not just the pnpm ones', () => {
  const found = discoverLockfiles();
  // Whatever else moves, these must be in the set: they are the two the old
  // loops missed, and the miss shipped a high advisory (js-yaml 4.3.1,
  // GHSA-2883-xcg3-v3hh) in the vscode extension for as long as it existed.
  assert.ok(
    found.includes('editors/vscode/4da/package-lock.json'),
    `the vscode extension's npm lockfile must be audited — found: ${found.join(', ')}`,
  );
  assert.ok(
    found.includes('mcp-memory-server/package-lock.json'),
    `the memory server's npm lockfile must be audited — found: ${found.join(', ')}`,
  );
  assert.ok(found.includes('pnpm-lock.yaml'), 'the root lockfile must be audited');
});

test('THE case: discovery covers strictly more than the list it replaced', () => {
  const dirs = new Set(discoverLockfiles().map((f) => commandFor(f).dir));
  for (const d of OLD_HARDCODED_DIRS) {
    assert.ok(dirs.has(d), `regression: ${d} was covered by the old loop and must stay covered`);
  }
  assert.ok(
    dirs.size > OLD_HARDCODED_DIRS.length,
    `discovery must cover MORE than the ${OLD_HARDCODED_DIRS.length} hardcoded dirs, got ${dirs.size} — ` +
      'if this fails, the audit has been narrowed back to a list',
  );
});

test('each lockfile format gets the tool that can read it', () => {
  const pnpm = commandFor('site/pnpm-lock.yaml');
  assert.strictEqual(pnpm.file, 'npx');
  assert.ok(pnpm.args.includes('audit'));
  assert.ok(pnpm.args.some((a) => a.startsWith('pnpm@')), 'pnpm audit needs the 11+ one-shot pin');

  const npm = commandFor('editors/vscode/4da/package-lock.json');
  assert.strictEqual(npm.file, 'npm');
  assert.deepStrictEqual(npm.args.slice(0, 1), ['audit']);
  assert.strictEqual(npm.cwd, 'editors/vscode/4da', 'npm has no --dir; it audits its CWD');
});

test('pnpm runs from the repo root, because --dir is relative to the CWD', () => {
  // Passing both `cwd: site` and `--dir site` made pnpm look for `site/site`
  // and report `ENOENT` under the heading "advisories in site".
  const c = commandFor('site/pnpm-lock.yaml');
  assert.strictEqual(c.cwd, '.');
  assert.deepStrictEqual(
    c.args.slice(c.args.indexOf('--dir'), c.args.indexOf('--dir') + 2),
    ['--dir', 'site'],
  );
});

test('only the root pnpm lockfile is audited --prod', () => {
  assert.ok(commandFor('pnpm-lock.yaml').args.includes('--prod'), 'root keeps the prod-only scope');
  for (const f of ['site/pnpm-lock.yaml', 'paddle-webhook/pnpm-lock.yaml']) {
    assert.ok(!commandFor(f).args.includes('--prod'), `${f} audits its whole closure`);
  }
});

test('a lockfile format with no audit tool FAILS rather than being skipped', () => {
  assert.ok(KNOWN.has('yarn.lock'), 'yarn.lock must be recognised even though none is tracked');
  const c = commandFor('somewhere/yarn.lock');
  assert.strictEqual(c.unsupported, true, 'an unwired format must not silently pass');
});

test('a tool that could not run is never reported as advisories', () => {
  // Both tools exit 1 for "found something" AND "could not run". Confusing the
  // two sends the reader to bump a dependency that was never the problem.
  assert.strictEqual(
    looksLikeHarnessError("[ERROR] ENOENT: no such file or directory, lstat 'site/site'"),
    true,
  );
  assert.strictEqual(looksLikeHarnessError('npm: command not found'), true);
  assert.strictEqual(looksLikeHarnessError('For help, run: pnpm help audit'), true);
  // …but a real audit result is not a harness error, even when it mentions a path.
  assert.strictEqual(looksLikeHarnessError('2 vulnerabilities (1 moderate, 1 high)'), false);
  assert.strictEqual(looksLikeHarnessError('No known vulnerabilities found'), false);
  assert.strictEqual(looksLikeHarnessError('found 0 vulnerabilities'), false);
});

test('the annotation survives being a single line', () => {
  const body = annotationBody(
    'js-yaml  4.0.0 - 4.3.1\nSeverity: high\n2 vulnerabilities (1 moderate, 1 high)',
  );
  assert.ok(!body.includes('\n'), 'a raw newline truncates a GitHub annotation');
  assert.match(body, /Severity: high/);
  assert.match(body, /%0A/);
});
