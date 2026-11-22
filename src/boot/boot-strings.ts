// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The boot screen's few lines of text, in the language the app will start in.
 *
 * i18next sits behind the dynamic App import, so it is not available while the
 * boot screen is on screen. Each locale directory carries a tiny `boot.json`
 * instead; all of them are bundled with the entry (a few hundred bytes each).
 * A language without the file, or without a key, reads in English.
 */
import { initialLanguage } from '../i18n/languages';
import en from '../locales/en/boot.json';

export type BootStrings = typeof en;

const TABLES = import.meta.glob<Partial<BootStrings>>('../locales/*/boot.json', {
  eager: true,
  import: 'default',
});

/** Languages that ship a boot table (for tests). */
export function bootLanguages(): string[] {
  return Object.keys(TABLES)
    .map((file) => file.split('/').slice(-2)[0] ?? '')
    .filter(Boolean);
}

export function bootStrings(language: string = initialLanguage()): BootStrings {
  return { ...en, ...TABLES[`../locales/${language}/boot.json`] };
}
