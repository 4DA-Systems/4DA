// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// Error reporter — local-first, zero third-party telemetry.
//
// In development: logs to the console.
// In production: forwards to the LOCAL rotating log on the user's machine via
// the `log_frontend_error` Tauri command. Nothing is transmitted off-device;
// the backend scrubs usernames and secret-shaped tokens before writing, and
// the user can bundle these logs on demand via Settings → Privacy →
// "Export diagnostics". This replaced the removed Sentry integration.
const isDev = import.meta.env.DEV;

function messageOf(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? `${error.name}: ${error.message}`;
  }
  if (typeof error === 'string') return error;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function reportError(context: string, error: unknown): void {
  if (isDev) {
    console.error(`[4DA] ${context}:`, error);
    return;
  }
  // Production: persist to the local on-device log, fire-and-forget. The
  // dynamic import keeps this safe in non-Tauri environments (tests, browser),
  // where the call simply rejects and is swallowed.
  void import('./commands')
    .then(({ cmd }) => cmd('log_frontend_error', { context, message: messageOf(error) }))
    .catch(() => {
      /* non-Tauri env or command unavailable — nothing to do, stays local */
    });
}
