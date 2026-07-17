/**
 * Kill any process occupying a given port.
 * Usage: node scripts/kill-port.cjs 4444
 *        node scripts/kill-port.cjs vite-config
 *
 * Prevents "Error: Port 4444 is already in use" when a previous
 * dev server didn't shut down cleanly. Runs silently if the port
 * is already free.
 *
 * `vite-config` resolves the port from THIS tree's vite.config.ts instead
 * of a hardcoded number (2026-07-17): worktree lanes live-verify on bumped
 * ports (4448+), and a hardcoded `kill-port 4444` from any of them killed
 * the operator's dev server on the real port. Each launch now clears only
 * the port it is actually about to bind.
 */
'use strict';

const { execSync } = require('child_process');
let port = process.argv[2];

if (!port) {
  console.error('Usage: node scripts/kill-port.cjs <port>|vite-config');
  process.exit(1);
}

if (port === 'vite-config') {
  const fs = require('fs');
  const path = require('path');
  const viteConfig = fs.readFileSync(
    path.resolve(__dirname, '..', 'vite.config.ts'),
    'utf8',
  );
  // First `port: NNNN` in the file is the dev-server port (the hmr port
  // follows it) — the same value vite itself will bind.
  const m = viteConfig.match(/port:\s*(\d+)/);
  if (!m) {
    console.error('kill-port: no `port:` found in vite.config.ts');
    process.exit(1);
  }
  port = m[1];
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
