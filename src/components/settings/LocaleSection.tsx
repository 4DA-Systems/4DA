import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

const COUNTRIES = [
  { code: 'US', name: 'United States', lang: 'en', currency: 'USD' },
  { code: 'GB', name: 'United Kingdom', lang: 'en', currency: 'GBP' },
  { code: 'DE', name: 'Germany', lang: 'de', currency: 'EUR' },
  { code: 'FR', name: 'France', lang: 'fr', currency: 'EUR' },
  { code: 'NL', name: 'Netherlands', lang: 'nl', currency: 'EUR' },
  { code: 'CA', name: 'Canada', lang: 'en', currency: 'CAD' },
  { code: 'AU', name: 'Australia', lang: 'en', currency: 'AUD' },
  { code: 'JP', name: 'Japan', lang: 'ja', currency: 'JPY' },
  { code: 'IN', name: 'India', lang: 'en', currency: 'INR' },
  { code: 'BR', name: 'Brazil', lang: 'pt', currency: 'BRL' },
  { code: 'IT', name: 'Italy', lang: 'it', currency: 'EUR' },
  { code: 'ES', name: 'Spain', lang: 'es', currency: 'EUR' },
  { code: 'SE', name: 'Sweden', lang: 'sv', currency: 'SEK' },
  { code: 'NO', name: 'Norway', lang: 'no', currency: 'NOK' },
  { code: 'DK', name: 'Denmark', lang: 'da', currency: 'DKK' },
  { code: 'CH', name: 'Switzerland', lang: 'de', currency: 'CHF' },
  { code: 'KR', name: 'South Korea', lang: 'ko', currency: 'KRW' },
  { code: 'NZ', name: 'New Zealand', lang: 'en', currency: 'NZD' },
  { code: 'AT', name: 'Austria', lang: 'de', currency: 'EUR' },
  { code: 'BE', name: 'Belgium', lang: 'nl', currency: 'EUR' },
  { code: 'IE', name: 'Ireland', lang: 'en', currency: 'EUR' },
  { code: 'PT', name: 'Portugal', lang: 'pt', currency: 'EUR' },
  { code: 'FI', name: 'Finland', lang: 'fi', currency: 'EUR' },
  { code: 'SG', name: 'Singapore', lang: 'en', currency: 'SGD' },
  { code: 'MX', name: 'Mexico', lang: 'es', currency: 'MXN' },
];

const CURRENCIES = [
  'USD', 'EUR', 'GBP', 'CAD', 'AUD', 'JPY', 'INR', 'BRL',
  'CHF', 'SEK', 'NOK', 'DKK', 'NZD', 'KRW', 'SGD', 'MXN', 'CNY',
];

const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'fr', name: 'Francais' },
  { code: 'es', name: 'Espanol' },
  { code: 'pt', name: 'Portugues' },
  { code: 'nl', name: 'Nederlands' },
  { code: 'it', name: 'Italiano' },
  { code: 'ja', name: 'Japanese' },
  { code: 'ko', name: 'Korean' },
  { code: 'sv', name: 'Svenska' },
  { code: 'no', name: 'Norsk' },
  { code: 'da', name: 'Dansk' },
  { code: 'fi', name: 'Suomi' },
];

function getLanguageName(code: string): string {
  return LANGUAGES.find(l => l.code === code)?.name ?? code;
}

export function LocaleSection() {
  const [country, setCountry] = useState('US');
  const [language, setLanguage] = useState('en');
  const [currency, setCurrency] = useState('USD');
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const locale = await invoke<{ country: string; language: string; currency: string }>('get_locale');
        if (cancelled) return;
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
    try {
      await invoke('set_locale', { country: c, language: l, currency: cur });
    } catch {
      // Silent failure - locale is not critical
    }
  }, []);

  const handleCountryChange = useCallback((code: string) => {
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
    setLanguage(code);
    saveLocale(country, code, currency);
  }, [country, currency, saveLocale]);

  const handleCurrencyChange = useCallback((cur: string) => {
    setCurrency(cur);
    saveLocale(country, language, cur);
  }, [country, language, saveLocale]);

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-blue-400">&#x1f310;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">Language &amp; Region</h3>
          <p className="text-gray-500 text-sm mt-1">
            Set your country, language, and currency for localized content
          </p>
        </div>
      </div>

      {loaded ? (
        <div className="space-y-3">
          <div>
            <label className="block text-xs text-gray-500 uppercase tracking-wider mb-1.5">
              Country
            </label>
            <select
              value={country}
              onChange={(e) => handleCountryChange(e.target.value)}
              className="w-full bg-bg-secondary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
            >
              {COUNTRIES.map((c) => (
                <option key={c.code} value={c.code}>{c.name}</option>
              ))}
            </select>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-gray-500 uppercase tracking-wider mb-1.5">
                Language
              </label>
              <select
                value={language}
                onChange={(e) => handleLanguageChange(e.target.value)}
                className="w-full bg-bg-secondary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
              >
                {LANGUAGES.map((l) => (
                  <option key={l.code} value={l.code}>{l.name}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs text-gray-500 uppercase tracking-wider mb-1.5">
                Currency
              </label>
              <select
                value={currency}
                onChange={(e) => handleCurrencyChange(e.target.value)}
                className="w-full bg-bg-secondary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
              >
                {CURRENCIES.map((c) => (
                  <option key={c} value={c}>{c}</option>
                ))}
              </select>
            </div>
          </div>

          <p className="text-xs text-gray-500 pt-1">
            Prices in {currency}, content prioritized in {getLanguageName(language)}
          </p>
        </div>
      ) : (
        <div className="text-sm text-gray-500">Detecting region...</div>
      )}
    </div>
  );
}
