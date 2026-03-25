import { useTranslation } from 'react-i18next';

interface OnboardingChoiceGateProps {
  isAnimating: boolean;
  onStartUsing: () => void;
  onContinueSetup: () => void;
}

export function OnboardingChoiceGate({
  isAnimating,
  onStartUsing,
  onContinueSetup,
}: OnboardingChoiceGateProps) {
  const { t } = useTranslation();

  return (
    <div
      className={`text-center transition-all duration-500 ${
        isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'
      }`}
    >
      <div className="text-4xl mb-4">&#x2728;</div>
      <h2 className="text-2xl font-semibold text-white mb-2">
        {t('onboarding.choice.title', 'You\'re ready to go')}
      </h2>
      <p className="text-text-secondary text-sm max-w-md mx-auto mb-8">
        {t(
          'onboarding.choice.description',
          '4DA is already learning about your projects in the background. You can start exploring now or fine-tune your setup first.',
        )}
      </p>

      <div className="flex flex-col items-center gap-4 max-w-sm mx-auto">
        {/* Primary: Start using 4DA */}
        <button
          onClick={onStartUsing}
          className="w-full px-8 py-3.5 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-[1.02] active:scale-[0.98]"
        >
          {t('onboarding.choice.startUsing', 'Start using 4DA')}
        </button>
        <p className="text-xs text-text-muted -mt-2">
          {t(
            'onboarding.choice.startHint',
            'Analysis continues in the background',
          )}
        </p>

        {/* Separator */}
        <div className="flex items-center gap-3 w-full my-1">
          <div className="flex-1 h-px bg-border" />
          <span className="text-xs text-text-muted">
            {t('onboarding.choice.or', 'or')}
          </span>
          <div className="flex-1 h-px bg-border" />
        </div>

        {/* Secondary: Continue setup */}
        <button
          onClick={onContinueSetup}
          className="w-full px-8 py-3 bg-bg-secondary text-white rounded-lg border border-border hover:border-[#3A3A3A] transition-all font-medium"
        >
          {t('onboarding.choice.continueSetup', 'Continue setup')}
        </button>
        <p className="text-xs text-text-muted -mt-2">
          {t(
            'onboarding.choice.continueHint',
            'Configure AI provider, stack, interests',
          )}
        </p>
      </div>

      <p className="text-[10px] text-text-muted mt-6">
        {t('onboarding.keyboardHint', 'Pro tip: Press R to analyze, / to search, ? for all shortcuts')}
      </p>
    </div>
  );
}
