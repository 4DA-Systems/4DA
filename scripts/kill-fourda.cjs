/**
 * Kill any running fourda.exe THAT WAS BUILT FROM THIS TREE before starting dev.
 * Usage: node scripts/kill-fourda.cjs
 *
 * Why this exists: on Windows, `cargo run` can't replace the fourda.exe
 * binary while it's executing — Windows file-locks the running exe. This
 * causes "Access is denied. (os error 5)" during dev restart and leaves
 * the OLD fourda.exe process running. That old process has stale module
 * paths cached (especially after refactors like game-* → fourda-*),
 * causing the webview to show a black screen when the frontend imports
 * paths the old binary can't resolve.
 *
 * SCOPED TO THIS TREE (2026-07-17): the fleet runs multiple checkouts —
 * the shared root tree (the operator's daily driver) plus isolated
 * worktrees doing live verification. The file-lock this script clears is
 * on THIS tree's target\debug\fourda.exe; an instance built from another
 * tree locks a different file. The old indiscriminate kill meant every
 * worktree dev launch executed the operator's running app (observed live:
 * repeated daily-driver deaths whenever any lane live-verified). Only
 * instances whose ExecutablePath lives under this tree are killed.
 *
 * Runs silently if no matching fourda.exe process exists.
 */
'use strict';

const { execSync } = require('child_process');
const path = require('path');

if (process.platform !== 'win32') {
  // No-op on macOS/Linux — Unix replaces running binaries cleanly.
  process.exit(0);
}

// The tree this dev launch runs from (package.json lives next to scripts/).
const treeRoot = path.resolve(__dirname, '..').toLowerCase();

try {
  // ExecutablePath per PID — tasklist can't provide paths. -EncodedCommand
  // sidesteps cmd.exe quoting entirely (a plain -c string gets its `$(...)`
  // mangled by the shell — caught live before shipping).
  const psScript =
    'Get-CimInstance Win32_Process -Filter "Name=\'fourda.exe\'" | ' +
    'ForEach-Object { "$($_.ProcessId)|$($_.ExecutablePath)" }';
  const encoded = Buffer.from(psScript, 'utf16le').toString('base64');
  const output = execSync(`powershell -nop -EncodedCommand ${encoded}`, {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  const pids = [];
  for (const line of output.trim().split('\n')) {
    const [pid, exePath] = line.trim().split('|');
    if (!pid || !/^\d+$/.test(pid)) continue;
    const p = (exePath || '').toLowerCase();
    if (p.startsWith(treeRoot)) {
      pids.push(pid);
    } else if (p) {
      console.log(
        `Leaving fourda.exe PID ${pid} alone — built from another tree (${exePath})`,
      );
    }
  }

  if (pids.length === 0) {
    // No fourda.exe from this tree — clean start, nothing to kill
    return;
  }

  for (const pid of pids) {
    try {
      execSync(`taskkill /F /PID ${pid}`, { stdio: 'pipe' });
      console.log(`Killed stale fourda.exe (PID ${pid}) — prevents file-lock on rebuild`);
    } catch {
      // Process may have exited between listing and kill — safe to ignore
    }
  }

  // Brief pause to let the OS release the file handle before cargo tries to write
  // 250ms is enough in practice; reduces "Access is denied" race conditions
  execSync('powershell -nop -c "Start-Sleep -Milliseconds 250"', { stdio: 'pipe' });
} catch {
  // Process listing failed — likely no matching process, safe to proceed
}
