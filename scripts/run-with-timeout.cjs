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
 */
'use strict';

const { spawn, spawnSync } = require('child_process');

const IS_WINDOWS = process.platform === 'win32';
const TIMEOUT_EXIT_CODE = 124;

/**
 * Transitive descendant PIDs of `rootPid` within a process snapshot.
 *
 * Pure function over `[{ pid, ppid }]` so the tree-walk is unit-testable
 * without spawning anything. Returns descendants only, deepest-last order is
 * not guaranteed; callers kill leaves first by reversing.
 *
 * Guards against PPID cycles (Windows recycles PIDs, and a recycled PID can
 * make a process appear to be its own ancestor).
 */
function collectDescendants(rootPid, snapshot) {
  const childrenOf = new Map();
  for (const { pid, ppid } of snapshot) {
    if (pid === ppid) continue; // self-parented: recycled PID, ignore
    if (!childrenOf.has(ppid)) childrenOf.set(ppid, []);
    childrenOf.get(ppid).push(pid);
  }

  const found = [];
  const seen = new Set([rootPid]);
  const queue = [rootPid];

  while (queue.length > 0) {
    const current = queue.shift();
    for (const child of childrenOf.get(current) || []) {
      if (seen.has(child)) continue; // cycle or diamond
      seen.add(child);
      found.push(child);
      queue.push(child);
    }
  }

  return found;
}

/**
 * Parse `ProcessId,ParentProcessId` CSV rows into `[{ pid, ppid }]`.
 * Tolerates the header row, blank lines and CRLF.
 */
function parseProcessSnapshot(text) {
  const rows = [];
  for (const rawLine of String(text).split('\n')) {
    const line = rawLine.trim();
    if (!line) continue;
    const [a, b] = line.split(',');
    const pid = Number.parseInt(a, 10);
    const ppid = Number.parseInt(b, 10);
    if (!Number.isInteger(pid) || !Number.isInteger(ppid)) continue; // header
    rows.push({ pid, ppid });
  }
  return rows;
}

/** Snapshot every process on the box as `[{ pid, ppid }]`. */
function snapshotProcesses() {
  if (IS_WINDOWS) {
    // CIM, not the removed-in-Win11 `wmic`. ConvertTo-Csv keeps it parseable
    // without depending on the console code page.
    const ps = spawnSync(
      'powershell',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        'Get-CimInstance Win32_Process | ForEach-Object { "$($_.ProcessId),$($_.ParentProcessId)" }',
      ],
      { encoding: 'utf8', timeout: 60_000 },
    );
    return parseProcessSnapshot(ps.stdout || '');
  }
  const ps = spawnSync('ps', ['-eo', 'pid=,ppid='], { encoding: 'utf8', timeout: 60_000 });
  return parseProcessSnapshot(
    (ps.stdout || '')
      .split('\n')
      .map((l) => l.trim().replace(/\s+/g, ','))
      .join('\n'),
  );
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
 */
function killTree(rootPid) {
  let descendants = [];
  try {
    descendants = collectDescendants(rootPid, snapshotProcesses());
  } catch {
    /* snapshot failed; still kill what we were handed */
  }

  for (const pid of descendants.reverse()) killPid(pid);
  killPid(rootPid);

  if (IS_WINDOWS) {
    // Belt and braces: catches anything spawned between snapshot and kill.
    try {
      spawnSync('taskkill', ['/PID', String(rootPid), '/T', '/F'], {
        stdio: 'ignore',
        timeout: 30_000,
      });
    } catch {
      /* ignore */
    }
  }

  return descendants.length;
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

  // `shell: true` so Windows resolves `pnpm`/`npx` .cmd shims. The shell
  // becomes the tree root; killTree walks down from it.
  const child = spawn(command[0], command.slice(1), { stdio: 'inherit', shell: true });

  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    console.error(
      `\n::error::run-with-timeout: "${command.join(' ')}" exceeded ${minutes} minute(s) - killing the process tree.`,
    );
    const n = killTree(child.pid);
    console.error(`run-with-timeout: killed pid ${child.pid} and ${n} descendant process(es).`);
  }, minutes * 60_000);

  child.on('error', (err) => {
    clearTimeout(timer);
    console.error(`run-with-timeout: failed to start "${command[0]}": ${err.message}`);
    process.exit(2);
  });

  child.on('exit', (code, signal) => {
    clearTimeout(timer);
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

module.exports = { collectDescendants, parseProcessSnapshot, parseArgs, killTree };
