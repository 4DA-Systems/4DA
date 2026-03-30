import { useState, useEffect, useCallback } from 'react';
import { cmd } from '../../lib/commands';
import { useTranslation } from 'react-i18next';
import { TranslationEditor } from './TranslationEditor';

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

export function LocaleSection() {
  const { t, i18n } = useTranslation();
  const [country, setCountry] = useState('US');
  const [language, setLanguage] = useState('en');
  const [currency, setCurrency] = useState('USD');
  const [loaded, setLoaded] = useState(false);
  const [translationCoverage, setTranslationCoverage] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const locale = await cmd('get_locale');
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
      await cmd('set_locale', { country: c, language: l, currency: cur });
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
    i18n.changeLanguage(code);
    localStorage.setItem('4da_language', code);
    saveLocale(country, code, currency);
    // Check translation coverage for the new language
    if (code !== 'en') {
      cmd('get_translation_status', { lang: code })
        .then((status) => setTranslationCoverage(Math.round(status.coverage ?? 0)))
        .catch(() => setTranslationCoverage(null));
    } else {
      setTranslationCoverage(null);
    }
  }, [country, currency, i18n, saveLocale]);

  const handleCurrencyChange = useCallback((cur: string) => {
    setCurrency(cur);
    saveLocale(country, language, cur);
  }, [country, language, saveLocale]);

  const [showEditor, setShowEditor] = useState(false);

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-blue-400">&#x1f310;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">{t('settings.locale.title')}</h3>
          <p className="text-text-muted text-sm mt-1">
            {t('settings.locale.description')}
          </p>
        </div>
      </div>

      {loaded ? (
        <div className="space-y-3">
          <div>
            <label className="block text-xs text-text-muted uppercase tracking-wider mb-1.5">
              {t('settings.locale.country')}
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
              <label className="block text-xs text-text-muted uppercase tracking-wider mb-1.5">
                {t('settings.locale.language')}
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
              <label className="block text-xs text-text-muted uppercase tracking-wider mb-1.5">
                {t('settings.locale.currency')}
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

          <p className="text-xs text-text-muted pt-1">
            {t('settings.locale.priceInfo', { currency, language: getLanguageName(language) })}
          </p>

          {/* Translation coverage indicator */}
          {language !== 'en' && translationCoverage !== null && (
            <div className="flex items-center gap-2 pt-1">
              <div className="flex-1 h-1.5 bg-bg-secondary rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${
                    translationCoverage >= 95 ? 'bg-green-500' :
                    translationCoverage >= 80 ? 'bg-amber-500' : 'bg-red-500'
                  }`}
                  style={{ width: `${translationCoverage}%` }}
                />
              </div>
              <span className={`text-[10px] ${
                translationCoverage >= 95 ? 'text-green-400' :
                translationCoverage >= 80 ? 'text-amber-400' : 'text-red-400'
              }`}>
                {translationCoverage}%
              </span>
            </div>
          )}

          {/* Translation Editor toggle (only for non-English) */}
          {language !== 'en' && (
            <div className="pt-2">
              <button
                onClick={() => setShowEditor(!showEditor)}
                className="text-xs text-accent-gold hover:text-[#C4A030] transition-colors"
              >
                {showEditor ? '- ' : '+ '}
                {t('settings.translations.editorToggle')}
              </button>
              {showEditor && (
                <div className="mt-3">
                  <TranslationEditor language={language} />
                </div>
              )}
            </div>
          )}
        </div>
      ) : (
        <div className="text-sm text-text-muted">Detecting region...</div>
      )}
    </div>
  );
}
