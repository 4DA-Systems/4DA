// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import React from 'react';
import ReactDOM from 'react-dom/client';
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import App from './App';
import type { InstantBriefingSnapshot } from './store/types';
import { initSentry } from './lib/sentry-init';

// ============================================================================
// Sentry — opt-in anonymous crash reporting (privacy-first)
// ============================================================================
// Initialized BEFORE React mounts so uncaught errors during first-paint
// are captured. Reads the user's opt-in flag from settings via a privileged
// Tauri command. If settings can't be read (first-run, non-Tauri env), opt-in
// is treated as false and Sentry stays dormant.
//
// See src/lib/sentry-init.ts for the full privacy-stripping rules.
// ============================================================================
try {
  const { cmd } = await import('./lib/commands');
  const privacy = await cmd('get_privacy_config');
  initSentry(Boolean(privacy?.crash_reporting_opt_in));
} catch {
  // Non-Tauri environment or settings unreadable — don't initialize Sentry
}

// ============================================================================
// Sovereign Cold Boot — instant briefing first paint
// ============================================================================
//
// Read the pre-baked briefing snapshot from disk via the privileged Tauri
// command BEFORE the React tree mounts. The result lands on a window global
// (window.__4DA_INSTANT_SNAPSHOT__) which the briefing slice consumes on its
// first construction. This is the difference between "the user opens 4DA
// and waits 5+ seconds for content" and "the user opens 4DA and yesterday's
// briefing is already on screen with a small refreshing indicator".
//
// Critical path: keep this short. We deliberately do NOT await any other
// I/O before the React render — only the snapshot fetch. The Tauri command
// reads a single small JSON file and returns synchronously from the user's
// perspective (the round-trip is sub-millisecond once the IPC channel exists).
//
// All errors are silently swallowed: a missing/corrupt/expired snapshot just
// means the React tree will show its normal first-run state. The user is
// never shown an error from this path.
// ============================================================================
try {
  // Use the typed `cmd` wrapper so the IPC validator is satisfied and we
  // get full type-checking on the snapshot shape. The dynamic import keeps
  // this safe in non-Tauri environments (tests, browser).
  const { cmd } = await import('./lib/commands');
  const raw = await cmd('get_briefing_snapshot');

  if (raw) {
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
  }
} catch {
  // Non-Tauri environment OR snapshot fetch failed — silently fall through
  // to normal first-run rendering. The frontend already handles the no-data
  // case correctly via its existing empty state.
}

// Signal Rust that the frontend JS loaded BEFORE React mounts.
// This fires ~300-500ms earlier than SplashScreen's useEffect,
// allowing the hidden window to show immediately with the splash
// animation instead of waiting for the full component tree.
try {
  const { emit } = await import('@tauri-apps/api/event');
  const result = emit('frontend-ready');
  if (result && typeof result.catch === 'function') {
    result.catch(() => { /* ignore in browser mode */ });
  }
} catch {
  // Non-Tauri environment (tests, browser) — silently ignore
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
