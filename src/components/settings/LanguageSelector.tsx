import { memo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

const LANGUAGES = [
  { code: 'en', name: 'English', native: 'English', flag: 'GB' },
  { code: 'ar', name: 'Arabic', native: 'العربية', flag: 'SA', rtl: true },
  { code: 'de', name: 'German', native: 'Deutsch', flag: 'DE' },
  { code: 'es', name: 'Spanish', native: 'Español', flag: 'ES' },
  { code: 'fr', name: 'French', native: 'Français', flag: 'FR' },
  { code: 'hi', name: 'Hindi', native: 'हिन्दी', flag: 'IN' },
  { code: 'it', name: 'Italian', native: 'Italiano', flag: 'IT' },
  { code: 'ja', name: 'Japanese', native: '日本語', flag: 'JP' },
  { code: 'ko', name: 'Korean', native: '한국어', flag: 'KR' },
  { code: 'pt-BR', name: 'Portuguese (BR)', native: 'Português', flag: 'BR' },
  { code: 'ru', name: 'Russian', native: 'Русский', flag: 'RU' },
  { code: 'tr', name: 'Turkish', native: 'Türkçe', flag: 'TR' },
  { code: 'zh', name: 'Chinese', native: '中文', flag: 'CN' },
] as const;

export const LanguageSelector = memo(function LanguageSelector() {
  const { i18n } = useTranslation();
  const currentLang = i18n.language;

  const handleChange = useCallback(
    (code: string) => {
      i18n.changeLanguage(code);
      // Persist preference
      localStorage.setItem('4da_language', code);
    },
    [i18n],
  );

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-1">Language</h3>
      <p className="text-xs text-text-muted mb-3">
        {LANGUAGES.length} languages supported. Changes apply immediately.
      </p>
      <div className="grid grid-cols-2 gap-1.5">
        {LANGUAGES.map((lang) => (
          <button
            key={lang.code}
            onClick={() => handleChange(lang.code)}
            className={`flex items-center gap-2 px-3 py-2 rounded-lg text-left transition-all ${
              currentLang === lang.code
                ? 'bg-white/10 border border-white/20 text-white'
                : 'border border-transparent text-text-secondary hover:bg-white/5 hover:text-white'
            }`}
          >
            <span className="text-xs font-medium w-16 shrink-0">{lang.native}</span>
            <span className="text-[10px] text-text-muted">{lang.name}</span>
            {currentLang === lang.code && (
              <span className="ml-auto w-1.5 h-1.5 rounded-full bg-[#22C55E]" />
            )}
          </button>
        ))}
      </div>
    </div>
  );
});
