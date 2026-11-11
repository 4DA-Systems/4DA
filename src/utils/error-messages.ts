// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Turns a rejected backend call into text a person can read.
 *
 * A Tauri invoke rejects with whatever the command's error type serialises
 * to. A `FourDaError` arrives as an OBJECT `{ code, title, detail,
 * remediation, severity }` (src-tauri/src/error.rs), a `Result<_, String>`
 * command as a plain string, and the webview itself throws `Error`s.
 * `String(e)` prints "[object Object]" for the first, so nothing outside this
 * module stringifies a rejection:
 *
 * - `readableError`  what to show: the backend's own words.
 * - `translateError` what to show when only the category matters: known
 *   patterns map to short localized messages, anything else stays generic.
 * - `rawErrorText`   what to match against, or to log.
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
function t(key: string, fallback: string, opts?: Record<string, unknown>): string {
  try {
    // i18n singleton is already initialized by the time errors are shown
    const i18n = (window as unknown as Record<string, unknown>).__4da_i18n as
      | { t: (k: string, opts?: Record<string, unknown>) => string; isInitialized: boolean; language?: string }
      | undefined;
    if (i18n?.isInitialized) {
      return i18n.t(key, { ...opts, defaultValue: fallback }) || fallback;
    }
  } catch {
    // i18n not available — use fallback
  }
  return fallback;
}

/** A `FourDaError` as the frontend receives it (Rust `UserError`, error.rs). */
interface StructuredError {
  code: string;
  title: string;
  detail: string;
  remediation?: string[];
  severity?: 'info' | 'warning' | 'error' | 'critical';
}

/**
 * Codes whose `detail` IS the backend's original sentence: the catch-all arm
 * (`Internal`, which every `Err("...".into())` becomes) and `Validation`
 * (error.rs `to_user_error`). Their title ("Unexpected error", "Invalid
 * input") and remediation ("Try the operation again.") are boilerplate.
 */
const PASSTHROUGH_CODES = new Set(['E9999', 'E6001']);

function isStructuredError(error: unknown): error is StructuredError {
  if (typeof error !== 'object' || error === null) return false;
  const obj = error as Record<string, unknown>;
  return typeof obj.code === 'string' && typeof obj.title === 'string' && typeof obj.detail === 'string';
}

/** The structured error in `error`: the object itself, or one serialised into a string. */
function parseStructuredError(error: unknown): StructuredError | null {
  if (isStructuredError(error)) return error;
  const raw = typeof error === 'string' ? error : error instanceof Error ? error.message : '';
  if (!raw.trimStart().startsWith('{')) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (isStructuredError(parsed)) return parsed;
  } catch {
    // Not JSON: an ordinary message.
  }
  return null;
}

/** A structured error as one line: the backend's sentence, and its first fix when asked. */
function structuredText(error: StructuredError, withRemediation: boolean): string {
  const detail = error.detail.trim();
  if (detail && PASSTHROUGH_CODES.has(error.code)) return detail;
  const head = detail ? `${error.title}: ${detail}` : error.title;
  const fix = withRemediation && Array.isArray(error.remediation) ? error.remediation[0] : undefined;
  return fix ? `${head} ${fix}` : head;
}

/**
 * The backend's own words for a rejection, whatever shape it arrived in:
 * structured object, `Error`, string, or anything else. For matching (the
 * Signal gate, the budget token, a known phrase) and for logs. Empty when the
 * rejection carries no text. Show `readableError` to users instead.
 */
export function rawErrorText(error: unknown): string {
  const structured = parseStructuredError(error);
  if (structured) return structuredText(structured, false);
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null) {
    const { message, detail } = error as { message?: unknown; detail?: unknown };
    if (typeof message === 'string') return message;
    return typeof detail === 'string' ? detail : '';
  }
  return error === null || error === undefined ? '' : String(error);
}

/**
 * Any rejection as text a person can read. Never "[object Object]", never a
 * machine token.
 *
 * - A spent AI budget and a Signal gate get their own localized sentence.
 * - A structured error with a specific code reads as `translateError` renders it.
 * - Everything else is the backend's own sentence, unchanged: the `detail` of
 *   a pass-through structured error, an `Error`'s message, a string.
 * - A rejection with no text gives `fallback`, or the generic message.
 */
export function readableError(error: unknown, fallback?: string): string {
  const special = budgetRefusalMessage(error) ?? signalGateMessage(error);
  if (special) return special;
  const structured = parseStructuredError(error);
  if (structured && !PASSTHROUGH_CODES.has(structured.code)) return translateError(structured);
  return rawErrorText(error).trim() || fallback || t(FALLBACK_KEY, FALLBACK_MESSAGE);
}

/**
 * A rejection as a short, categorized message: known patterns (network,
 * timeout, auth, ...) map to localized copy; an unrecognised structured error
 * reads as the backend wrote it; anything else stays generic so backend
 * internals never reach a banner.
 *
 * @param error - The raw error from a Tauri invoke catch block (can be any type)
 */
export function translateError(error: unknown): string {
  // Neither is a fault to retry: a spent daily AI budget says when it resets,
  // a tier gate says it is one.
  const special = budgetRefusalMessage(error) ?? signalGateMessage(error);
  if (special) return special;

  const raw = rawErrorText(error);
  if (raw && raw !== 'undefined' && raw !== 'null') {
    for (const pattern of ERROR_PATTERNS) {
      if (pattern.test.test(raw)) return t(pattern.key, pattern.fallback);
    }
    const structured = parseStructuredError(error);
    if (structured) return structuredText(structured, true);
  }
  return t(FALLBACK_KEY, FALLBACK_MESSAGE);
}

const SIGNAL_GATE = /requires 4DA Signal/i;

/**
 * Detect a Signal-tier paywall rejection.
 *
 * `require_signal_feature` (src-tauri/src/settings/license/gating.rs) is the
 * sole producer: `"<Feature> requires 4DA Signal — ..."`. It sets no code of
 * its own. Commands returning `Result<_, String>` reject with that sentence;
 * commands returning `FourDaError` reject with the catch-all object
 * `{ code: "E9999", title: "Unexpected error", detail: "<sentence>" }`, so the
 * sentence is matched in whichever field carries it. A gate that was only
 * looked for in a string turned the Knowledge Gaps upgrade card into
 * "Couldn't check... Retry" for every free user.
 *
 * A paywall is NOT a fault: a gated surface that catches this renders the
 * upgrade path, never a red error banner (antibody AB-011,
 * display-contradicts-data).
 */
export function isSignalGateError(error: unknown): boolean {
  const structured = parseStructuredError(error);
  return SIGNAL_GATE.test(structured ? `${structured.title} ${structured.detail}` : rawErrorText(error));
}

/**
 * What a gate rejection reads as wherever no upgrade card catches it. The
 * backend's sentence offers a free trial, which starts by itself on first
 * launch and cannot be started again, so it is never shown.
 */
function signalGateMessage(error: unknown): string | null {
  if (!isSignalGateError(error)) return null;
  return t('errors:errorMsg.signalRequired', 'This is part of 4DA Signal. Upgrade to unlock it.');
}

/** When a refused call's daily AI budget comes back (local time). */
export interface BudgetReset {
  /** Local reset time, `HH:MM`. */
  time: string;
  /** Minutes until the reset. */
  minutes: number;
}

/**
 * The reset carried by a budget refusal. The backend
 * (`state_llm_limits::daily_budget_reached_message`) is the sole producer: its
 * English sentence ends with a `[budget_reset:HH:MM:MINUTES]` token so the UI
 * can say it in the user's language. A budget refusal used to surface as
 * "Assessment failed — try again" although retrying could not succeed until
 * the reset (audit L2, 2026-10-12).
 */
export function parseBudgetReset(error: unknown): BudgetReset | null {
  const match = /\[budget_reset:(\d{2}:\d{2}):(\d+)\]/.exec(rawErrorText(error));
  if (!match) return null;
  return { time: match[1]!, minutes: Number(match[2]) };
}

/**
 * A brief deferred while the startup dependency rescan runs: it would have
 * stated facts from the previous session's inventory (live 2026-10-18).
 */
export function inventoryRefreshingMessage(): string {
  return t(
    'errors:errorMsg.inventoryRefreshing',
    'Your projects are being rescanned. The brief will be written as soon as the scan finishes.',
  );
}

/** The brief is still being written after the IPC wait ran out: progress, not an error. */
export function briefStillWritingMessage(): string {
  return t(
    'errors:errorMsg.briefStillWriting',
    'Still writing your brief. It will appear here as soon as it is ready.',
  );
}

/**
 * The localized budget refusal when `error` is one, else null. For surfaces
 * that otherwise show the backend's own (already user-facing) message: only
 * the budget case is replaced, so the token never renders verbatim.
 */
export function budgetRefusalMessage(error: unknown): string | null {
  const reset = parseBudgetReset(error);
  if (!reset) return null;
  const vars = budgetResetVars(reset, currentLanguage());
  return t('errors:errorMsg.budgetReached', englishBudgetReached(vars), vars);
}

/** `{time, in}` for the budget keys; `in` is a localized "in 3 hours". */
export function budgetResetVars(reset: BudgetReset, language: string): { time: string; in: string } {
  let rtf: Intl.RelativeTimeFormat;
  try {
    rtf = new Intl.RelativeTimeFormat(language, { numeric: 'always' });
  } catch {
    rtf = new Intl.RelativeTimeFormat('en', { numeric: 'always' });
  }
  // Whole hours only when that is within 5 minutes of the truth ("in 3
  // hours" for 3 h 2 min); otherwise exact minutes ("in 89 minutes", never
  // "in 1 hour" for 1 h 29 min).
  const minutes = Math.max(1, reset.minutes);
  const remainder = minutes % 60;
  const inText =
    minutes >= 60 && (remainder < 5 || remainder > 55)
      ? rtf.format(Math.round(minutes / 60), 'hour')
      : rtf.format(minutes, 'minute');
  return { time: reset.time, in: inText };
}

function englishBudgetReached(vars: { time: string; in: string }): string {
  return `Daily AI budget reached — resets at ${vars.time} (${vars.in})`;
}

function currentLanguage(): string {
  const i18n = (window as unknown as Record<string, unknown>).__4da_i18n as { language?: string } | undefined;
  return i18n?.language ?? 'en';
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
  return [translateError(error), rawErrorText(error)];
}
