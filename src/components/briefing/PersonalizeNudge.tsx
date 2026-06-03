// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

interface PersonalizeNudgeProps {
  onScanProjects: () => void;
  onOpenSettings: () => void;
  onDismiss: () => void;
  isScanning: boolean;
}

/**
 * First-run personalization nudge.
 *
 * Shown inline when a user finished onboarding with no interests configured —
 * typically because they skipped the recommended project scan. It offers the same
 * one-click, fully-local "Scan my projects" action as the onboarding choice gate
 * (reuses `ace_auto_discover` via the store's `runAutoDiscovery`), so a skipper can
 * recover from keyword-only mode without hunting through Settings. Explicit click is
 * the consent (INV-004); the card stays dismissible, and Settings remains a secondary
 * path for manual directory configuration.
 */
export const PersonalizeNudge = memo(function PersonalizeNudge({
  onScanProjects,
  onOpenSettings,
  onDismiss,
  isScanning,
}: PersonalizeNudgeProps) {
  const { t } = useTranslation();

  return (
    <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-4 flex items-start justify-between gap-3">
      <div className="flex-1 min-w-0">
        <h3 className="text-sm font-medium text-white mb-1">{t('briefing.personalizeTitle')}</h3>
        <p className="text-xs text-text-secondary mb-3">
          {t(
            'onboarding.choice.scanProjectsDesc',
            '100% local — nothing ever leaves your machine. Personalizes 4DA to your real stack in about a minute.',
          )}
        </p>
        {isScanning ? (
          <div
            className="flex items-center gap-2 text-xs text-text-secondary"
            role="status"
            aria-live="polite"
          >
            <span
              className="w-4 h-4 border-2 border-blue-400 border-t-transparent rounded-full animate-spin"
              aria-hidden="true"
            />
            {t('onboarding.choice.scanning', 'Scanning your projects… this stays on your device')}
          </div>
        ) : (
          <div className="flex items-center gap-3 flex-wrap">
            <button
              onClick={onScanProjects}
              className="px-3 py-1.5 text-xs bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-lg hover:bg-blue-500/30 transition-all font-medium"
            >
              {t('onboarding.choice.scanProjects', 'Scan my projects')}
            </button>
            <button
              onClick={onOpenSettings}
              className="text-xs text-text-muted hover:text-text-secondary transition-colors"
            >
              {t('header.settings')}
            </button>
          </div>
        )}
      </div>
      {/* eslint-disable i18next/no-literal-string */}
      <button
        onClick={onDismiss}
        disabled={isScanning}
        className="text-text-muted hover:text-white transition-colors flex-shrink-0 p-1 disabled:opacity-30"
        aria-label={t('action.dismiss')}
      >
        &#x2715;
      </button>
      {/* eslint-enable i18next/no-literal-string */}
    </div>
  );
});
