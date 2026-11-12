// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import type { ScanOutcome } from './scan-outcome';

/**
 * What the onboarding scan found. Found: the counts and a Continue button.
 * Nothing or an error: guidance shown above the folder picker, so the user
 * can tick another folder and scan again, or leave by the other exits.
 */
export function ScanOutcomePanel({
  outcome,
  busy,
  onContinue,
}: {
  outcome: ScanOutcome;
  busy: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation();

  if (outcome.kind === 'found') {
    return (
      <div className="flex flex-col items-center gap-4 max-w-sm mx-auto" role="status" data-testid="scan-outcome-found">
        <p className="text-sm text-text-primary">
          {t('onboarding.scanResult.found', { projects: outcome.projects, dependencies: outcome.dependencies })}
        </p>
        {outcome.ecosystems.length > 0 && (
          <ul className="flex flex-wrap justify-center gap-2" aria-label={t('onboarding.scanResult.byEcosystem')}>
            {outcome.ecosystems.map(e => (
              <li key={e.ecosystem} className="px-2 py-0.5 text-xs rounded bg-bg-secondary border border-border text-text-secondary">
                {t(`onboarding.scanResult.ecosystem.${e.ecosystem}`)} {e.count}
              </li>
            ))}
          </ul>
        )}
        <p className="text-xs text-text-muted">{t('onboarding.scanResult.foundNext')}</p>
        <button
          onClick={onContinue}
          disabled={busy}
          className="w-full px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-semibold disabled:opacity-50"
        >
          {t('onboarding.scanResult.continue')}
        </button>
      </div>
    );
  }

  return (
    <div role="alert" data-testid={`scan-outcome-${outcome.kind}`} className="w-full p-3 rounded-lg border border-amber-500/30 bg-amber-500/10 text-start">
      <p className="text-sm text-amber-400">
        {outcome.kind === 'none'
          ? t('onboarding.scanResult.none')
          : t('onboarding.scanResult.error', { error: outcome.message || t('onboarding.scanResult.errorUnknown') })}
      </p>
      <p className="text-xs text-text-muted mt-1">{t('onboarding.scanResult.noneHint')}</p>
    </div>
  );
}
