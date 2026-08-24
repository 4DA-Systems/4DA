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

// Port to bind. Local runs default to 4444 (matching vite.config.ts) so the
// busy-port guard below detects a live dev server and skips instead of
// disrupting it. CI sets VITE_SMOKE_PORT to a dedicated port (validate.yml:
// 4445) so the cold-start NEVER contends with — and never needs to kill —
// anything already running on the self-hosted dev box. Issue #501: the CI
// flavor of this step used `taskkill /F /IM fourda.exe`, force-killing the
// operator's live app (silent exit code 1) on every frontend-touching run,
// while freeing nothing (fourda.exe is the dev server's client, not the
// listener on 4444).
const PORT = Number(process.env.VITE_SMOKE_PORT ?? 4444);
const DEV_HOST = `http://127.0.0.1:${PORT}`;
const STARTUP_TIMEOUT_MS = Number(process.env.VITE_SMOKE_STARTUP_TIMEOUT_MS ?? 90000);
const REQUEST_TIMEOUT_MS = Number(process.env.VITE_SMOKE_REQUEST_TIMEOUT_MS ?? 60000);
const ROUTE_RETRY_DELAY_MS = Number(process.env.VITE_SMOKE_ROUTE_RETRY_DELAY_MS ?? 3000);

// Critical modules that MUST resolve on a cold start.
// main.tsx is intentionally before App.tsx: the startup path now paints a
// lightweight BootShell before dynamically importing the full app graph, so the
// smoke test should prove that entry independently before App.tsx forces the
// heavy dependency optimizer path.
const CRITICAL_ROUTES = [
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
  '/src/App.tsx',
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

/** Anything LISTENING on the port — the same signal kill-port kills by. */
function portIsBusy(port) {
  const { execSync } = require('node:child_process');
  try {
    if (process.platform === 'win32') {
      const out = execSync(`netstat -ano | findstr :${port} | findstr LISTENING`, {
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
      });
      return out.trim().length > 0;
    }
    const out = execSync(`lsof -ti :${port}`, {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    return out.trim().length > 0;
  } catch {
    return false; // findstr/lsof exit non-zero when nothing matches
  }
}

async function main() {
  // A live dev server owns port 4444 AND the node_modules/.vite/deps cache.
  // Killing it and wiping that cache from here crashes the running fourda.exe
  // ("Cannot find module vite@..." — the exact bug this script exists to catch),
  // which is how fleet validation cycles kept murdering the operator's dev app
  // (observed live 2026-07-17: three kills in 15 minutes). A cold-start check
  // is impossible without disrupting the running instance, so skip honestly —
  // CI and any dev-server-down run keep full coverage.
  //
  // Guard v2 (observed live 2026-07-18): the original 1.5s HTTP-200 probe
  // missed under load — 18 queued CI jobs starved the running vite past the
  // timeout, the guard read "no dev server", and kill-port murdered it anyway.
  // A LISTENING socket is the same signal kill-port kills by and has no
  // timing sensitivity: if ANYTHING listens on the port, the cold-start
  // cannot run safely — skip. (A dead-but-listening zombie would fail this
  // run visibly at bind time rather than silently killing a live instance.)
  if (portIsBusy(PORT)) {
    log(`SKIPPED: something is listening on port ${PORT} (a dev server is likely running).`);
    log('Cold-start smoke cannot run without killing it (port + .vite/deps cache are shared).');
    log('Full coverage still runs in CI and whenever no dev server is up.');
    process.exit(0);
  }

  log('Starting fresh Vite dev server...');

  // Clean the Vite deps cache so we do a true cold start
  const viteCacheRoot = path.join(__dirname, '..', 'node_modules', '.vite');
  const viteCacheEntries = fs.existsSync(viteCacheRoot)
    ? fs.readdirSync(viteCacheRoot).filter((entry) => entry === 'deps' || entry.startsWith('deps_temp_'))
    : [];
  if (viteCacheEntries.length > 0) {
    log(`Clearing Vite optimizer cache (${viteCacheEntries.length} director${viteCacheEntries.length === 1 ? 'y' : 'ies'})...`);
    for (const entry of viteCacheEntries) {
      fs.rmSync(path.join(viteCacheRoot, entry), { recursive: true, force: true });
    }
  }

  const viteBin = path.join(__dirname, '..', 'node_modules', 'vite', 'bin', 'vite.js');
  // --port pins the resolved PORT (vite.config.ts says 4444; VITE_SMOKE_PORT
  // may override). --strictPort makes vite FAIL at bind time instead of
  // silently hopping to the next free port — without it, a port grabbed
  // between the busy-guard above and this spawn would leave the probe loop
  // polling a port vite never bound.
  const child = spawn('node', [viteBin, '--port', String(PORT), '--strictPort'], {
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
    const MAX_RETRIES = 3;
    let lastErr = null;
    let ok = false;
    for (let attempt = 0; attempt < MAX_RETRIES && !ok; attempt++) {
      try {
        if (attempt > 0) await new Promise((r) => setTimeout(r, ROUTE_RETRY_DELAY_MS));
        const res = await httpGet(`${DEV_HOST}${route}`);
        if (res.status !== 200) {
          lastErr = `HTTP ${res.status}`;
          continue;
        }
        const moduleErr = findCannotFindModule(res.body);
        if (moduleErr) {
          lastErr = moduleErr;
          continue;
        }
        log(`  OK ${route}`);
        ok = true;
      } catch (e) {
        lastErr = e.message;
      }
    }
    if (!ok) failures.push({ route, reason: lastErr });
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
