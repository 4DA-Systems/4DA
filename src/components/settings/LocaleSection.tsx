import { useState, useEffect, useCallback } from 'react';
import { cmd } from '../../lib/commands';
import { useTranslation } from 'react-i18next';
import { TranslationEditor } from './TranslationEditor';
import type { TranslationConfig } from '../../lib/commands';

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

const TRANSLATION_PROVIDERS = [
  { value: 'auto', labelKey: 'settings.translation.providerAuto' },
  { value: 'deepl', labelKey: 'settings.translation.providerDeepL' },
  { value: 'google', labelKey: 'settings.translation.providerGoogle' },
  { value: 'azure', labelKey: 'settings.translation.providerAzure' },
  { value: 'ollama', labelKey: 'settings.translation.providerOllama' },
  { value: 'llm', labelKey: 'settings.translation.providerLLM' },
] as const;

const PROVIDERS_REQUIRING_KEY = new Set(['deepl', 'google', 'azure']);

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
  const [txConfig, setTxConfig] = useState<TranslationConfig>({
    provider: 'auto', api_key: '', auto_translate: false, translate_descriptions: false,
  });
  const [embeddingInfo, setEmbeddingInfo] = useState<{
    model: string; reembed_in_progress: boolean; multilingual_model: string;
  } | null>(null);

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

  useEffect(() => {
    cmd('get_translation_config').then(setTxConfig).catch(() => {});
    cmd('get_embedding_model_info').then(setEmbeddingInfo).catch(() => {});
  }, []);

  const saveTxConfig = useCallback(async (next: TranslationConfig) => {
    setTxConfig(next);
    try { await cmd('set_translation_config', { config: next }); } catch { /* non-critical */ }
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

          {/* Translation Provider config (only for non-English) */}
          {language !== 'en' && (
            <div className="pt-4 mt-4 border-t border-border space-y-3">
              <h4 className="text-sm text-white font-medium">{t('settings.translation.title')}</h4>

              <div>
                <label className="block text-xs text-text-muted uppercase tracking-wider mb-1.5">
                  {t('settings.translation.provider')}
                </label>
                <select
                  value={txConfig.provider}
                  onChange={(e) => saveTxConfig({ ...txConfig, provider: e.target.value })}
                  className="w-full bg-bg-secondary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
                >
                  {TRANSLATION_PROVIDERS.map((p) => (
                    <option key={p.value} value={p.value}>{t(p.labelKey)}</option>
                  ))}
                </select>
              </div>

              {PROVIDERS_REQUIRING_KEY.has(txConfig.provider) && (
                <div>
                  <label className="block text-xs text-text-muted uppercase tracking-wider mb-1.5">
                    {t('settings.translation.apiKey')}
                  </label>
                  <input
                    type="password"
                    value={txConfig.api_key}
                    onChange={(e) => setTxConfig({ ...txConfig, api_key: e.target.value })}
                    onBlur={() => saveTxConfig(txConfig)}
                    className="w-full bg-bg-secondary border border-border rounded-lg px-3 py-2 text-white text-sm focus:border-orange-500 focus:outline-none"
                    placeholder="sk-..."
                  />
                  <p className="text-[10px] text-text-muted mt-1">
                    {t('settings.translation.apiKeyHelp')}
                  </p>
                </div>
              )}

              <div className="flex items-center justify-between">
                <span className="text-sm text-white">{t('settings.translation.autoTranslate')}</span>
                <button
                  onClick={() => saveTxConfig({ ...txConfig, auto_translate: !txConfig.auto_translate })}
                  className={`relative w-10 h-5 rounded-full transition-colors ${
                    txConfig.auto_translate ? 'bg-green-500/40' : 'bg-gray-600'
                  }`}
                >
                  <span
                    className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                      txConfig.auto_translate ? 'translate-x-5' : ''
                    }`}
                  />
                </button>
              </div>

              <div className="flex items-center justify-between">
                <span className="text-sm text-white">{t('settings.translation.translateDescriptions')}</span>
                <button
                  onClick={() => saveTxConfig({ ...txConfig, translate_descriptions: !txConfig.translate_descriptions })}
                  className={`relative w-10 h-5 rounded-full transition-colors ${
                    txConfig.translate_descriptions ? 'bg-green-500/40' : 'bg-gray-600'
                  }`}
                >
                  <span
                    className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                      txConfig.translate_descriptions ? 'translate-x-5' : ''
                    }`}
                  />
                </button>
              </div>
            </div>
          )}

          {/* Embedding Model info (read-only) */}
          {embeddingInfo && (
            <div className="pt-4 mt-4 border-t border-border space-y-2">
              <h4 className="text-sm text-white font-medium">{t('settings.translation.embedding')}</h4>
              <p className="text-xs text-text-muted">
                {t('settings.translation.embeddingCurrent', { model: embeddingInfo.model })}
              </p>
              {embeddingInfo.reembed_in_progress && (
                <div className="flex items-center gap-2">
                  <span className="w-3 h-3 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
                  <span className="text-xs text-orange-400">
                    {t('settings.translation.embeddingReindexing')}
                  </span>
                </div>
              )}
              {language !== 'en' && embeddingInfo.multilingual_model && (
                <p className="text-xs text-text-muted">
                  {t('settings.translation.embeddingMultilingual', { model: embeddingInfo.multilingual_model })}
                </p>
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
