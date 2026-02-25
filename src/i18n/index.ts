import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

// Import locale files directly (bundled, no HTTP backend needed for desktop app)
import enUi from '../locales/en/ui.json';
import enCoach from '../locales/en/coach.json';
import enStreets from '../locales/en/streets.json';
import enErrors from '../locales/en/errors.json';

i18n
  .use(initReactI18next)
  .init({
    resources: {
      en: {
        ui: enUi,
        coach: enCoach,
        streets: enStreets,
        errors: enErrors,
      },
    },
    lng: 'en',
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
