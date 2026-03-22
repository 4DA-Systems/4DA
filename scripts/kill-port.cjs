/**
 * Kill any process occupying a given port.
 * Usage: node scripts/kill-port.cjs 4444
 *
 * Prevents "Error: Port 4444 is already in use" when a previous
 * dev server didn't shut down cleanly. Runs silently if the port
 * is already free.
 */
'use strict';

const { execSync } = require('child_process');
const port = process.argv[2];

if (!port) {
  console.error('Usage: node scripts/kill-port.cjs <port>');
  process.exit(1);
}

try {
  if (process.platform === 'win32') {
    // Find PID listening on the port
    const output = execSync(`netstat -ano | findstr :${port} | findstr LISTENING`, {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const pids = new Set();
    for (const line of output.trim().split('\n')) {
      const parts = line.trim().split(/\s+/);
      const pid = parts[parts.length - 1];
      if (pid && pid !== '0' && /^\d+$/.test(pid)) {
        pids.add(pid);
      }
    }
    for (const pid of pids) {
      try {
        execSync(`taskkill /F /PID ${pid}`, { stdio: 'pipe' });
        console.log(`Killed stale process on port ${port} (PID ${pid})`);
      } catch {
        // Process may have already exited
      }
    }
  } else {
    // macOS / Linux
    const output = execSync(`lsof -ti :${port}`, {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const pids = output.trim().split('\n').filter(Boolean);
    for (const pid of pids) {
      try {
        execSync(`kill -9 ${pid}`, { stdio: 'pipe' });
        console.log(`Killed stale process on port ${port} (PID ${pid})`);
      } catch {
        // Process may have already exited
      }
    }
  }
} catch {
  // No process on port — nothing to kill, all good
}
