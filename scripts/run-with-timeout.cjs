#!/usr/bin/env node
/**
 * Run a command under a hard wall-clock deadline, and on expiry kill the whole
 * PROCESS TREE - not just the command we spawned.
 *
 * Usage:
 *   node scripts/run-with-timeout.cjs --minutes 15 -- pnpm run test -- --run
 *
 * Exit code: the child's own code, or 124 (GNU `timeout` convention) if the
 * deadline fired.
 *
 * WHY THIS EXISTS (2026-08-31 CI outage)
 * --------------------------------------
 * The Frontend job's `vitest run` hung on the single self-hosted Windows
 * runner. The job's `timeout-minutes: 30` fired and GitHub cancelled the job -
 * but cancellation did NOT kill the process tree on Windows. `vitest.mjs`
 * re-parented, survived its dead `pnpm` parent, kept burning CPU, and held the
 * runner's only slot. Every later self-hosted job queued for over an hour
 * while the API cheerfully reported the runner idle.
 *
 * Nothing inside vitest bounds that. Verified against vitest 3.2.6 + tinypool
 * 1.1.1 in this repo's node_modules:
 *
 *   - `testTimeout` bounds an individual test, inside the worker. It cannot
 *     see a wedge between test files.
 *   - `teardownTimeout` is passed to tinypool as `terminateTimeout` AND arms
 *     vitest's own force-exit watchdog - but that watchdog is armed inside
 *     `Vitest.exit()`, i.e. only once the run has already finished. A hang
 *     during the run never reaches it.
 *   - tinypool's `WorkerInfo.destroy(timeout)` awaits the worker's `teardown`
 *     task with NO timeout at all; its `timeout` timer is armed only AFTER
 *     that await resolves. With `isolate: true` + `maxForks: 1` that teardown
 *     round-trip happens between EVERY test file, so a wedged worker parks the
 *     main process forever, un-timed-out.
 *   - `process.exit()` in the vitest main process does not reap forked
 *     workers on Windows; they orphan and survive.
 *
 * So the only reliable bound is external, and it has to kill descendants that
 * may ALREADY have been re-parented. `taskkill /T` alone is not enough: it
 * walks live ParentProcessId links, so a grandchild whose parent already died
 * is invisible to it - which is precisely the shape of this outage. We instead
 * snapshot every process, compute the transitive descendant set ourselves, and
 * kill by explicit PID.
 *
 * WHY THE GENERATION CHECK (2026-09-01)
 * -------------------------------------
 * Walking ParentProcessId downwards has the mirror-image hazard of walking it
 * upwards: Windows never clears a dead parent's PID out of a survivor's
 * ParentProcessId field, and it recycles PIDs aggressively. A process whose
 * real parent died hours ago still advertises that dead PID as its parent
 * forever. The moment something new is handed that recycled PID, the stale
 * process looks - to a pure PPID walk - like its child.
 *
 * Measured on the self-hosted runner (2026-09-01): of 424 live processes, 18
 * carried a ParentProcessId pointing at a PID that no longer exists, including
 * `explorer.exe`, `Lightshot.exe`, a pair of long-running local daemons and
 * Docker's backend. The worst of them: pids 21416/21456 (the two `cmd.exe` that
 * host `Runner.Listener.exe`, and below one of them `Runner.Worker.exe`) claim dead
 * pids 13976/13984 as parents. If a timed-out job's shell were ever handed
 * either of those PIDs, the walk would adopt the CI runner and kill it - the
 * exact outage this script exists to prevent, caused by the fix for it.
 *
 * The check that closes this is a lineage impossibility, not a heuristic: a
 * real child cannot have been created BEFORE its parent. And the adoption case
 * is always an inversion, because a PID can only be recycled after its previous
 * owner exits - so the stale process necessarily pre-dates whatever now holds
 * the PID it points at. We therefore carry each process's creation time in the
 * snapshot and refuse any edge whose child is strictly older than its claimed
 * parent, at EVERY hop of the walk. Ties are accepted: coarse timestamp
 * resolution can only collapse a real ordering into equality, never invert it,
 * so the check has no false rejects. On the same 424-process snapshot it fired
 * on exactly 2 edges, both kernel pseudo-processes (`Secure System`,
 * `Registry`) hanging off PID 4, neither reachable from any tree we walk.
 */
'use strict';

const { spawn, spawnSync } = require('child_process');

const IS_WINDOWS = process.platform === 'win32';
const TIMEOUT_EXIT_CODE = 124;

/**
 * How long to wait for the child to actually die after we kill its tree before
 * exiting under our own power. Without this the wrapper hangs on a `killTree`
 * that failed, and the job reverts to GitHub's 20-minute cancel - which is the
 * orphan-making mechanism this script replaces.
 */
const KILL_GRACE_MS = 30_000;

/**
 * Transitive descendant PIDs of `rootPid` within a process snapshot.
 *
 * Pure function over `[{ pid, ppid, created }]` so the tree-walk is
 * unit-testable without spawning anything. `created` is epoch-ms (or null when
 * the platform could not supply it). Returns descendants only, deepest-last
 * order is not guaranteed; callers kill leaves first by reversing.
 *
 * Two independent guards against Windows PID recycling:
 *   - PPID cycles (a recycled PID can make a process look like its own
 *     ancestor) are broken by the `seen` set.
 *   - Stale-PPID ADOPTION - the far more dangerous case, because it silently
 *     widens the kill into unrelated processes - is rejected by the generation
 *     check: a child that pre-dates its claimed parent is not its child.
 *
 * `fallbackRootCreated` is used only if the snapshot has no creation time for
 * the root itself (e.g. it exited between the timeout firing and the snapshot).
 * Pass the moment we spawned it; that slightly UNDER-states the root's true
 * creation time, which can only make the guard more permissive, never less.
 */
function collectDescendants(rootPid, snapshot, fallbackRootCreated = null) {
  const childrenOf = new Map();
  const createdOf = new Map();

  for (const { pid, ppid, created } of snapshot) {
    if (Number.isFinite(created)) createdOf.set(pid, created);
    if (pid === ppid) continue; // self-parented: recycled PID, ignore
    if (!childrenOf.has(ppid)) childrenOf.set(ppid, []);
    childrenOf.get(ppid).push(pid);
  }

  if (!createdOf.has(rootPid) && Number.isFinite(fallbackRootCreated)) {
    createdOf.set(rootPid, fallbackRootCreated);
  }

  const found = [];
  const seen = new Set([rootPid]);
  const queue = [rootPid];

  while (queue.length > 0) {
    const current = queue.shift();
    const parentCreated = createdOf.get(current);

    for (const child of childrenOf.get(current) || []) {
      if (seen.has(child)) continue; // cycle or diamond

      // Generation check. Deliberately NOT added to `seen` on rejection: this
      // PID may still be a genuine descendant via some other edge, and must
      // stay eligible for that path.
      const childCreated = createdOf.get(child);
      if (
        Number.isFinite(parentCreated) &&
        Number.isFinite(childCreated) &&
        childCreated < parentCreated
      ) {
        continue; // impossible lineage - a stale PPID pointing at a recycled PID
      }

      seen.add(child);
      found.push(child);
      queue.push(child);
    }
  }

  return found;
}

/**
 * Parse `ProcessId,ParentProcessId[,createdEpochMs]` CSV rows into
 * `[{ pid, ppid, created }]`. Tolerates the header row, blank lines, CRLF and
 * a missing/blank third column (`created` is then null and the generation
 * check simply does not apply to that row).
 */
function parseProcessSnapshot(text) {
  const rows = [];
  for (const rawLine of String(text).split('\n')) {
    const line = rawLine.trim();
    if (!line) continue;
    const [a, b, c] = line.split(',');
    const pid = Number.parseInt(a, 10);
    const ppid = Number.parseInt(b, 10);
    if (!Number.isInteger(pid) || !Number.isInteger(ppid)) continue; // header
    const created = Number.parseInt(c, 10);
    rows.push({ pid, ppid, created: Number.isInteger(created) ? created : null });
  }
  return rows;
}

/** Turn `ps` whitespace columns into the CSV shape `parseProcessSnapshot` wants. */
function posixRowsToCsv(stdout, nowMs) {
  return String(stdout || '')
    .split('\n')
    .map((line) => {
      const parts = line.trim().split(/\s+/);
      if (parts.length < 2) return '';
      // `etimes` is whole seconds of elapsed run time; convert to the same
      // epoch-ms basis Windows reports so one comparison covers both.
      if (parts.length >= 3 && nowMs !== null) {
        const secs = Number.parseInt(parts[2], 10);
        const created = Number.isInteger(secs) ? nowMs - secs * 1000 : '';
        return `${parts[0]},${parts[1]},${created}`;
      }
      return `${parts[0]},${parts[1]},`;
    })
    .join('\n');
}

/** Fail loudly rather than silently degrading to a root-only kill. */
function assertSpawnOk(res, what) {
  if (res.error) throw new Error(`${what} failed to run: ${res.error.message}`);
  if (res.signal) throw new Error(`${what} was killed by ${res.signal}`);
  if (res.status !== 0) {
    const detail = String(res.stderr || '')
      .trim()
      .split('\n')[0]
      .slice(0, 200);
    throw new Error(`${what} exited ${res.status}${detail ? `: ${detail}` : ''}`);
  }
}

/**
 * Snapshot every process on the box as `[{ pid, ppid, created }]`.
 *
 * THROWS on failure. It must: a snapshot that quietly comes back empty leaves
 * `killTree` doing a root-only kill while still reporting "0 descendants",
 * which reads as a clean success and hides exactly the re-parented survivors
 * this script exists to reap.
 */
function snapshotProcesses() {
  let text;

  if (IS_WINDOWS) {
    // CIM, not the removed-in-Win11 `wmic`. Emitting the creation time as
    // Unix ms keeps it locale-independent and inside Number's safe range
    // (a raw FILETIME tick count is not).
    const ps = spawnSync(
      'powershell',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        'Get-CimInstance Win32_Process | ForEach-Object { $c = \'\'; ' +
          'if ($_.CreationDate) { $c = [long]([DateTimeOffset]$_.CreationDate).ToUnixTimeMilliseconds() }; ' +
          '"$($_.ProcessId),$($_.ParentProcessId),$c" }',
      ],
      { encoding: 'utf8', timeout: 60_000 },
    );
    assertSpawnOk(ps, 'Win32_Process snapshot');
    text = ps.stdout || '';
  } else {
    // `etimes` is Linux `ps`; BSD/macOS `ps` rejects it. Losing it costs only
    // the generation check, so fall back rather than fail the whole kill.
    const withAge = spawnSync('ps', ['-eo', 'pid=,ppid=,etimes='], {
      encoding: 'utf8',
      timeout: 60_000,
    });
    if (!withAge.error && withAge.status === 0 && String(withAge.stdout || '').trim()) {
      text = posixRowsToCsv(withAge.stdout, Date.now());
    } else {
      const plain = spawnSync('ps', ['-eo', 'pid=,ppid='], { encoding: 'utf8', timeout: 60_000 });
      assertSpawnOk(plain, 'ps snapshot');
      text = posixRowsToCsv(plain.stdout, null);
    }
  }

  const rows = parseProcessSnapshot(text);
  if (rows.length === 0) throw new Error('process snapshot returned no usable rows');
  return rows;
}

/** Kill one PID as hard as the platform allows. Never throws. */
function killPid(pid) {
  try {
    if (IS_WINDOWS) {
      // PID-scoped. NEVER an image-name kill (`/IM node.exe`) - this runner is
      // the operator's dev box and shares it with the fleet.
      spawnSync('taskkill', ['/PID', String(pid), '/F'], { stdio: 'ignore', timeout: 30_000 });
    } else {
      process.kill(pid, 'SIGKILL');
    }
  } catch {
    /* already gone - that is the desired end state */
  }
}

/**
 * Kill `rootPid` and every descendant, including ones already re-parented away
 * from it. Leaves are killed first so a dying parent cannot spawn a replacement.
 *
 * Returns `{ killed, snapshotError }`. `snapshotError` is non-null when the
 * process snapshot failed, in which case `killed` is 0 because we knew nothing
 * about the tree - NOT because the tree was empty. Callers must report the two
 * cases differently.
 *
 * `snapshotFn` is an injection seam so the failure path can be proven in tests
 * without breaking the host's PATH; production always uses the default.
 */
function killTree(rootPid, fallbackRootCreated = null, snapshotFn = snapshotProcesses) {
  let descendants = [];
  let snapshotError = null;

  try {
    descendants = collectDescendants(rootPid, snapshotFn(), fallbackRootCreated);
  } catch (err) {
    // Still kill what we were handed - a root-only kill beats no kill - but
    // the caller has to say so out loud.
    snapshotError = err.message || String(err);
  }

  for (const pid of descendants.reverse()) killPid(pid);
  killPid(rootPid);

  if (IS_WINDOWS) {
    // Belt and braces: catches anything spawned between snapshot and kill.
    // `/T` walks only LIVE parent links, so it adds nothing a stale PPID could
    // widen - it cannot see re-parented processes at all, which is why the
    // explicit PID pass above exists.
    try {
      spawnSync('taskkill', ['/PID', String(rootPid), '/T', '/F'], {
        stdio: 'ignore',
        timeout: 30_000,
      });
    } catch {
      /* ignore */
    }
  }

  return { killed: descendants.length, snapshotError };
}

/** Split argv into `{ minutes, command }` around the `--` separator. */
function parseArgs(argv) {
  const sep = argv.indexOf('--');
  if (sep === -1) throw new Error('missing "--" separator before the command');

  const flags = argv.slice(0, sep);
  const command = argv.slice(sep + 1);
  if (command.length === 0) throw new Error('no command given after "--"');

  const idx = flags.indexOf('--minutes');
  if (idx === -1 || !flags[idx + 1]) throw new Error('missing required --minutes <n>');

  const minutes = Number(flags[idx + 1]);
  if (!Number.isFinite(minutes) || minutes <= 0) {
    throw new Error(`--minutes must be a positive number, got "${flags[idx + 1]}"`);
  }

  return { minutes, command };
}

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (err) {
    console.error(`run-with-timeout: ${err.message}`);
    console.error('usage: node scripts/run-with-timeout.cjs --minutes <n> -- <command> [args...]');
    process.exit(2);
  }

  const { minutes, command } = parsed;
  const started = Date.now();

  // `shell: true` on WINDOWS ONLY, so `pnpm`/`npx` .cmd shims resolve; there
  // the shell becomes the tree root and killTree walks down from it.
  //
  // Deliberately NOT on POSIX. With `shell: true` Node runs `/bin/sh -c` on a
  // string it builds by joining argv with SPACES and no quoting, so sh re-parses
  // every argument: `-- node -e 'process.exit(0)'` died with
  //   /bin/sh: 1: Syntax error: "(" unexpected
  // and the watchdog exited 2 having run nothing. Any argument carrying `(`,
  // `;`, `&`, `|`, `*` or a space was silently at risk. POSIX resolves
  // executables from PATH without a shell, so drop it and hand argv through
  // verbatim; killTree walks descendants of the child either way. Caught the
  // day this script's own tests were finally wired into CI.
  const child = spawn(command[0], command.slice(1), {
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  let timedOut = false;
  let graceTimer = null;

  const timer = setTimeout(() => {
    timedOut = true;
    console.error(
      `\n::error::run-with-timeout: "${command.join(' ')}" exceeded ${minutes} minute(s) - killing the process tree.`,
    );

    const { killed, snapshotError } = killTree(child.pid, started);
    if (snapshotError) {
      console.error(
        `::error::run-with-timeout: process snapshot FAILED (${snapshotError}) - killed pid ${child.pid} directly, ` +
          'but the descendant set was never computed and re-parented processes may have survived.',
      );
    } else {
      console.error(
        `run-with-timeout: killed pid ${child.pid} and ${killed} descendant process(es).`,
      );
    }

    // If the kill did not actually end the child, exiting on our own is the
    // only safe move: waiting hands the job back to GitHub's 20-minute cancel,
    // which is precisely the orphan-making mechanism this script replaces.
    graceTimer = setTimeout(() => {
      console.error(
        `::error::run-with-timeout: pid ${child.pid} still had not exited ${KILL_GRACE_MS / 1000}s after the kill - ` +
          'giving up and exiting so the job cannot hang.',
      );
      process.exit(TIMEOUT_EXIT_CODE);
    }, KILL_GRACE_MS);
  }, minutes * 60_000);

  child.on('error', (err) => {
    clearTimeout(timer);
    if (graceTimer) clearTimeout(graceTimer);
    console.error(`run-with-timeout: failed to start "${command[0]}": ${err.message}`);
    process.exit(2);
  });

  child.on('exit', (code, signal) => {
    clearTimeout(timer);
    if (graceTimer) clearTimeout(graceTimer);
    const secs = ((Date.now() - started) / 1000).toFixed(1);
    if (timedOut) {
      console.error(`run-with-timeout: timed out after ${secs}s.`);
      process.exit(TIMEOUT_EXIT_CODE);
    }
    if (signal) {
      console.error(`run-with-timeout: child terminated on ${signal} after ${secs}s.`);
      process.exit(1);
    }
    process.exit(code ?? 1);
  });
}

if (require.main === module) main();

module.exports = {
  collectDescendants,
  parseProcessSnapshot,
  parseArgs,
  killTree,
  snapshotProcesses,
  assertSpawnOk,
  KILL_GRACE_MS,
};
