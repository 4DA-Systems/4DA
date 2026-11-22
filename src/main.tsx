// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import React from 'react';
import ReactDOM from 'react-dom/client';
import { BootScreen } from './boot/BootScreen';
import { widenResourceCount } from './boot/boot-clock';
import type { InstantBriefingSnapshot } from './store/types';

const STARTUP_SNAPSHOT_TIMEOUT_MS = 750;

async function withStartupTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T | null> {
  return Promise.race([
    promise,
    new Promise<null>((resolve) => setTimeout(() => resolve(null), timeoutMs)),
  ]);
}

async function signalFrontendReady(): Promise<void> {
  await Promise.allSettled([
    import('@tauri-apps/api/event').then(({ emit }) => emit('frontend-ready')),
    import('./lib/commands').then(({ cmd }) => cmd('mark_frontend_ready')),
  ]);
}

async function hydrateStartupSnapshot(): Promise<void> {
  try {
    // Use the typed `cmd` wrapper so the IPC validator is satisfied and we
    // get full type-checking on the snapshot shape. The dynamic import keeps
    // this safe in non-Tauri environments (tests, browser).
    const { cmd } = await import('./lib/commands');
    const raw = await withStartupTimeout(
      cmd('get_briefing_snapshot').catch(() => null),
      STARTUP_SNAPSHOT_TIMEOUT_MS,
    );

    if (!raw) return;

    // Convert snake_case (Rust contract) to camelCase (TypeScript convention)
    // exactly once, here, so the rest of the frontend stays clean.
    const snapshot: InstantBriefingSnapshot = {
      version: raw.version,
      generatedAtUnix: raw.generated_at_unix,
      generatedAtDisplay: raw.generated_at_display,
      title: raw.briefing.title,
      items: raw.briefing.items.map(i => ({
        title: i.title,
        sourceType: i.source_type,
        score: i.score,
        signalType: i.signal_type ?? null,
        url: i.url ?? null,
        itemId: i.item_id ?? null,
        signalPriority: i.signal_priority ?? null,
        description: i.description ?? null,
        matchedDeps: i.matched_deps ?? [],
      })),
      totalRelevant: raw.briefing.total_relevant,
      synthesis: raw.briefing.synthesis ?? null,
      wisdomSynthesis: raw.briefing.wisdom_synthesis ?? null,
    };
    (window as Window & { __4DA_INSTANT_SNAPSHOT__?: InstantBriefingSnapshot | null }).__4DA_INSTANT_SNAPSHOT__ = snapshot;

    // App imports the store before any runtime startup code can execute, so
    // the window stash is only a compatibility bridge. Updating the live store
    // is the path that makes the snapshot visible without blocking first paint.
    const { useAppStore } = await import('./store');
    useAppStore.getState().setInstantSnapshot(snapshot);
  } catch {
    // Non-Tauri environment OR snapshot fetch failed — silently fall through
    // to normal first-run rendering. The frontend already handles the no-data
    // case correctly via its existing empty state.
  }
}

async function initNativeThemeChrome(): Promise<void> {
  try {
    const { initTheme } = await import('./lib/theme');
    initTheme();
  } catch {
    // Non-Tauri / test environment — webview-only theming.
  }
}

// The boot screen: 4DA's mark in motion while the app's code loads, then (if
// the wait runs long) what is happening and a way out. See src/boot.
function BootShell() {
  return <BootScreen phase="loading" devBuild={import.meta.env.DEV} />;
}

function BootError() {
  return <BootScreen phase="failed" devBuild={import.meta.env.DEV} />;
}

// ============================================================================
// Service-worker guard — the 4DA app frontend must NEVER be controlled by a
// service worker. The embedded Signal Terminal registers one on ITS OWN origin
// for offline support. Historically the terminal (prod) and the Vite dev server
// shared localhost:4444, so the terminal's SW could land on the app's origin,
// hijack the shell, and serve its cached "Signal Terminal Offline" page instead
// of the real UI whenever the dev server was momentarily unreachable. We now
// keep the ports disjoint, but we also defensively unregister any SW controlling
// this origin on every boot so a stale registration can never black-hole the app.
// ============================================================================
if ('serviceWorker' in navigator) {
  void navigator.serviceWorker
    .getRegistrations()
    .then((regs) => regs.forEach((r) => void r.unregister()))
    .catch(() => {
      /* non-fatal — SW API unavailable or blocked */
    });
}

// ============================================================================
// Error handling: 4DA has NO third-party crash reporting and NO telemetry.
// Production frontend errors are forwarded to the LOCAL rotating log via
// `log_frontend_error` (see src/lib/error-reporter.ts) and never leave the
// machine. The user can bundle them on demand via Settings → Privacy →
// "Export diagnostics". This is the local-first replacement for Sentry.
// ============================================================================

// ============================================================================
// Sovereign Cold Boot — non-blocking briefing snapshot hydration
// ============================================================================
//
// Read the pre-baked briefing snapshot from disk via the privileged Tauri
// command without blocking React's first render. The App import constructs the
// store before startup code runs, so the reliable path is to update the live
// store as soon as the bounded IPC call returns.
//
// Critical path: keep this short. We deliberately do NOT await any other
// I/O before the React render. The snapshot is optional cold-boot acceleration;
// a slow or wedged IPC path must not leave the root div empty.
//
// All errors are silently swallowed: a missing/corrupt/expired snapshot just
// means the React tree will show its normal first-run state. The user is
// never shown an error from this path.
// ============================================================================
const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);

// A development build's boot screen shows the interface's files arriving.
if (import.meta.env.DEV) widenResourceCount();

root.render(
  <React.StrictMode>
    <BootShell />
  </React.StrictMode>,
);

// Signal Rust after the boot shell has been submitted for rendering, before
// optional startup data fetches or the full app import. Emit the Tauri event
// and also call the typed command path; both are background-only and neither
// can block first paint.
void signalFrontendReady();
void hydrateStartupSnapshot();
void initNativeThemeChrome();

void import('./App')
  .then(({ default: App }) => {
    root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  })
  .catch((error: unknown) => {
    // The boot screen offers a retry; the cause goes to the console (and, in
    // an installed build, the local log via the error reporter once loaded).
    console.error('[boot] the app could not be loaded:', error);
    root.render(
      <React.StrictMode>
        <BootError />
      </React.StrictMode>,
    );
  });

// Dev-only: expose briefing trigger for testing (call __testBriefing() in devtools console)
if (import.meta.env.DEV) {
  void import('./lib/commands').then(({ cmd }) => {
    (window as unknown as Record<string, unknown>).__testBriefing = () =>
      cmd('trigger_morning_briefing').then(console.log, console.error);
  }).catch(() => {});
}
