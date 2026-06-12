// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface OnboardingChoiceGateProps {
  isAnimating: boolean;
  hasProviderConfigured: boolean;
  onStartUsing: () => void;
  onContinueSetup: () => void;
  onScanProjects: () => void | Promise<void>;
}

export function OnboardingChoiceGate({
  isAnimating,
  hasProviderConfigured,
  onStartUsing,
  onContinueSetup,
  onScanProjects,
}: OnboardingChoiceGateProps) {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);

  const handleScan = () => {
    if (scanning) return;
    setScanning(true);
    void Promise.resolve(onScanProjects()).catch(() => {
      // The scan handler proceeds on error; reset state defensively.
      setScanning(false);
    });
  };

  return (
    <div
      className={`text-center transition-all duration-500 ${
        isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'
      }`}
    >
      <div className="text-4xl mb-4" aria-hidden="true">&#x2728;</div>
      <h2 className="text-2xl font-semibold text-text-primary mb-2">
        {t('onboarding.choice.title', 'You\'re ready to go')}
      </h2>
      <p className="text-text-secondary text-sm max-w-md mx-auto mb-6">
        {t(
          'onboarding.choice.description',
          '4DA is already learning about your projects in the background. You can start exploring now or fine-tune your setup first.',
        )}
      </p>

      {/* Provider status — shown ONLY once a provider is actually configured.
          A bare "AI Provider" pill on a path-choice screen read as a stray heading
          (configuring a provider is one of the offered paths below), so we suppress
          it in the common not-yet-configured case and only surface the affirmative
          green "configured" confirmation when it's true. */}
      {hasProviderConfigured && (
        <div className="flex items-center justify-center gap-2 mb-6">
          <span
            className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-green-500/10 text-green-400"
            role="status"
          >
            <svg className="w-3.5 h-3.5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0Zm3.78 5.22a.75.75 0 0 0-1.06 0L7 8.94 5.28 7.22a.75.75 0 0 0-1.06 1.06l2.25 2.25a.75.75 0 0 0 1.06 0l4.25-4.25a.75.75 0 0 0 0-1.06Z" />
            </svg>
            {t('onboarding.choice.providerReady', 'AI Provider configured')}
          </span>
        </div>
      )}

      {scanning ? (
        /* Inline scanning state — everything stays on the user's device */
        <div className="flex flex-col items-center gap-3 max-w-sm mx-auto py-4" role="status" aria-live="polite">
          <div className="w-6 h-6 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" aria-hidden="true" />
          <p className="text-sm text-text-secondary">
            {t('onboarding.choice.scanning', 'Scanning your projects… this stays on your device')}
          </p>
        </div>
      ) : (
        <div className="flex flex-col items-center gap-4 max-w-sm mx-auto">
          {/* Primary, recommended path: a fully-local project scan */}
          <button
            onClick={handleScan}
            disabled={scanning}
            className="w-full px-8 py-4 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-semibold text-lg hover:scale-[1.02] active:scale-[0.98] disabled:opacity-50"
          >
            <span className="inline-flex items-center justify-center gap-2">
              {t('onboarding.choice.scanProjects', 'Scan my projects')}
              <span className="text-[10px] px-1.5 py-0.5 bg-text-primary/20 text-text-primary rounded font-medium uppercase tracking-wide">
                {t('onboarding.choice.scanRecommended', 'Recommended')}
              </span>
            </span>
          </button>
          <p className="text-xs text-orange-400/80 -mt-2">
            {t(
              'onboarding.choice.scanProjectsDesc',
              '100% local — nothing ever leaves your machine. Personalizes 4DA to your real stack.',
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

          {/* Secondary path: manual full setup */}
          <button
            onClick={onContinueSetup}
            disabled={scanning}
            className="w-full px-8 py-2.5 bg-bg-secondary text-text-secondary rounded-lg border border-border hover:border-[#3A3A3A] transition-all text-sm disabled:opacity-50"
          >
            {t('onboarding.choice.continueSetup', 'Continue full setup')}
          </button>
          <p className="text-xs text-text-muted -mt-2">
            {t(
              'onboarding.choice.continueHint',
              'Configure AI provider, stack, interests',
            )}
          </p>

          {/* Tertiary, muted path: keyword matching only */}
          <button
            onClick={onStartUsing}
            disabled={scanning}
            className="w-full px-8 py-2 bg-transparent text-text-muted rounded-lg border border-transparent hover:text-text-secondary transition-all text-sm disabled:opacity-50"
          >
            {t('onboarding.choice.keywordOnly', 'Not now — keyword matching only')}
          </button>
          <p className="text-xs text-text-muted/60 -mt-2">
            {t(
              'onboarding.choice.keywordOnlyHint',
              'You can scan or add a provider anytime in Settings.',
            )}
          </p>
        </div>
      )}

      <p className="text-[10px] text-text-muted mt-6" aria-live="polite">
        {t('onboarding.keyboardHint', 'Pro tip: Press R to analyze, / to search, ? for all shortcuts')}
      </p>
    </div>
  );
}
