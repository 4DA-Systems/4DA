import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface SetupLocaleProps {
  onLocaleChange: (country: string, language: string, currency: string) => void;
}

const COUNTRIES = [
  { code: 'US', name: 'United States', lang: 'en', currency: 'USD' },
  { code: 'GB', name: 'United Kingdom', lang: 'en', currency: 'GBP' },
  { code: 'DE', name: 'Germany', lang: 'de', currency: 'EUR' },
  { code: 'FR', name: 'France', lang: 'fr', currency: 'EUR' },
  { code: 'NL', name: 'Netherlands', lang: 'en', currency: 'EUR' },
  { code: 'CA', name: 'Canada', lang: 'en', currency: 'CAD' },
  { code: 'AU', name: 'Australia', lang: 'en', currency: 'AUD' },
  { code: 'JP', name: 'Japan', lang: 'ja', currency: 'JPY' },
  { code: 'IN', name: 'India', lang: 'hi', currency: 'INR' },
  { code: 'BR', name: 'Brazil', lang: 'pt-BR', currency: 'BRL' },
  { code: 'IT', name: 'Italy', lang: 'it', currency: 'EUR' },
  { code: 'ES', name: 'Spain', lang: 'es', currency: 'EUR' },
  { code: 'SE', name: 'Sweden', lang: 'en', currency: 'SEK' },
  { code: 'NO', name: 'Norway', lang: 'en', currency: 'NOK' },
  { code: 'DK', name: 'Denmark', lang: 'en', currency: 'DKK' },
  { code: 'CH', name: 'Switzerland', lang: 'de', currency: 'CHF' },
  { code: 'KR', name: 'South Korea', lang: 'ko', currency: 'KRW' },
  { code: 'NZ', name: 'New Zealand', lang: 'en', currency: 'NZD' },
  { code: 'AT', name: 'Austria', lang: 'de', currency: 'EUR' },
  { code: 'BE', name: 'Belgium', lang: 'en', currency: 'EUR' },
  { code: 'IE', name: 'Ireland', lang: 'en', currency: 'EUR' },
  { code: 'PT', name: 'Portugal', lang: 'en', currency: 'EUR' },
  { code: 'FI', name: 'Finland', lang: 'en', currency: 'EUR' },
  { code: 'SG', name: 'Singapore', lang: 'en', currency: 'SGD' },
  { code: 'MX', name: 'Mexico', lang: 'es', currency: 'MXN' },
];

const CURRENCIES = [
  'USD', 'EUR', 'GBP', 'CAD', 'AUD', 'JPY', 'INR', 'BRL',
  'CHF', 'SEK', 'NOK', 'DKK', 'NZD', 'KRW', 'SGD', 'MXN', 'CNY',
];

const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'es', name: 'Espa\u00f1ol' },
  { code: 'fr', name: 'Fran\u00e7ais' },
  { code: 'de', name: 'Deutsch' },
  { code: 'it', name: 'Italiano' },
  { code: 'pt-BR', name: 'Portugu\u00eas (BR)' },
  { code: 'ru', name: '\u0420\u0443\u0441\u0441\u043a\u0438\u0439' },
  { code: 'ja', name: '\u65e5\u672c\u8a9e' },
  { code: 'ko', name: '\ud55c\uad6d\uc5b4' },
  { code: 'zh', name: '\u4e2d\u6587' },
  { code: 'tr', name: 'T\u00fcrk\u00e7e' },
  { code: 'hi', name: '\u0939\u093f\u0928\u094d\u0926\u0940' },
  { code: 'ar', name: '\u0627\u0644\u0639\u0631\u0628\u064a\u0629' },
];

function getLanguageName(code: string): string {
  return LANGUAGES.find(l => l.code === code)?.name ?? code;
}

export function SetupLocale({ onLocaleChange }: SetupLocaleProps) {
  const { t } = useTranslation();
  const [country, setCountry] = useState('US');
  const [language, setLanguage] = useState('en');
  const [currency, setCurrency] = useState('USD');
  const [loaded, setLoaded] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const userInteracted = useRef(false);

  // Load current locale from backend on mount
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const locale = await cmd('get_locale');
        if (cancelled || userInteracted.current) return;
        setCountry(locale.country);
        setLanguage(locale.language);
        setCurrency(locale.currency);
      } catch {
        // Default values already set
      }
      setLoaded(true);
    })();
    return () => { cancelled = true; };
  }, []);

  const saveLocale = useCallback(async (c: string, l: string, cur: string) => {
    setSaveError(null);
    try {
      await cmd('set_locale', { country: c, language: l, currency: cur });
      onLocaleChange(c, l, cur);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch {
      setSaveError(t('onboarding.locale.saveFailed'));
    }
  }, [onLocaleChange, t]);

  const handleCountryChange = useCallback((code: string) => {
    userInteracted.current = true;
    setCountry(code);
    const match = COUNTRIES.find(c => c.code === code);
    if (match) {
      setLanguage(match.lang);
      setCurrency(match.currency);
      saveLocale(code, match.lang, match.currency);
    } else {
      saveLocale(code, language, currency);
    }
  }, [language, currency, saveLocale]);

  const handleLanguageChange = useCallback((code: string) => {
    userInteracted.current = true;
    setLanguage(code);
    saveLocale(country, code, currency);
  }, [country, currency, saveLocale]);

  const handleCurrencyChange = useCallback((cur: string) => {
    userInteracted.current = true;
    setCurrency(cur);
    saveLocale(country, language, cur);
  }, [country, language, saveLocale]);

  if (!loaded) {
    return (
      <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border">
        <p className="text-sm text-text-muted">{t('onboarding.locale.detecting')}</p>
      </div>
    );
  }

  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
      {/* Country */}
      <div>
        <label className="block text-xs text-text-secondary uppercase tracking-wider mb-1.5">
          {t('onboarding.locale.country')}
        </label>
        <select
          value={country}
          onChange={(e) => handleCountryChange(e.target.value)}
          aria-label={t('onboarding.locale.selectCountry')}
          className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
        >
          {COUNTRIES.map((c) => (
            <option key={c.code} value={c.code}>{c.name}</option>
          ))}
        </select>
      </div>

      {/* Language & Currency side by side */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-xs text-text-secondary uppercase tracking-wider mb-1.5">
            {t('onboarding.locale.language')}
          </label>
          <select
            value={language}
            onChange={(e) => handleLanguageChange(e.target.value)}
            aria-label={t('onboarding.locale.selectLanguage')}
            className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
          >
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>{l.name}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs text-text-secondary uppercase tracking-wider mb-1.5">
            {t('onboarding.locale.currency')}
          </label>
          <select
            value={currency}
            onChange={(e) => handleCurrencyChange(e.target.value)}
            aria-label={t('onboarding.locale.selectCurrency')}
            className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
          >
            {CURRENCIES.map((c) => (
              <option key={c} value={c}>{c}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Preview */}
      <div className="flex items-center gap-2">
        <p className="text-xs text-text-muted">
          {t('onboarding.locale.priceInfo', { currency, language: getLanguageName(language) })}
        </p>
        {saved && <span className="text-xs text-green-400">{t('onboarding.locale.saved')}</span>}
      </div>
      {saveError && <p className="text-xs text-red-400">{saveError}</p>}
    </div>
  );
}
