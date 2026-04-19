// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Translates raw Rust backend error strings into user-friendly messages.
 *
 * Tauri invoke errors often expose internal Rust error chains like
 * "Failed to fetch: reqwest::Error { kind: Request, ... }" which are
 * meaningless (and alarming) to users. This utility maps known patterns
 * to concise, actionable messages.
 */

interface ErrorPattern {
  /** Regex or string to test against the raw error */
  test: RegExp;
  /** i18n key for the user-friendly message (namespace: errors) */
  key: string;
  /** Fallback English message (used if i18n not loaded) */
  fallback: string;
}

const ERROR_PATTERNS: ErrorPattern[] = [
  { test: /failed to fetch|network error|reqwest/i, key: 'errors:errorMsg.network', fallback: 'Network request failed. Check your internet connection.' },
  { test: /timeout|timed?\s*out|deadline.*exceeded/i, key: 'errors:errorMsg.timeout', fallback: 'Request timed out. Try again in a moment.' },
  { test: /api\s*key|unauthorized|401|403|invalid.*key/i, key: 'errors:errorMsg.auth', fallback: 'Authentication failed. Check your API key in Settings.' },
  { test: /rate\s*limit|429|too many requests/i, key: 'errors:errorMsg.rateLimit', fallback: 'Rate limit reached. Please wait a moment and try again.' },
  { test: /ollama.*not.*running|ollama.*connect|(?=.*connection refused)(?=.*ollama)|(?=.*ECONNREFUSED)(?=.*ollama)/i, key: 'errors:errorMsg.ollamaNotRunning', fallback: 'Ollama is not running. Start Ollama or set an API key in Settings.' },
  { test: /ollama|embedding.*fail|model.*not.*found/i, key: 'errors:errorMsg.embedding', fallback: 'Embedding service unavailable. Check that Ollama is running.' },
  { test: /connection refused|ECONNREFUSED/i, key: 'errors:errorMsg.connectionRefused', fallback: 'Network request failed. Check your internet connection.' },
  { test: /sqlite|database.*locked|disk\s*i\/o|database.*error/i, key: 'errors:errorMsg.database', fallback: 'Database error. Try restarting the app.' },
  { test: /permission\s*denied|EACCES/i, key: 'errors:errorMsg.permission', fallback: 'Permission denied. Check file permissions.' },
  { test: /serde|deserialize|json.*error|parse.*error/i, key: 'errors:errorMsg.parse', fallback: 'Data format error. Try again or restart the app.' },
  { test: /already running|already in progress/i, key: 'errors:errorMsg.alreadyRunning', fallback: 'Already in progress. Please wait for it to complete.' },
  { test: /no such file|file not found|ENOENT|path.*not.*exist/i, key: 'errors:errorMsg.fileNotFound', fallback: 'File not found. It may have been moved or deleted.' },
  { test: /__TAURI__|invoke.*error|ipc.*error/i, key: 'errors:errorMsg.ipc', fallback: 'App communication error. Please restart the app.' },
  { test: /git.*error|not a git repository/i, key: 'errors:errorMsg.git', fallback: 'Git operation failed. Check the repository path.' },
];

const FALLBACK_KEY = 'errors:errorMsg.fallback';
const FALLBACK_MESSAGE = 'Something went wrong. Please try again.';

// Access i18n instance for translating error messages.
// Uses the already-initialized singleton from ../i18n — no circular dep
// because this module only reads from it at call time, never at import time.
function t(key: string, fallback: string): string {
  try {
    // i18n singleton is already initialized by the time errors are shown
    const i18n = (window as unknown as Record<string, unknown>).__4da_i18n as
      | { t: (k: string, opts?: Record<string, unknown>) => string; isInitialized: boolean }
      | undefined;
    if (i18n?.isInitialized) {
      return i18n.t(key, { defaultValue: fallback }) || fallback;
    }
  } catch {
    // i18n not available — use fallback
  }
  return fallback;
}

/**
 * Structured error from the 4DA backend (matches Rust UserError).
 * When the backend returns errors, they now come as structured objects
 * with error codes, titles, details, and remediation steps.
 */
export interface StructuredError {
  code: string;
  title: string;
  detail: string;
  remediation: string[];
  severity: 'info' | 'warning' | 'error' | 'critical';
}

/**
 * Check if an error is a structured error from the backend.
 */
export function isStructuredError(error: unknown): error is StructuredError {
  if (typeof error !== 'object' || error === null) return false;
  const obj = error as Record<string, unknown>;
  return typeof obj.code === 'string' && typeof obj.title === 'string' && typeof obj.detail === 'string';
}

/**
 * Parse a structured error from various error shapes.
 * The backend may return the error directly or wrapped in a string.
 */
export function parseStructuredError(error: unknown): StructuredError | null {
  // Direct object
  if (isStructuredError(error)) return error;

  // String that might be JSON
  const raw = typeof error === 'string' ? error : error instanceof Error ? error.message : String(error ?? '');
  try {
    const parsed = JSON.parse(raw);
    if (isStructuredError(parsed)) return parsed;
  } catch {
    // Not JSON — fall through to pattern matching
  }
  return null;
}

/**
 * Translate a raw backend error into a user-friendly message.
 *
 * First tries to parse as a structured error (from the new error framework).
 * Falls back to regex pattern matching for legacy/unstructured errors.
 *
 * @param error - The raw error from a Tauri invoke catch block (can be any type)
 * @returns A user-friendly error string
 */
export function translateError(error: unknown): string {
  // Try structured error first (new backend error framework)
  const structured = parseStructuredError(error);
  if (structured) {
    // Try to match the structured error against known patterns for i18n
    const combined = `${structured.title} ${structured.detail}`;
    for (const pattern of ERROR_PATTERNS) {
      if (pattern.test.test(combined)) {
        return t(pattern.key, pattern.fallback);
      }
    }
    // No pattern match — use raw structured error with remediation
    if (structured.remediation.length > 0) {
      return `${structured.title}: ${structured.detail} ${structured.remediation[0]}`;
    }
    return `${structured.title}: ${structured.detail}`;
  }

  const raw = error instanceof Error ? error.message : String(error ?? '');

  if (!raw || raw === 'undefined' || raw === 'null') {
    return FALLBACK_MESSAGE;
  }

  for (const pattern of ERROR_PATTERNS) {
    if (pattern.test.test(raw)) {
      return t(pattern.key, pattern.fallback);
    }
  }

  return t(FALLBACK_KEY, FALLBACK_MESSAGE);
}

/**
 * Like translateError but preserves the raw message for console logging
 * while returning the user-friendly version.
 *
 * Usage:
 *   const [userMsg, rawMsg] = translateErrorWithRaw(e);
 *   console.error(rawMsg);
 *   setError(userMsg);
 */
export function translateErrorWithRaw(error: unknown): [userFriendly: string, raw: string] {
  const raw = error instanceof Error ? error.message : String(error ?? '');
  return [translateError(error), raw];
}
