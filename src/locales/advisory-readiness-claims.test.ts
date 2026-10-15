// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Day-one copy must match what the advisory check has actually done (PR #961
// review). Blind Spots said "advisories that affect you are already in the
// Worklist" before any advisory had been downloaded, and the first-run screen
// reused the "starts within a few minutes" line for a stale check.
import { describe, expect, it } from 'vitest';

import ar from './ar/ui.json';
import de from './de/ui.json';
import en from './en/ui.json';
import es from './es/ui.json';
import fr from './fr/ui.json';
import hi from './hi/ui.json';
import it_ from './it/ui.json';
import ja from './ja/ui.json';
import ko from './ko/ui.json';
import ptBR from './pt-BR/ui.json';
import ru from './ru/ui.json';
import tr from './tr/ui.json';
import zh from './zh/ui.json';

// A new locale directory must be added here too.
const LOCALE_FILES: Record<string, Record<string, unknown>> = {
  ar, de, en, es, fr, hi, it: it_, ja, ko, 'pt-BR': ptBR, ru, tr, zh,
};
const locales = Object.keys(LOCALE_FILES);

function ui(locale: string): Record<string, string> {
  return (LOCALE_FILES[locale] ?? {}) as Record<string, string>;
}

describe('advisory-readiness copy', () => {
  it('Blind Spots does not claim advisories are already listed before the check', () => {
    expect(en['blindspots.firstDay.assessingNow']).not.toMatch(/already/i);
    expect(en['blindspots.firstDay.assessingNow']).toMatch(/once your dependencies have been checked/);
  });

  it.each(locales)('%s: a stale check has its own first-run line', (locale) => {
    const strings = ui(locale);
    expect(strings['firstRun.advisory.stale']).toBeTruthy();
    expect(strings['firstRun.advisory.stale']).not.toBe(strings['firstRun.advisory.notStarted']);
  });

  it.each(locales)('%s: no ecosystem count label (the number was wrong)', (locale) => {
    expect(ui(locale)['firstRun.ecosystems']).toBeUndefined();
  });
});
