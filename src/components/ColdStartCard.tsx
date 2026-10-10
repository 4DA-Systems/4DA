// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Cold start with nothing to read (intelligence doctrine rule 6, AD-054).
 *
 * When 4DA has found no project and every interest is off, there is nothing
 * for it to check yet. That is a setup step, not an empty result, so the
 * surface says what 4DA does and offers the two ways in: point it at code,
 * or turn interests on. It never says "no data".
 */
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export function ColdStartCard() {
  const { t } = useTranslation();
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const isScanning = useAppStore(s => s.isScanning);

  const scan = useCallback(async () => {
    await runAutoDiscovery();
    await loadUserContext();
    void startAnalysis();
  }, [runAutoDiscovery, loadUserContext, startAnalysis]);

  return (
    <section
      className="text-center py-12 px-6"
      aria-labelledby="cold-start-title"
      data-testid="cold-start-card"
    >
      <div className="max-w-md mx-auto">
        <h2 id="cold-start-title" className="text-xl font-semibold text-text-primary mb-2">
          {t('coldStart.title')}
        </h2>
        <p className="text-text-secondary text-sm mb-6">{t('coldStart.body')}</p>
        <div className="flex flex-wrap items-center justify-center gap-3">
          <button
            onClick={() => { void scan(); }}
            disabled={isScanning}
            className="px-5 py-2.5 bg-accent-primary text-bg-primary font-medium rounded-lg hover:bg-accent-primary-hover transition-colors disabled:opacity-60"
          >
            {isScanning ? t('coldStart.scanning') : t('coldStart.scan')}
          </button>
          <button
            onClick={() => setShowSettings(true)}
            className="px-5 py-2.5 bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-text-primary transition-colors"
          >
            {t('coldStart.interests')}
          </button>
        </div>
        <p className="text-text-muted text-xs mt-4">{t('coldStart.interestsHint')}</p>
      </div>
    </section>
  );
}
