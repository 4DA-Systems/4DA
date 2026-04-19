// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import resourcesToBackend from 'i18next-resources-to-backend';

// Supported languages — must match locale directories in src/locales/
// Ship 10 languages — ar/hi/it deferred (RTL untested / insufficient market demand)
// Locale files preserved in repo for future activation.
const SUPPORTED_LANGS = new Set([
  'en', 'de', 'es', 'fr', 'ja', 'ko', 'pt-BR', 'ru', 'tr', 'zh',
]);

// Detect saved language preference or system locale.
// Handles regional variants: navigator.language "pt-BR" → match "pt-BR" directly,
// "pt-PT" → fall back to "en" (no generic pt), "zh-TW" → fall back to "zh".
const savedLang = typeof localStorage !== 'undefined'
  ? localStorage.getItem('4da_language')
  : null;

function detectSystemLang(): string {
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
const systemLang = detectSystemLang();

i18n
  .use(
    resourcesToBackend(
      (language: string, namespace: string) =>
        import(`../locales/${language}/${namespace}.json`),
    ),
  )
  .use(initReactI18next)
  .init({
    lng: savedLang || systemLang || 'en',
    fallbackLng: 'en',
    defaultNS: 'ui',
    ns: ['ui', 'coach', 'streets', 'errors', 'signals'],
    interpolation: {
      escapeValue: false, // React already escapes
    },
    react: {
      useSuspense: false, // Desktop app, no need for suspense
    },
  });

// Expose i18n instance for non-React code (error-messages.ts, etc.)
// This avoids circular import issues while giving utilities access to translations.
(window as unknown as Record<string, unknown>).__4da_i18n = i18n;

export default i18n;
