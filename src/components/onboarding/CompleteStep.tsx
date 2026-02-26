import { useTranslation } from 'react-i18next';

interface CompleteStepProps {
  isAnimating: boolean;
  onComplete: () => void;
}

export function CompleteStep({ isAnimating, onComplete }: CompleteStepProps) {
  const { t } = useTranslation();

  return (
    <div className={`text-center transition-all duration-500 ${isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}>
      <div className="w-24 h-24 mx-auto mb-6 bg-gradient-to-br from-green-500/30 to-green-600/20 rounded-full flex items-center justify-center ring-4 ring-green-500/20">
        <span className="text-5xl animate-bounce">&#x1f389;</span>
      </div>
      <h2 className="text-3xl font-semibold text-white mb-3">{t('onboarding.complete.title')}</h2>
      <p className="text-gray-400 mb-8 max-w-md mx-auto">
        {t('onboarding.complete.subtitle')}
      </p>

      <div className="bg-bg-secondary p-5 rounded-lg mb-8 text-left max-w-md mx-auto">
        <h3 className="text-white font-medium mb-4 text-center">{t('onboarding.complete.keepImproving')}</h3>
        <ul className="space-y-4">
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-7 h-7 bg-green-500/20 rounded-full flex items-center justify-center text-sm text-green-400">
              &#x2713;
            </span>
            <div>
              <strong className="text-white block text-sm">{t('onboarding.complete.saveTitle')}</strong>
              <span className="text-gray-400 text-sm">{t('onboarding.complete.saveDesc')}</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-7 h-7 bg-red-500/20 rounded-full flex items-center justify-center text-sm text-red-400">
              &#x2715;
            </span>
            <div>
              <strong className="text-white block text-sm">{t('onboarding.complete.dismissTitle')}</strong>
              <span className="text-gray-400 text-sm">{t('onboarding.complete.dismissDesc')}</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-7 h-7 bg-orange-500/20 rounded-full flex items-center justify-center text-sm text-orange-400">
              &#x26a1;
            </span>
            <div>
              <strong className="text-white block text-sm">{t('onboarding.complete.letItRunTitle')}</strong>
              <span className="text-gray-400 text-sm">{t('onboarding.complete.letItRunDesc')}</span>
            </div>
          </li>
        </ul>
      </div>

      <button
        onClick={onComplete}
        aria-label={t('onboarding.complete.startUsing')}
        className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
      >
        {t('onboarding.complete.startUsing')} &rarr;
      </button>

      <p className="text-xs text-gray-600 mt-4">
        {t('onboarding.complete.settingsHint')}
      </p>
    </div>
  );
}
