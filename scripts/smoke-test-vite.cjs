#!/usr/bin/env node
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Vite Cold-Start Smoke Test
 *
 * Starts a fresh Vite dev server, requests critical entry points, and verifies
 * every module resolves without "Cannot find module" errors.
 *
 * Catches the class of bug where a dependency update leaves stale paths in a
 * running process — we literally lived through this on 2026-04-11 when
 * updating vite 8.0.4 → 8.0.8 left the running fourda.exe holding phantom
 * references to vite@8.0.4 + old @emnapi paths.
 *
 * This script proves a cold-start is clean BEFORE the user ever opens the app.
 */

const { spawn } = require('node:child_process');
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');

const PORT = 4444;
const DEV_HOST = `http://localhost:${PORT}`;
const STARTUP_TIMEOUT_MS = 30000;
const REQUEST_TIMEOUT_MS = 20000;

// Critical modules that MUST resolve on a cold start.
// main.tsx is last because it imports the entire app tree — requesting lighter
// modules first gives Vite's dep optimizer time to finish pre-bundling before
// the heavy entry point is fetched (avoids 10s timeout on CI cold cache).
const CRITICAL_ROUTES = [
  '/src/App.tsx',
  '/src/store/index.ts',
  '/src/lib/commands.ts',
  '/src/lib/trust-feedback.ts',
  '/src/components/ViewRouter.tsx',
  '/src/components/ViewTabBar.tsx',
  '/src/components/preemption/PreemptionView.tsx',
  '/src/components/blindspots/BlindSpotsView.tsx',
  '/src/components/trust/TrustDashboard.tsx',
  '/src/components/IntelligenceConsole.tsx',
  '/src/components/BriefingView.tsx',
  '/src/components/DecisionMemory.tsx',
  '/src/main.tsx',
];

function log(msg) { console.log(`[smoke] ${msg}`); }
function err(msg) { console.error(`[smoke] ERROR: ${msg}`); }

function httpGet(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, { timeout: REQUEST_TIMEOUT_MS }, (res) => {
      let body = '';
      res.on('data', (chunk) => { body += chunk; });
      res.on('end', () => resolve({ status: res.statusCode, body }));
    });
    req.on('error', reject);
    req.on('timeout', () => {
      req.destroy();
      reject(new Error(`Request to ${url} timed out`));
    });
  });
}

async function waitForServerReady(maxWaitMs) {
  const deadline = Date.now() + maxWaitMs;
  while (Date.now() < deadline) {
    try {
      const res = await httpGet(`${DEV_HOST}/`);
      if (res.status === 200) return true;
    } catch { /* not ready yet */ }
    await new Promise((r) => setTimeout(r, 500));
  }
  return false;
}

function findCannotFindModule(body) {
  // Match Vite's dep optimizer "Cannot find module" error in the response body
  if (typeof body !== 'string') return null;
  const m = body.match(/Cannot find module[^\n]+/i);
  return m ? m[0].trim() : null;
}

async function main() {
  log('Starting fresh Vite dev server...');

  // Kill anything already on the port first (matches pnpm run dev behavior)
  try {
    const { execSync } = require('node:child_process');
    execSync(`node "${path.join(__dirname, 'kill-port.cjs')}" ${PORT}`, { stdio: 'ignore' });
  } catch { /* port may already be free */ }

  // Clean the Vite deps cache so we do a true cold start
  const depsCache = path.join(__dirname, '..', 'node_modules', '.vite', 'deps');
  if (fs.existsSync(depsCache)) {
    log('Clearing node_modules/.vite/deps cache...');
    fs.rmSync(depsCache, { recursive: true, force: true });
  }

  const viteBin = path.join(__dirname, '..', 'node_modules', 'vite', 'bin', 'vite.js');
  const child = spawn('node', [viteBin], {
    cwd: path.join(__dirname, '..'),
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, FORCE_COLOR: '0' },
  });

  let output = '';
  child.stdout.on('data', (d) => { output += d.toString(); });
  child.stderr.on('data', (d) => { output += d.toString(); });

  // Ensure cleanup no matter what
  const cleanup = () => {
    try { child.kill('SIGTERM'); } catch { /* ignore */ }
  };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });

  log('Waiting for server to be ready...');
  const ready = await waitForServerReady(STARTUP_TIMEOUT_MS);
  if (!ready) {
    err(`Server did not become ready within ${STARTUP_TIMEOUT_MS}ms`);
    err('Server output:');
    console.error(output);
    cleanup();
    process.exit(1);
  }

  log('Server ready. Warming up dep optimizer...');
  // Fetch index.html to trigger Vite's dependency pre-bundling before we
  // request individual modules — prevents timeout on heavy entry points.
  try { await httpGet(`${DEV_HOST}/?t=${Date.now()}`); } catch { /* ok */ }
  await new Promise((r) => setTimeout(r, 2000));

  log('Requesting critical routes...');
  const failures = [];

  for (const route of CRITICAL_ROUTES) {
    try {
      const res = await httpGet(`${DEV_HOST}${route}`);
      if (res.status !== 200) {
        failures.push({ route, reason: `HTTP ${res.status}` });
        continue;
      }
      // Also scan the body for "Cannot find module" error overlays
      const moduleErr = findCannotFindModule(res.body);
      if (moduleErr) {
        failures.push({ route, reason: moduleErr });
        continue;
      }
      log(`  OK ${route}`);
    } catch (e) {
      failures.push({ route, reason: e.message });
    }
  }

  // Also scan server output for any "Cannot find module" errors surfaced
  // by Vite's dep optimizer (which runs asynchronously on first request)
  await new Promise((r) => setTimeout(r, 1500));
  const outputErr = findCannotFindModule(output);
  if (outputErr) {
    failures.push({ route: '(server stderr)', reason: outputErr });
  }

  cleanup();
  await new Promise((r) => setTimeout(r, 500));

  if (failures.length > 0) {
    err('COLD-START SMOKE TEST FAILED');
    for (const f of failures) {
      err(`  ${f.route}: ${f.reason}`);
    }
    err('');
    err('This usually means:');
    err('  1. A dependency update left stale paths in Vite dep optimizer');
    err('  2. A running fourda.exe has old paths cached in memory');
    err('  3. An import points to a nonexistent module');
    err('');
    err('Fix: kill running fourda.exe + run `pnpm install --frozen-lockfile`');
    process.exit(1);
  }

  log(`COLD-START SMOKE TEST PASSED — ${CRITICAL_ROUTES.length} routes verified`);
  process.exit(0);
}

main().catch((e) => {
  err(`Uncaught: ${e.message}`);
  process.exit(1);
});
