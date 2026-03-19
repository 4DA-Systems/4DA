import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import resourcesToBackend from 'i18next-resources-to-backend';

// Detect saved language preference or system locale
const savedLang = typeof localStorage !== 'undefined'
  ? localStorage.getItem('4da_language')
  : null;
const systemLang = typeof navigator !== 'undefined'
  ? navigator.language?.split('-')[0]
  : 'en';

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
    ns: ['ui', 'coach', 'streets', 'errors'],
    interpolation: {
      escapeValue: false, // React already escapes
    },
    react: {
      useSuspense: false, // Desktop app, no need for suspense
    },
  });

export default i18n;
