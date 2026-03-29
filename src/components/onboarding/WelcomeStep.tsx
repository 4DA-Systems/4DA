import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import sunLogo from '../../assets/sun-logo.webp';

/** All supported languages — displayed in native script for instant recognition */
const SUPPORTED_LANGUAGES = [
  { code: 'en', native: 'English' },
  { code: 'es', native: 'Espa\u00f1ol' },
  { code: 'fr', native: 'Fran\u00e7ais' },
  { code: 'de', native: 'Deutsch' },
  { code: 'it', native: 'Italiano' },
  { code: 'pt-BR', native: 'Portugu\u00eas' },
  { code: 'ru', native: '\u0420\u0443\u0441\u0441\u043a\u0438\u0439' },
  { code: 'ja', native: '\u65e5\u672c\u8a9e' },
  { code: 'ko', native: '\ud55c\uad6d\uc5b4' },
  { code: 'zh', native: '\u4e2d\u6587' },
  { code: 'tr', native: 'T\u00fcrk\u00e7e' },
  { code: 'hi', native: '\u0939\u093f\u0928\u094d\u0926\u0940' },
  { code: 'ar', native: '\u0627\u0644\u0639\u0631\u0628\u064a\u0629' },
];

interface WelcomeStepProps {
  isAnimating: boolean;
  onNext: () => void;
  onSkip?: () => void;
}

export function WelcomeStep({ isAnimating, onNext, onSkip }: WelcomeStepProps) {
  const { t, i18n } = useTranslation();
  const [showLangPicker, setShowLangPicker] = useState(false);

  // Enter key advances, Escape skips
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Enter') { e.preventDefault(); onNext(); }
      if (e.key === 'Escape' && onSkip) { e.preventDefault(); onSkip(); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onNext, onSkip]);

  return (
    <div className={`text-center transition-all duration-500 ${isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}>
      {/* Language selector — first thing users see, top-right corner */}
      <div className="absolute top-6 end-6">
        <button
          onClick={() => setShowLangPicker(!showLangPicker)}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-text-secondary bg-bg-secondary border border-border rounded-lg hover:border-orange-500/30 hover:text-white transition-all"
          aria-label="Change language"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
          </svg>
          {SUPPORTED_LANGUAGES.find(l => l.code === i18n.language)?.native ?? 'English'}
        </button>
        {showLangPicker && (
          <div className="absolute end-0 mt-1 w-44 bg-bg-secondary border border-border rounded-lg shadow-xl z-50 max-h-64 overflow-y-auto">
            {SUPPORTED_LANGUAGES.map(lang => (
              <button
                key={lang.code}
                onClick={() => {
                  void i18n.changeLanguage(lang.code);
                  localStorage.setItem('4da_language', lang.code);
                  setShowLangPicker(false);
                }}
                className={`w-full text-start px-3 py-2 text-sm hover:bg-bg-tertiary transition-colors ${
                  i18n.language === lang.code ? 'text-orange-400 font-medium' : 'text-text-secondary'
                }`}
              >
                {lang.native}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="w-32 h-32 mx-auto mb-6 rounded-full overflow-hidden shadow-2xl ring-4 ring-orange-500/20">
        <img src={sunLogo} alt="4DA" className="w-full h-full object-cover" />
      </div>
      <h1 className="text-4xl font-semibold text-white mb-3">{t('onboarding.welcome.title')}</h1>
      <p className="text-xl text-orange-400 mb-2 font-medium">{t('onboarding.welcome.tagline')}</p>
      <p className="text-text-muted mb-8 max-w-md mx-auto">
        {t('onboarding.welcome.description')}
      </p>
      <div className="space-y-3 text-start bg-bg-secondary p-5 rounded-lg mb-8 max-w-md mx-auto">
        <ul className="text-text-secondary space-y-3">
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
              <span className="text-green-400">&#x1f512;</span>
            </span>
            <div>
              <strong className="text-white block">{t('onboarding.welcome.privateTitle')}</strong>
              <span className="text-sm">{t('onboarding.welcome.privateDesc')}</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span className="text-orange-400">&#x26a1;</span>
            </span>
            <div>
              <strong className="text-white block">{t('onboarding.welcome.autonomousTitle')}</strong>
              <span className="text-sm">{t('onboarding.welcome.autonomousDesc')}</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
              <span className="text-blue-400">&#x1f511;</span>
            </span>
            <div>
              <strong className="text-white block">{t('onboarding.welcome.byokTitle')}</strong>
              <span className="text-sm">{t('onboarding.welcome.byokDesc')}</span>
            </div>
          </li>
        </ul>
      </div>
      <button
        onClick={onNext}
        className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
      >
        {t('onboarding.welcome.getStarted')} &rarr;
      </button>
      <p className="text-xs text-text-muted mt-4">{t('onboarding.welcome.quickSetupHint')}</p>
      {onSkip && (
        <div className="mt-6">
          <button
            onClick={onSkip}
            className="text-sm text-text-muted hover:text-white transition-colors"
          >
            {t('onboarding.welcome.skipToContent')}
          </button>
          <p className="text-xs text-text-muted mt-2">{t('onboarding.welcome.skipHint')}</p>
        </div>
      )}
      <p className="text-[10px] text-text-muted mt-6">
        {t('onboarding.keyboardHint', 'Pro tip: Press R to analyze, / to search, ? for all shortcuts')}
      </p>
    </div>
  );
}
