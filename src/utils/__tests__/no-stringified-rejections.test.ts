// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// A backend rejection is usually an OBJECT (src-tauri/src/error.rs), so
// `String(e)` and `${error}` print "[object Object]". Sixty-seven call sites
// did exactly that in onboarding, Settings and the stores before W0-C routed
// them through `readableError` / `rawErrorText` (utils/error-messages.ts).
// This scan keeps the next catch block from reintroducing it.
import { describe, expect, it } from 'vitest';

// Every non-test source file under src/, as text, keyed by its path from here.
const SOURCES = import.meta.glob<string>(
  ['../../**/*.{ts,tsx}', '!../../**/*.test.*', '!../../**/*.spec.*', '!../../**/*.d.ts'],
  { query: '?raw', import: 'default', eager: true },
);

// Files allowed to stringify a rejection, each for a stated reason.
const ALLOWED = new Set([
  // The one module that turns a rejection into text.
  'utils/error-messages.ts',
  // Local diagnostic log only: it JSON-encodes objects before this fallback.
  'lib/error-reporter.ts',
]);

// A caught value, by the names this codebase gives it, stringified whole.
const CAUGHT = '(?:e|err|error|reason|lastError)';
const STRINGIFIED = [
  new RegExp(`String\\(\\s*${CAUGHT}\\s*(?:\\?\\?[^)]*)?\\)`),
  new RegExp(`\\$\\{\\s*${CAUGHT}\\s*\\}`),
];

describe('rejections are never stringified whole', () => {
  it('finds source files to scan', () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(300);
    expect(Object.keys(SOURCES)).toContain('../error-messages.ts');
  });

  it('no catch site prints a rejection with String(e) or a template literal', () => {
    const offenders: string[] = [];
    for (const [path, text] of Object.entries(SOURCES)) {
      // '../error-messages.ts' and '../../store/x.ts' both become paths from src/.
      const name = path.startsWith('../../') ? path.slice('../../'.length) : `utils/${path.slice('../'.length)}`;
      if (ALLOWED.has(name)) continue;
      text.split('\n').forEach((line, index) => {
        // Comments may quote the old pattern when explaining it.
        if (/^\s*(\/\/|\*|\/\*)/.test(line)) return;
        if (STRINGIFIED.some((pattern) => pattern.test(line))) offenders.push(`${name}:${index + 1}: ${line.trim()}`);
      });
    }
    expect(offenders, 'use readableError(e) to show it, rawErrorText(e) to match or log it').toEqual([]);
  });
});
