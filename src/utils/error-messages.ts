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
  /** User-friendly replacement message */
  message: string;
}

const ERROR_PATTERNS: ErrorPattern[] = [
  // Network / connectivity
  {
    test: /failed to fetch|network error|reqwest/i,
    message: 'Network request failed. Check your internet connection.',
  },

  // Timeout
  {
    test: /timeout|timed?\s*out|deadline.*exceeded/i,
    message: 'Request timed out. Try again in a moment.',
  },

  // API keys / authentication
  {
    test: /api\s*key|unauthorized|401|403|invalid.*key/i,
    message: 'Authentication failed. Check your API key in Settings.',
  },

  // Rate limiting
  {
    test: /rate\s*limit|429|too many requests/i,
    message: 'Rate limit reached. Please wait a moment and try again.',
  },

  // Ollama not running (must come before general Ollama and network connection-refused)
  {
    test: /ollama.*not.*running|ollama.*connect|(?=.*connection refused)(?=.*ollama)|(?=.*ECONNREFUSED)(?=.*ollama)/i,
    message: 'Ollama is not running. Start Ollama or set an API key in Settings.',
  },

  // Ollama / embedding (general)
  {
    test: /ollama|embedding.*fail|model.*not.*found/i,
    message: 'Embedding service unavailable. Check that Ollama is running.',
  },

  // Network connection refused (after Ollama patterns to avoid catching Ollama errors)
  {
    test: /connection refused|ECONNREFUSED/i,
    message: 'Network request failed. Check your internet connection.',
  },

  // Database
  {
    test: /sqlite|database.*locked|disk\s*i\/o|database.*error/i,
    message: 'Database error. Try restarting the app.',
  },

  // File system permissions
  {
    test: /permission\s*denied|EACCES/i,
    message: 'Permission denied. Check file permissions.',
  },

  // Serialization / parsing
  {
    test: /serde|deserialize|json.*error|parse.*error/i,
    message: 'Data format error. Try again or restart the app.',
  },

  // Already running
  {
    test: /already running|already in progress/i,
    message: 'Already in progress. Please wait for it to complete.',
  },

  // File not found (kept from original, useful catch-all)
  {
    test: /no such file|file not found|ENOENT|path.*not.*exist/i,
    message: 'File not found. It may have been moved or deleted.',
  },

  // Tauri / IPC
  {
    test: /__TAURI__|invoke.*error|ipc.*error/i,
    message: 'App communication error. Please restart the app.',
  },

  // Git operations
  {
    test: /git.*error|not a git repository/i,
    message: 'Git operation failed. Check the repository path.',
  },
];

const FALLBACK_MESSAGE = 'Something went wrong. Please try again.';

/**
 * Translate a raw backend error into a user-friendly message.
 *
 * @param error - The raw error from a Tauri invoke catch block (can be any type)
 * @returns A user-friendly error string
 */
export function translateError(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error ?? '');

  if (!raw || raw === 'undefined' || raw === 'null') {
    return FALLBACK_MESSAGE;
  }

  for (const pattern of ERROR_PATTERNS) {
    if (pattern.test.test(raw)) {
      return pattern.message;
    }
  }

  return FALLBACK_MESSAGE;
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
