// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  collectDescendants,
  parseProcessSnapshot,
  parseArgs,
  killTree,
  snapshotProcesses,
  assertSpawnOk,
  KILL_GRACE_MS,
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

// --- the generation check --------------------------------------------------
//
// Windows never clears a dead parent's PID out of a survivor's
// ParentProcessId, and it recycles PIDs. So a stale process can point at a PID
// that something unrelated now holds, and a pure PPID walk adopts it - and its
// whole subtree - into the kill. Measured on the self-hosted runner
// (2026-09-01): 18 of 424 live processes carried a stale PPID, including the
// two `cmd.exe` that host `Runner.Listener.exe`. The impossibility that closes
// this: a real child cannot pre-date its parent.

test('collectDescendants refuses a stale-PPID process older than its claimed parent', () => {
  // pid 500 is our freshly spawned shell. pid 700 has pointed at PID 500 since
  // long before that PID was recycled - it is NOT a descendant.
  const snapshot = [
    { pid: 500, ppid: 1, created: 5_000 }, // root, spawned just now
    { pid: 600, ppid: 500, created: 5_100 }, // real child
    { pid: 700, ppid: 500, created: 1_000 }, // stale PPID - predates the root
  ];

  const found = collectDescendants(500, snapshot).sort((a, b) => a - b);
  assert.deepEqual(found, [600], 'adopted a process that pre-dates its claimed parent');
});

test('the generation check applies at every hop, not just the first', () => {
  // The impostor hangs off a genuine grandchild, three levels down. A check
  // applied only to the root's direct children would let this straight in.
  const snapshot = [
    { pid: 500, ppid: 1, created: 5_000 },
    { pid: 600, ppid: 500, created: 5_100 },
    { pid: 610, ppid: 600, created: 5_200 },
    { pid: 800, ppid: 610, created: 900 }, // stale PPID at depth 3
    { pid: 810, ppid: 800, created: 950 }, // ...and its subtree
  ];

  const found = collectDescendants(500, snapshot).sort((a, b) => a - b);
  assert.deepEqual(found, [600, 610], 'adopted a stale-PPID subtree at a deeper hop');
});

test('the whole runner subtree stays out of the kill', () => {
  // The live case, with the real PIDs measured on the runner on 2026-09-01:
  // cmd.exe 21416/21456 claim dead pids 13976/13984 as parents and host
  // Runner.Listener.exe / Runner.Worker.exe. If a timed-out job's shell were
  // handed PID 13976, the walk would kill the CI runner out from under itself.
  const snapshot = [
    { pid: 13976, ppid: 1, created: 9_000_000 }, // our shell, on a recycled PID
    { pid: 21416, ppid: 13976, created: 1_000 }, // cmd.exe - stale, from the dead 13976
    { pid: 23104, ppid: 21416, created: 1_100 }, // Runner.Listener.exe
    { pid: 26112, ppid: 23104, created: 1_200 }, // Runner.Worker.exe
    { pid: 21500, ppid: 21416, created: 1_050 }, // conhost.exe
  ];

  assert.deepEqual(collectDescendants(13976, snapshot), []);
});

test('equal creation times are accepted - a real child may tie with its parent', () => {
  // Coarse timestamp resolution can collapse a real ordering into equality but
  // can never invert it, so ties must pass or the guard starts leaving genuine
  // orphans alive - the very outage it sits inside.
  const found = collectDescendants(500, [
    { pid: 500, ppid: 1, created: 5_000 },
    { pid: 600, ppid: 500, created: 5_000 },
  ]);
  assert.deepEqual(found, [600]);
});

test('a missing creation time disables the check for that edge, it does not reject', () => {
  const found = collectDescendants(500, [
    { pid: 500, ppid: 1, created: 5_000 },
    { pid: 600, ppid: 500, created: null }, // platform gave us nothing
    { pid: 700, ppid: 500 }, // field absent entirely
  ]).sort((a, b) => a - b);
  assert.deepEqual(found, [600, 700]);
});

test('a rejected child stays eligible via a legitimate edge', () => {
  // Synthetic double-edge (real snapshots give each PID one parent) probing the
  // internal invariant: a generation rejection must not add the PID to `seen`,
  // or one bad edge would permanently hide a genuine descendant. Here 700 is
  // rejected under 600 and must still be found under 650.
  const found = collectDescendants(500, [
    { pid: 500, ppid: 1, created: 5_000 },
    { pid: 600, ppid: 500, created: 9_000 },
    { pid: 650, ppid: 500, created: 5_100 },
    { pid: 700, ppid: 600, created: 5_200 }, // rejected under 600...
    { pid: 700, ppid: 650, created: 5_200 }, // ...accepted under 650
  ]).sort((a, b) => a - b);
  assert.deepEqual(found, [600, 650, 700]);
});

test('the root falls back to a caller-supplied creation time when absent from the snapshot', () => {
  // If the root exited between the timeout firing and the snapshot, we still
  // know when we spawned it - and the guard must keep working.
  const snapshot = [{ pid: 700, ppid: 500, created: 1_000 }];
  assert.deepEqual(collectDescendants(500, snapshot, 5_000), []);
  // ...and with no fallback there is nothing to compare against, so it passes.
  assert.deepEqual(collectDescendants(500, snapshot), [700]);
});

// --- snapshot parsing ------------------------------------------------------

test('parseProcessSnapshot skips headers and blank lines', () => {
  const rows = parseProcessSnapshot(
    'ProcessId,ParentProcessId\r\n100,4\r\n\r\n200,100\r\n',
  );
  assert.deepEqual(rows, [
    { pid: 100, ppid: 4, created: null },
    { pid: 200, ppid: 100, created: null },
  ]);
});

test('parseProcessSnapshot reads the creation-time column', () => {
  const rows = parseProcessSnapshot('100,4,1788018563255\r\n200,100,\r\n300,100,junk\r\n');
  assert.deepEqual(rows, [
    { pid: 100, ppid: 4, created: 1788018563255 },
    { pid: 200, ppid: 100, created: null },
    { pid: 300, ppid: 100, created: null },
  ]);
});

test('snapshotProcesses supplies a real creation time for this very process', () => {
  // The guard is inert without this column, and a silently-2-column snapshot
  // would look identical to a working one in every other test.
  const rows = snapshotProcesses();
  const self = rows.find((r) => r.pid === process.pid);
  assert.ok(self, `own pid ${process.pid} missing from the snapshot`);
  assert.ok(Number.isFinite(self.created), 'snapshot carried no creation time');
  assert.ok(self.created > 0 && self.created <= Date.now() + 5_000, `implausible: ${self.created}`);
});

// --- snapshot failure must be loud, not a false "0 descendants" ------------

test('assertSpawnOk rejects a failed, signalled or non-zero snapshot', () => {
  assert.throws(() => assertSpawnOk({ error: new Error('ENOENT') }, 'snap'), /snap failed to run/);
  assert.throws(() => assertSpawnOk({ signal: 'SIGKILL' }, 'snap'), /killed by SIGKILL/);
  assert.throws(() => assertSpawnOk({ status: 1, stderr: 'boom\nmore' }, 'snap'), /exited 1: boom/);
  assert.doesNotThrow(() => assertSpawnOk({ status: 0 }, 'snap'));
});

test('killTree reports a failed snapshot instead of a clean "0 descendants"', { timeout: 60_000 }, () => {
  // Before: the snapshot failure was swallowed and the caller printed
  // "killed pid N and 0 descendant process(es)" - indistinguishable from a
  // genuinely childless tree, while re-parented survivors kept running.
  // killTree kills its root, so the root here is a process we spawned.
  const child = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 120000)'], {
    stdio: 'ignore',
  });
  try {
    const outcome = killTree(child.pid, Date.now(), () => {
      throw new Error('simulated snapshot failure');
    });
    assert.equal(outcome.killed, 0);
    assert.match(outcome.snapshotError, /simulated snapshot failure/);

    // A successful snapshot must NOT report an error, so the two cases are
    // genuinely distinguishable by the caller.
    const ok = killTree(child.pid, Date.now(), () => [{ pid: child.pid, ppid: 1, created: 1 }]);
    assert.equal(ok.snapshotError, null);
  } finally {
    try {
      child.kill('SIGKILL');
    } catch {
      /* already dead - the desired end state */
    }
  }
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

// --- the wrapper must always exit, even if the kill fails ------------------

test('the watchdog arms a bounded self-exit deadline after killing the tree', () => {
  // Deliberately a STRUCTURAL assertion. The only way this backstop fires in
  // production is the OS refusing `taskkill /F` (or SIGKILL), which cannot be
  // staged from a test - so what is provable is that the deadline is armed
  // unconditionally and is bounded. Without it, a killTree that failed to reap
  // the child left the wrapper parked until GitHub's 20-minute job cancel, the
  // exact orphan-making mechanism this whole script exists to replace.
  assert.ok(Number.isFinite(KILL_GRACE_MS), 'KILL_GRACE_MS must be a finite bound');
  assert.ok(KILL_GRACE_MS > 0 && KILL_GRACE_MS <= 120_000, `implausible grace: ${KILL_GRACE_MS}`);

  const src = fs.readFileSync(SCRIPT, 'utf8');
  const start = src.indexOf('const timer = setTimeout(');
  const end = src.indexOf('}, minutes * 60_000);', start);
  assert.ok(start !== -1 && end > start, 'could not locate the watchdog timeout handler');
  const handler = src.slice(start, end);

  assert.match(handler, /killTree\(/, 'the handler no longer kills the tree');
  assert.match(
    handler,
    /graceTimer = setTimeout\(/,
    'the post-kill deadline is not armed unconditionally - the wrapper can hang again',
  );
  assert.match(
    handler,
    /process\.exit\(TIMEOUT_EXIT_CODE\)/,
    'the post-kill deadline does not actually exit',
  );
});

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
