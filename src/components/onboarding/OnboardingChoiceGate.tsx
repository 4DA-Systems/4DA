import { useTranslation } from 'react-i18next';

interface OnboardingChoiceGateProps {
  isAnimating: boolean;
  hasProviderConfigured: boolean;
  onStartUsing: () => void;
  onContinueSetup: () => void;
}

export function OnboardingChoiceGate({
  isAnimating,
  hasProviderConfigured,
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
      <div className="text-4xl mb-4" aria-hidden="true">&#x2728;</div>
      <h2 className="text-2xl font-semibold text-white mb-2">
        {t('onboarding.choice.title', 'You\'re ready to go')}
      </h2>
      <p className="text-text-secondary text-sm max-w-md mx-auto mb-6">
        {t(
          'onboarding.choice.description',
          '4DA is already learning about your projects in the background. You can start exploring now or fine-tune your setup first.',
        )}
      </p>

      {/* Configuration status checklist */}
      <div className="flex items-center justify-center gap-2 mb-6">
        <span
          className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium ${
            hasProviderConfigured
              ? 'bg-green-500/10 text-green-400'
              : 'bg-bg-tertiary text-text-muted'
          }`}
          role="status"
          aria-label={
            hasProviderConfigured
              ? t('onboarding.choice.providerReady', 'AI Provider configured')
              : t('onboarding.choice.providerPending', 'AI Provider not configured')
          }
        >
          {hasProviderConfigured ? (
            <svg className="w-3.5 h-3.5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0Zm3.78 5.22a.75.75 0 0 0-1.06 0L7 8.94 5.28 7.22a.75.75 0 0 0-1.06 1.06l2.25 2.25a.75.75 0 0 0 1.06 0l4.25-4.25a.75.75 0 0 0 0-1.06Z" />
            </svg>
          ) : (
            <svg className="w-3.5 h-3.5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0Zm0 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13Z" />
            </svg>
          )}
          {t('onboarding.choice.aiProvider', 'AI Provider')}
        </span>
      </div>

      <div className="flex flex-col items-center gap-4 max-w-sm mx-auto">
        {hasProviderConfigured ? (
          <>
            {/* Provider configured: "Start using 4DA" is primary */}
            <button
              onClick={onStartUsing}
              className="w-full px-8 py-4 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-semibold text-lg hover:scale-[1.02] active:scale-[0.98]"
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
            <div className="flex items-center gap-3 w-full my-1" role="separator">
              <div className="flex-1 h-px bg-border" aria-hidden="true" />
              <span className="text-xs text-text-muted">
                {t('onboarding.choice.or', 'or')}
              </span>
              <div className="flex-1 h-px bg-border" aria-hidden="true" />
            </div>

            <button
              onClick={onContinueSetup}
              className="w-full px-8 py-2.5 bg-bg-secondary text-text-secondary rounded-lg border border-border hover:border-[#3A3A3A] transition-all text-sm"
            >
              {t('onboarding.choice.continueSetup', 'Continue setup')}
            </button>
            <p className="text-xs text-text-muted -mt-2">
              {t(
                'onboarding.choice.continueHint',
                'Configure AI provider, stack, interests',
              )}
            </p>
          </>
        ) : (
          <>
            {/* No provider: "Continue setup" is primary */}
            <button
              onClick={onContinueSetup}
              className="w-full px-8 py-4 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-semibold text-lg hover:scale-[1.02] active:scale-[0.98]"
            >
              {t('onboarding.choice.continueSetup', 'Continue setup')}
            </button>
            <p className="text-xs text-orange-400/80 -mt-2">
              {t(
                'onboarding.choice.recommendProvider',
                'Recommended: configure an AI provider for the best experience',
              )}
            </p>

            {/* Separator */}
            <div className="flex items-center gap-3 w-full my-1" role="separator">
              <div className="flex-1 h-px bg-border" aria-hidden="true" />
              <span className="text-xs text-text-muted">
                {t('onboarding.choice.or', 'or')}
              </span>
              <div className="flex-1 h-px bg-border" aria-hidden="true" />
            </div>

            <button
              onClick={onStartUsing}
              className="w-full px-8 py-2.5 bg-bg-secondary text-text-secondary rounded-lg border border-border hover:border-[#3A3A3A] transition-all text-sm"
            >
              {t('onboarding.choice.startUsing', 'Start using 4DA')}
            </button>
            <p className="text-xs text-text-muted -mt-2">
              {t(
                'onboarding.choice.startHint',
                'Analysis continues in the background',
              )}
            </p>
          </>
        )}
      </div>

      <p className="text-[10px] text-text-muted mt-6" aria-live="polite">
        {t('onboarding.keyboardHint', 'Pro tip: Press R to analyze, / to search, ? for all shortcuts')}
      </p>
    </div>
  );
}
