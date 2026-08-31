// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  collectDescendants,
  parseProcessSnapshot,
  parseArgs,
} = require('./run-with-timeout.cjs');

const SCRIPT = path.join(__dirname, 'run-with-timeout.cjs');

// --- tree walk -------------------------------------------------------------

test('collectDescendants finds grandchildren, not just direct children', () => {
  const snapshot = [
    { pid: 1, ppid: 0 },
    { pid: 10, ppid: 1 }, // root
    { pid: 20, ppid: 10 }, // child
    { pid: 30, ppid: 20 }, // grandchild
    { pid: 40, ppid: 30 }, // great-grandchild
    { pid: 99, ppid: 1 }, // unrelated - MUST NOT be killed
  ];

  const found = collectDescendants(10, snapshot).sort((a, b) => a - b);
  assert.deepEqual(found, [20, 30, 40]);
});

test('collectDescendants never returns the root itself', () => {
  const found = collectDescendants(10, [
    { pid: 10, ppid: 1 },
    { pid: 20, ppid: 10 },
  ]);
  assert.ok(!found.includes(10));
});

test('collectDescendants survives a PPID cycle from a recycled PID', () => {
  // Windows recycles PIDs; a recycled PID can make the graph cyclic. A naive
  // walk loops forever and the watchdog never kills anything.
  const snapshot = [
    { pid: 10, ppid: 20 },
    { pid: 20, ppid: 10 },
    { pid: 30, ppid: 20 },
  ];
  const found = collectDescendants(10, snapshot).sort((a, b) => a - b);
  assert.deepEqual(found, [20, 30]);
});

test('collectDescendants ignores a self-parented process', () => {
  const found = collectDescendants(10, [
    { pid: 10, ppid: 10 },
    { pid: 20, ppid: 10 },
  ]);
  assert.deepEqual(found, [20]);
});

test('collectDescendants returns nothing for a childless root', () => {
  assert.deepEqual(collectDescendants(10, [{ pid: 10, ppid: 1 }]), []);
});

// --- snapshot parsing ------------------------------------------------------

test('parseProcessSnapshot skips headers and blank lines', () => {
  const rows = parseProcessSnapshot(
    'ProcessId,ParentProcessId\r\n100,4\r\n\r\n200,100\r\n',
  );
  assert.deepEqual(rows, [
    { pid: 100, ppid: 4 },
    { pid: 200, ppid: 100 },
  ]);
});

// --- argument parsing ------------------------------------------------------

test('parseArgs splits flags from the command at "--"', () => {
  const { minutes, command } = parseArgs(['--minutes', '15', '--', 'pnpm', 'run', 'test']);
  assert.equal(minutes, 15);
  assert.deepEqual(command, ['pnpm', 'run', 'test']);
});

test('parseArgs keeps a second "--" as part of the command', () => {
  // `pnpm run test -- --run` is exactly the CI invocation; the inner `--`
  // must survive intact or vitest gets the wrong argv.
  const { command } = parseArgs(['--minutes', '15', '--', 'pnpm', 'run', 'test', '--', '--run']);
  assert.deepEqual(command, ['pnpm', 'run', 'test', '--', '--run']);
});

test('parseArgs rejects a missing separator, command, or bad minutes', () => {
  assert.throws(() => parseArgs(['--minutes', '15']), /missing "--" separator/);
  assert.throws(() => parseArgs(['--minutes', '15', '--']), /no command given/);
  assert.throws(() => parseArgs(['--', 'echo']), /missing required --minutes/);
  assert.throws(() => parseArgs(['--minutes', '0', '--', 'echo']), /positive number/);
  assert.throws(() => parseArgs(['--minutes', 'abc', '--', 'echo']), /positive number/);
});

// --- the claim that actually matters --------------------------------------

function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (err) {
    return err.code === 'EPERM'; // exists but not ours
  }
}

test(
  'timeout kills the whole tree, not just the spawned command',
  { timeout: 120_000 },
  () => {
    // This is the regression under test. GitHub's job cancellation killed the
    // TOP of the tree and orphaned everything below it, which is how a hung
    // vitest kept burning CPU and held the runner's only slot. The watchdog
    // must kill LEAVES FIRST while the tree is still intact, so nothing is
    // left re-parented and running.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), '4da-timeout-'));
    try {
      const pidFile = path.join(dir, 'grandchild.pid');
      const grandchild = path.join(dir, 'grandchild.js');
      const parent = path.join(dir, 'parent.js');

      fs.writeFileSync(
        grandchild,
        `require('node:fs').writeFileSync(process.argv[2], String(process.pid));
         setTimeout(() => {}, 300000);`,
      );
      fs.writeFileSync(
        parent,
        `require('node:child_process').spawn(
           process.execPath, [${JSON.stringify(grandchild)}, process.argv[2]],
           { stdio: 'ignore' });
         setTimeout(() => {}, 300000);`,
      );

      const started = Date.now();
      const res = spawnSync(
        process.execPath,
        [SCRIPT, '--minutes', '0.15', '--', process.execPath, parent, pidFile],
        { encoding: 'utf8', timeout: 90_000 },
      );
      const elapsed = Date.now() - started;

      assert.equal(res.status, 124, `expected timeout exit 124, got ${res.status}\n${res.stderr}`);
      // 0.15 min = 9s; must not have waited anywhere near the 300s the tree wanted.
      assert.ok(elapsed < 60_000, `watchdog took ${elapsed}ms - it should fire at ~9s`);

      const grandchildPid = Number(fs.readFileSync(pidFile, 'utf8').trim());
      assert.ok(Number.isInteger(grandchildPid) && grandchildPid > 0, 'grandchild never started');

      // The grandchild is two levels below the process we spawned (shell ->
      // node parent -> node grandchild). If the walk only handled direct
      // children, or relied on `taskkill /T` finding a re-parented process,
      // this is where it fails.
      assert.equal(
        isAlive(grandchildPid),
        false,
        `grandchild pid ${grandchildPid} survived the tree kill`,
      );
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
);

test('a command that finishes in time passes its own exit code through', { timeout: 60_000 }, () => {
  const ok = spawnSync(
    process.execPath,
    [SCRIPT, '--minutes', '5', '--', process.execPath, '-e', 'process.exit(0)'],
    { encoding: 'utf8', timeout: 50_000 },
  );
  assert.equal(ok.status, 0, ok.stderr);

  // A real test failure must still fail the step - the watchdog must not
  // swallow non-zero codes on its way out.
  const fail = spawnSync(
    process.execPath,
    [SCRIPT, '--minutes', '5', '--', process.execPath, '-e', 'process.exit(7)'],
    { encoding: 'utf8', timeout: 50_000 },
  );
  assert.equal(fail.status, 7, fail.stderr);
});
