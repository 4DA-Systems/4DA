// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

interface PersonalizeNudgeProps {
  onOpenSettings: () => void;
  onDismiss: () => void;
}

/**
 * First-run personalization nudge card.
 * Shown inline when user has no interests configured.
 */
export const PersonalizeNudge = memo(function PersonalizeNudge({
  onOpenSettings,
  onDismiss,
}: PersonalizeNudgeProps) {
  const { t } = useTranslation();

  return (
    <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-4 flex items-start justify-between gap-3">
      <div>
        <h3 className="text-sm font-medium text-white mb-1">{t('briefing.personalizeTitle')}</h3>
        <p className="text-xs text-text-secondary mb-3">{t('briefing.personalizeBody')}</p>
        <button
          onClick={onOpenSettings}
          className="px-3 py-1.5 text-xs bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-lg hover:bg-blue-500/30 transition-all font-medium"
        >
          {t('header.settings')}
        </button>
      </div>
      <button
        onClick={onDismiss}
        className="text-text-muted hover:text-white transition-colors flex-shrink-0 p-1"
        aria-label={t('action.dismiss')}
      >
        &#x2715;
      </button>
    </div>
  );
});
