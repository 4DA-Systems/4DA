/**
 * PlaybookLanguageSelector — language switcher for STREETS lesson content.
 *
 * Shows the current lesson language, indicates which modules have translations,
 * and lets users trigger on-demand translation for individual modules.
 */

import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';

const LANGUAGES: { code: string; name: string; native: string }[] = [
  { code: 'en', name: 'English', native: 'English' },
  { code: 'ar', name: 'Arabic', native: 'العربية' },
  { code: 'de', name: 'German', native: 'Deutsch' },
  { code: 'es', name: 'Spanish', native: 'Español' },
  { code: 'fr', name: 'French', native: 'Français' },
  { code: 'hi', name: 'Hindi', native: 'हिन्दी' },
  { code: 'it', name: 'Italian', native: 'Italiano' },
  { code: 'ja', name: 'Japanese', native: '日本語' },
  { code: 'ko', name: 'Korean', native: '한국어' },
  { code: 'pt-BR', name: 'Portuguese', native: 'Português' },
  { code: 'ru', name: 'Russian', native: 'Русский' },
  { code: 'tr', name: 'Turkish', native: 'Türkçe' },
  { code: 'zh', name: 'Chinese', native: '中文' },
];

interface PlaybookLanguageSelectorProps {
  activeModuleId: string | null;
  onLanguageChange?: () => void;
}

export const PlaybookLanguageSelector = memo(function PlaybookLanguageSelector({
  activeModuleId,
  onLanguageChange,
}: PlaybookLanguageSelectorProps) {
  const { t, i18n } = useTranslation();
  const currentLang = i18n.language;
  const [translationStatus, setTranslationStatus] = useState<Record<string, boolean>>({});
  const [isTranslating, setIsTranslating] = useState(false);
  const [showPicker, setShowPicker] = useState(false);

  // Check translation status for current language
  useEffect(() => {
    if (currentLang === 'en') return;
    cmd('get_lesson_translation_status', { lang: currentLang })
      .then(setTranslationStatus)
      .catch(() => {});
  }, [currentLang]);

  const currentLangInfo = LANGUAGES.find((l) => l.code === currentLang) ?? LANGUAGES[0]!;
  const hasTranslation = activeModuleId ? translationStatus[activeModuleId] : false;

  const handleTranslateModule = useCallback(async () => {
    if (!activeModuleId || currentLang === 'en') return;
    setIsTranslating(true);
    try {
      await cmd('translate_playbook_module', { moduleId: activeModuleId, lang: currentLang });
      // Refresh status
      const status = await cmd('get_lesson_translation_status', { lang: currentLang });
      setTranslationStatus(status);
      onLanguageChange?.();
    } catch {
      // Translation failed — user can retry
    } finally {
      setIsTranslating(false);
    }
  }, [activeModuleId, currentLang, onLanguageChange]);

  // Don't show if user language is English
  if (currentLang === 'en') return null;

  return (
    <div className="relative">
      <button
        onClick={() => setShowPicker((p) => !p)}
        className="flex items-center gap-2 px-3 py-1.5 text-xs bg-bg-tertiary border border-border rounded-lg hover:border-white/20 transition-colors w-full"
        aria-label={t('streets:streets.language')}
      >
        <span className="text-text-secondary">{currentLangInfo.native}</span>
        {activeModuleId && !hasTranslation && currentLang !== 'en' && (
          <span className="ms-auto text-[9px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
            EN
          </span>
        )}
        {activeModuleId && hasTranslation && (
          <span className="ms-auto text-[9px] px-1.5 py-0.5 rounded bg-green-500/10 text-green-400 border border-green-500/20">
            ✓
          </span>
        )}
      </button>

      {/* Translation action for active module */}
      {activeModuleId && !hasTranslation && currentLang !== 'en' && (
        <button
          onClick={handleTranslateModule}
          disabled={isTranslating}
          className="mt-1.5 w-full px-3 py-1.5 text-[10px] text-accent-gold border border-accent-gold/20 rounded-lg hover:bg-accent-gold/10 transition-colors disabled:opacity-50 disabled:cursor-wait"
        >
          {isTranslating
            ? t('streets:streets.translating')
            : t('streets:streets.translateModule', { lang: currentLangInfo.native })}
        </button>
      )}

      {/* Language picker dropdown */}
      {showPicker && (
        <div className="absolute top-full mt-1 start-0 w-full bg-bg-secondary border border-border rounded-lg shadow-xl z-50 max-h-64 overflow-y-auto">
          {LANGUAGES.map((lang) => (
            <button
              key={lang.code}
              onClick={async () => {
                i18n.changeLanguage(lang.code);
                localStorage.setItem('4da_language', lang.code);
                await cmd('set_locale', { country: '', language: lang.code, currency: '' }).catch(() => {});
                setShowPicker(false);
                useAppStore.getState().reloadForLanguage();
                onLanguageChange?.();
              }}
              className={`w-full text-start px-3 py-2 text-xs hover:bg-bg-tertiary transition-colors flex items-center justify-between ${
                lang.code === currentLang ? 'text-accent-gold' : 'text-text-secondary'
              }`}
            >
              <span>{lang.native}</span>
              {lang.code === currentLang && (
                <span className="text-accent-gold">●</span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
});
