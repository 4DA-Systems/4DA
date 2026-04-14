/**
 * Kill any running fourda.exe process before starting dev.
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
 * Runs silently if no fourda.exe process exists.
 */
'use strict';

const { execSync } = require('child_process');

if (process.platform !== 'win32') {
  // No-op on macOS/Linux — Unix replaces running binaries cleanly.
  process.exit(0);
}

try {
  // Find all fourda.exe processes (there may be multiple from orphaned runs)
  const output = execSync('tasklist /FI "IMAGENAME eq fourda.exe" /FO CSV /NH', {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  const pids = [];
  for (const line of output.trim().split('\n')) {
    // CSV format: "fourda.exe","12345","Console","1","64,304 K"
    const match = line.match(/"fourda\.exe","(\d+)"/);
    if (match) pids.push(match[1]);
  }

  if (pids.length === 0) {
    // No running fourda.exe — clean start, nothing to kill
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
  // tasklist failed — likely no matching process, safe to proceed
}
