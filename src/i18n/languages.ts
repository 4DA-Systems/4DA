// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The shipped languages and how the app picks one. No i18next import: the
 * boot screen (src/boot) reads this before the app's module graph has loaded,
 * and must show its few lines in the same language the app will then use.
 */

// Supported languages — a subset of the locale directories in src/locales/.
// Ships 12 languages. `ar` is the one locale present in src/locales/ but NOT
// listed here (RTL layout untested), so its translations are unreachable at
// runtime; the files are preserved in the repo for future activation.
// Single source of truth for shipped languages, in display order, with native
// names. Used by i18n init AND every language picker so the offered set can
// never diverge from what the app actually ships (see LanguageSwitcher).
export const SUPPORTED_LANGUAGES: ReadonlyArray<{ code: string; native: string }> = [
  { code: 'en', native: 'English' },
  { code: 'de', native: 'Deutsch' },
  { code: 'es', native: 'Español' },
  { code: 'fr', native: 'Français' },
  { code: 'hi', native: 'हिन्दी' },
  { code: 'it', native: 'Italiano' },
  { code: 'ja', native: '日本語' },
  { code: 'ko', native: '한국어' },
  { code: 'pt-BR', native: 'Português' },
  { code: 'ru', native: 'Русский' },
  { code: 'tr', native: 'Türkçe' },
  { code: 'zh', native: '中文' },
];

/** localStorage key i18n reads on init and every picker must write to. */
export const LANGUAGE_STORAGE_KEY = '4da_language';

const SUPPORTED_LANGS = new Set(SUPPORTED_LANGUAGES.map((l) => l.code));

function savedLanguage(): string | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage.getItem(LANGUAGE_STORAGE_KEY) : null;
  } catch {
    return null; // storage unavailable
  }
}

// Handles regional variants: navigator.language "pt-BR" → match "pt-BR" directly,
// "pt-PT" → fall back to "en" (no generic pt), "zh-TW" → fall back to "zh".
function detectSystemLanguage(): string {
  if (typeof navigator === 'undefined') return 'en';
  const languages = navigator.languages ?? [navigator.language ?? 'en'];
  for (const tag of languages) {
    // Try exact match first (e.g., "pt-BR")
    if (SUPPORTED_LANGS.has(tag)) return tag;
    // Try base language (e.g., "de-AT" → "de")
    const base = tag.split('-')[0] ?? '';
    if (base && SUPPORTED_LANGS.has(base)) return base;
  }
  return 'en';
}

/** The language to start in: the saved preference, else the system locale, else English. */
export function initialLanguage(): string {
  return savedLanguage() || detectSystemLanguage() || 'en';
}
