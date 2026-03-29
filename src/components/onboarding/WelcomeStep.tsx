import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import sunLogo from '../../assets/sun-logo.webp';

interface WelcomeStepProps {
  isAnimating: boolean;
  onNext: () => void;
  onSkip?: () => void;
}

export function WelcomeStep({ isAnimating, onNext, onSkip }: WelcomeStepProps) {
  const { t } = useTranslation();

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
