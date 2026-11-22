// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import resourcesToBackend from 'i18next-resources-to-backend';

import { initialLanguage } from './languages';

// The shipped languages, the storage key and the start-up choice live in
// ./languages so the boot screen can read them without loading i18next.
export { SUPPORTED_LANGUAGES, LANGUAGE_STORAGE_KEY } from './languages';

void i18n
  .use(
    resourcesToBackend(
      (language: string, namespace: string) =>
        import(`../locales/${language}/${namespace}.json`),
    ),
  )
  .use(initReactI18next)
  .init({
    lng: initialLanguage(),
    fallbackLng: 'en',
    defaultNS: 'ui',
    ns: ['ui', 'errors', 'signals', 'usage'],
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
