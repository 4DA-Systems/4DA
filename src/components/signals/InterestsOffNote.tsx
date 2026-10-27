// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// What Signal shows below "Your stack" while every interest is off (AD-054:
// reading is opt-in). It used to be a full "Analysis Results" panel — search,
// source chips, six sorts, "53 relevant items from 278 analyzed" — over an
// empty list, because every row it held was a stack fact the lane above
// already states. One line says what is off and where to turn it on.

import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

export function InterestsOffNote() {
  const { t } = useTranslation();
  const setShowSettings = useAppStore((s) => s.setShowSettings);
  const setSettingsInitialTab = useAppStore((s) => s.setSettingsInitialTab);

  const openSources = useCallback(() => {
    setSettingsInitialTab('sources');
    setShowSettings(true);
  }, [setSettingsInitialTab, setShowSettings]);

  return (
    <div
      data-testid="interests-off-note"
      className="mx-4 mb-4 px-5 py-3 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg border border-border bg-bg-secondary"
    >
      <p className="flex-1 min-w-[12rem] text-xs text-text-muted">{t('signals.interestsOff')}</p>
      <button
        type="button"
        onClick={openSources}
        className="px-3 py-1.5 text-xs font-medium rounded-lg border border-border bg-bg-tertiary text-text-secondary hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-gold/50 transition-colors"
      >
        {t('signals.interestsOffAction')}
      </button>
    </div>
  );
}
