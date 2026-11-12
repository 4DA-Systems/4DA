// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { DiscoveryFolderPicker } from './DiscoveryFolderPicker';
import { ScanOutcomePanel } from './ScanOutcomePanel';
import type { ScanOutcome } from './scan-outcome';
import { useDiscoveryFolders } from './use-discovery-folders';

interface OnboardingChoiceGateProps {
  isAnimating: boolean;
  hasProviderConfigured: boolean;
  /** Finishing onboarding is in flight; every exit is disabled meanwhile. */
  busy?: boolean;
  onStartUsing: () => void;
  onContinueSetup: () => void;
  /** Scan exactly these folders (the ones the user left ticked). */
  onScanProjects: (dirs: string[]) => Promise<ScanOutcome>;
  /** Finish onboarding after a scan that found projects. */
  onFinish: () => void;
}

export function OnboardingChoiceGate({
  isAnimating,
  hasProviderConfigured,
  busy = false,
  onStartUsing,
  onContinueSetup,
  onScanProjects,
  onFinish,
}: OnboardingChoiceGateProps) {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);
  const [outcome, setOutcome] = useState<ScanOutcome | null>(null);
  const discovery = useDiscoveryFolders();
  const nothingTicked = discovery.selected.length === 0;

  const handleScan = () => {
    if (scanning || busy || nothingTicked) return;
    setScanning(true);
    setOutcome(null);
    void onScanProjects(discovery.selected)
      .then(setOutcome)
      .catch((e: unknown) => setOutcome({ kind: 'error', message: String(e) }))
      .finally(() => setScanning(false));
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
          'Choose how 4DA learns your stack: scan your project folders (stays on this device), set it up by hand, or start with defaults.',
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
      ) : outcome?.kind === 'found' ? (
        <ScanOutcomePanel outcome={outcome} busy={busy} onContinue={onFinish} />
      ) : (
        <div className="flex flex-col items-center gap-4 max-w-sm mx-auto">
          {outcome && <ScanOutcomePanel outcome={outcome} busy={busy} onContinue={onFinish} />}
          {/* Primary, recommended path: a fully-local project scan */}
          <button
            onClick={handleScan}
            disabled={scanning || busy || nothingTicked}
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
            {t('onboarding.choice.scanProjectsDesc')}
          </p>
          <div className="w-full p-3 bg-bg-secondary rounded-lg border border-border">
            <DiscoveryFolderPicker discovery={discovery} disabled={busy} />
          </div>

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
            disabled={scanning || busy}
            className="w-full px-8 py-2.5 bg-bg-secondary text-text-secondary rounded-lg border border-border hover:border-text-muted transition-all text-sm disabled:opacity-50"
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
            disabled={scanning || busy}
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
