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
const fs = require('fs');
const path = require('path');

function findObjectBodyAfterProperty(source, propertyName) {
  const match = new RegExp(`\\b${propertyName}\\s*:\\s*\\{`).exec(source);
  if (!match) return null;

  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  let bodyStart = -1;
  let quote = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;

  for (let i = openBrace; i < source.length; i++) {
    const ch = source[i];
    const next = source[i + 1];

    if (lineComment) {
      if (ch === '\n') lineComment = false;
      continue;
    }
    if (blockComment) {
      if (ch === '*' && next === '/') {
        blockComment = false;
        i++;
      }
      continue;
    }
    if (quote) {
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }

    if (ch === '/' && next === '/') {
      lineComment = true;
      i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      blockComment = true;
      i++;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '{') {
      depth++;
      if (depth === 1) bodyStart = i + 1;
      continue;
    }
    if (ch === '}') {
      depth--;
      if (depth === 0) return source.slice(bodyStart, i);
    }
  }
  return null;
}

function findTopLevelNumericProperty(source, propertyName) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;

  for (let i = 0; i < source.length; i++) {
    const ch = source[i];
    const next = source[i + 1];

    if (lineComment) {
      if (ch === '\n') lineComment = false;
      continue;
    }
    if (blockComment) {
      if (ch === '*' && next === '/') {
        blockComment = false;
        i++;
      }
      continue;
    }
    if (quote) {
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }

    if (ch === '/' && next === '/') {
      lineComment = true;
      i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      blockComment = true;
      i++;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '{') {
      depth++;
      continue;
    }
    if (ch === '}') {
      depth--;
      continue;
    }
    if (depth !== 0) continue;

    const rest = source.slice(i);
    const match = rest.match(new RegExp(`^\\s*${propertyName}\\s*:\\s*(\\d+)\\b`));
    if (match) return match[1];
  }
  return null;
}

function resolveViteConfigPort(viteConfig) {
  const serverBody = findObjectBodyAfterProperty(viteConfig, 'server');
  if (!serverBody) return null;
  return findTopLevelNumericProperty(serverBody, 'port');
}

function isValidPort(port) {
  const n = Number(port);
  return Number.isInteger(n) && n > 0 && n <= 65535 && String(port) === String(n);
}

function resolvePortArg(rawArg, repoRoot = path.resolve(__dirname, '..')) {
  if (!rawArg) {
    throw new Error('Usage: node scripts/kill-port.cjs <port>|vite-config');
  }
  if (rawArg !== 'vite-config') {
    if (!isValidPort(rawArg)) throw new Error(`kill-port: invalid port '${rawArg}'`);
    return rawArg;
  }

  const viteConfig = fs.readFileSync(path.resolve(repoRoot, 'vite.config.ts'), 'utf8');
  const port = resolveViteConfigPort(viteConfig);
  if (!port) throw new Error('kill-port: no top-level `server.port` found in vite.config.ts');
  return port;
}

function killPort(port) {
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
}

function main() {
  let port;
  try {
    port = resolvePortArg(process.argv[2]);
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }

  try {
    killPort(port);
  } catch {
    // No process on port — nothing to kill, all good
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  findObjectBodyAfterProperty,
  findTopLevelNumericProperty,
  resolvePortArg,
  resolveViteConfigPort,
};
