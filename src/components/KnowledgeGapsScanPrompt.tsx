// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

/**
 * What Learning Opportunities says when no dependency is known: nothing was
 * checked, so it names the action that fixes that instead of claiming "your
 * knowledge is current" (fresh-profile E2E 2026-10-09, doctrine rule 6). The
 * scan is the same one-click, fully-local `ace_auto_discover` the Brief's
 * personalize card runs; the click is the consent (INV-004).
 */
export const KnowledgeGapsScanPrompt = memo(function KnowledgeGapsScanPrompt() {
  const { t } = useTranslation();
  const isScanning = useAppStore((s) => s.isScanning);
  const runAutoDiscovery = useAppStore((s) => s.runAutoDiscovery);
  const loadUserContext = useAppStore((s) => s.loadUserContext);
  const startAnalysis = useAppStore((s) => s.startAnalysis);

  const scan = useCallback(async () => {
    await runAutoDiscovery();
    await loadUserContext();
    void startAnalysis();
  }, [runAutoDiscovery, loadUserContext, startAnalysis]);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border px-5 py-4" data-testid="knowledge-gaps-scan-prompt">
      <h3 className="font-medium text-text-primary text-sm">{t('knowledgeGaps.title')}</h3>
      <p className="text-xs text-text-muted mt-0.5 mb-3">{t('knowledgeGaps.noDependencies')}</p>
      <button
        type="button"
        onClick={() => void scan()}
        disabled={isScanning}
        className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg hover:border-text-muted transition-colors font-medium disabled:opacity-50"
      >
        {isScanning ? t('onboarding.choice.scanning') : t('onboarding.choice.scanProjects')}
      </button>
    </div>
  );
});
