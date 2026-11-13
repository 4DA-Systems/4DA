// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Every literal i18n key the app uses exists in the English locale.
 *
 * Audit 2026-11-11: 163 keys were used in code but missing from en/*.json.
 * They rendered their inline English default in every language, so no
 * translation could ever reach them and the translation tooling never saw
 * them. English is the fallback for every other locale, so a key that is in
 * en/ is at worst shown in English, and can be translated.
 */
import { describe, it, expect } from 'vitest';
import ui from './en/ui.json';
import errors from './en/errors.json';
import signals from './en/signals.json';
import usage from './en/usage.json';

const EN: Record<string, Record<string, unknown>> = { ui, errors, signals, usage };

/** A flat key ("a.b.c" as written) or a nested path, as i18next resolves it. */
function has(table: Record<string, unknown>, key: string): boolean {
  if (key in table) return true;
  let node: unknown = table;
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null || !(part in node)) return false;
    node = (node as Record<string, unknown>)[part];
  }
  return true;
}

const SOURCES = import.meta.glob(
  ['../**/*.{ts,tsx}', '!../**/*.test.{ts,tsx}', '!../**/*.d.ts', '!../locales/**', '!../test/**', '!../**/__tests__/**'],
  { query: '?raw', import: 'default', eager: true },
);

describe('English locale', () => {
  it('holds every literal key the code asks for', () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(100);
    const call = /(?<![\w.$])(?:i18n\.)?t\(\s*(['"`])([A-Za-z0-9_.:-]+)\1/g;
    const missing = new Set<string>();
    for (const [file, raw] of Object.entries(SOURCES)) {
      const text = String(raw);
      const fileNs = text.match(/useTranslation\(\s*['"](\w+)['"]/)?.[1] ?? 'ui';
      for (const m of text.matchAll(call)) {
        let key = m[2]!;
        if (key.endsWith('.') || !key.includes('.')) continue;
        let ns = fileNs;
        if (key.includes(':')) [ns, key] = key.split(':') as [string, string];
        const table = EN[ns];
        if (!table) continue;
        if (has(table, key) || has(table, `${key}_one`) || has(table, `${key}_other`)) continue;
        missing.add(`${ns}:${key} (${file})`);
      }
    }
    expect([...missing]).toEqual([]);
  });
});
