// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Sentry error tracking — privacy-first, opt-in only.
 *
 * This module sets up Sentry for 4DA with aggressive privacy defaults:
 *   • OPT-IN — the user must explicitly enable crash reports in Settings.
 *   • No IP address captured (`sendDefaultPii: false`).
 *   • File paths stripped from error context before send (they contain
 *     the user's Windows/macOS/Linux username).
 *   • API keys, license keys, and OAuth tokens stripped from error context
 *     by pattern match.
 *   • No session replay, no user identification, no breadcrumb capture
 *     for URL navigation (desktop app, no URL to leak).
 *   • Respects DO_NOT_TRACK environment conventions.
 *
 * Set the DSN via Vite env var `VITE_SENTRY_DSN` in a `.env.local` file
 * (gitignored). If the DSN is missing, Sentry initialization is silently
 * skipped — the app works identically without it.
 *
 * Privacy policy disclosure: `docs/legal/PRIVACY-POLICY.md` section on
 * opt-in telemetry documents exactly what is sent.
 */

import * as Sentry from '@sentry/react';

/** Pattern fragments that, if found in error context, get scrubbed. */
const SENSITIVE_PATTERNS = [
  /api[_-]?key/i,
  /license[_-]?key/i,
  /access[_-]?token/i,
  /refresh[_-]?token/i,
  /secret/i,
  /password/i,
  /bearer\s+[a-z0-9._-]+/i,
  /4da-[a-z0-9]+/i, // self-signed 4DA license format
  /BE[0-9A-F]{4}-[0-9A-F]{6}-[0-9A-F]+/i, // Keygen license key format
] as const;

const USERNAME_PATH_PATTERN = /([A-Z]:\\Users\\|\/Users\/|\/home\/)[^\\/]+/g;

/**
 * Strip file paths containing usernames and known secret patterns from a string.
 * Used by beforeSend to sanitize error messages before they leave the device.
 */
function sanitize(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  let out = value;

  // Replace `C:\Users\someone\` with `C:\Users\<user>\` (Windows)
  // Replace `/Users/someone/` with `/Users/<user>/` (macOS)
  // Replace `/home/someone/` with `/home/<user>/` (Linux)
  out = out.replace(USERNAME_PATH_PATTERN, (m) => {
    const base = m.replace(/[^\\/]+$/, '');
    return `${base}<user>`;
  });

  // Redact anything matching a secret pattern
  for (const pattern of SENSITIVE_PATTERNS) {
    if (pattern.test(out)) {
      out = out.replace(pattern, '<redacted>');
    }
  }

  return out;
}

/** Recursively sanitize all string values in a JSON-serializable object. */
function sanitizeObject<T>(obj: T): T {
  if (obj === null || obj === undefined) return obj;
  if (typeof obj === 'string') return sanitize(obj) as T;
  if (Array.isArray(obj)) return obj.map(sanitizeObject) as T;
  if (typeof obj === 'object') {
    const result: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      result[k] = sanitizeObject(v);
    }
    return result as T;
  }
  return obj;
}

let initialized = false;

/**
 * Initialize Sentry error tracking if the user has opted in.
 *
 * Call this AFTER reading user settings — before the main React tree mounts.
 *
 * @param optedIn - Whether the user has checked "Send anonymous crash reports"
 * @returns true if Sentry was initialized, false otherwise
 */
export function initSentry(optedIn: boolean): boolean {
  // Already initialized in this session — don't re-initialize
  if (initialized) return true;

  // User hasn't opted in — don't initialize
  if (!optedIn) return false;

  // Respect DO_NOT_TRACK conventions
  if (
    typeof navigator !== 'undefined' &&
    (navigator as Navigator & { doNotTrack?: string }).doNotTrack === '1'
  ) {
    return false;
  }

  // DSN is provided via Vite env var at build time
  // `VITE_SENTRY_DSN` will be embedded into the built JS.
  // If unset, Sentry is silently skipped.
  const dsn = import.meta.env.VITE_SENTRY_DSN;
  if (!dsn || typeof dsn !== 'string') return false;

  Sentry.init({
    dsn,
    environment: import.meta.env.PROD ? 'production' : 'development',

    // Release version — matches __APP_VERSION__ injected by Vite
    release: typeof __APP_VERSION__ !== 'undefined' ? `4da@${__APP_VERSION__}` : undefined,

    // Privacy: no IP, no user identification
    sendDefaultPii: false,

    // Don't auto-capture console logs — too noisy and may contain secrets
    integrations: [],

    // Sample 100% of errors but no performance traces (too much data)
    tracesSampleRate: 0,

    // Sanitize everything before it leaves the device
    beforeSend(event) {
      // Strip IP from user context
      if (event.user) {
        delete event.user.ip_address;
        delete event.user.email;
        delete event.user.username;
      }

      // Sanitize error messages and values
      if (event.message) {
        event.message = sanitize(event.message) as string;
      }
      if (event.exception?.values) {
        for (const value of event.exception.values) {
          if (value.value) value.value = sanitize(value.value) as string;
          if (value.stacktrace?.frames) {
            for (const frame of value.stacktrace.frames) {
              if (frame.filename) frame.filename = sanitize(frame.filename) as string;
              if (frame.abs_path) frame.abs_path = sanitize(frame.abs_path) as string;
            }
          }
        }
      }

      // Sanitize breadcrumbs (unlikely to have any, but defensive)
      if (event.breadcrumbs) {
        event.breadcrumbs = sanitizeObject(event.breadcrumbs);
      }

      // Sanitize extra context
      if (event.extra) {
        event.extra = sanitizeObject(event.extra);
      }
      if (event.contexts) {
        event.contexts = sanitizeObject(event.contexts);
      }

      return event;
    },
  });

  initialized = true;
  return true;
}

/**
 * Manually report an error to Sentry (if initialized).
 *
 * Errors from ErrorBoundary and uncaught rejections are captured
 * automatically — this is for explicit "something unexpected happened"
 * reporting in catch blocks.
 */
export function reportError(error: unknown, context?: Record<string, unknown>): void {
  if (!initialized) return;
  Sentry.captureException(error, {
    extra: context ? sanitizeObject(context) : undefined,
  });
}

/** Is Sentry currently initialized and actively reporting? */
export function isSentryActive(): boolean {
  return initialized;
}
