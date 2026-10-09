// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import { useAppStore } from '../../store';

/**
 * Day-one Blind Spots states (fresh-profile E2E 2026-10-09). Before this the
 * tab gave a new user nothing to act on: with no project scanned it was a
 * heading over a blank page, and after a scan a lone "94 direct dependencies"
 * bar — a count that informs no action (doctrine rule 3) — over nothing.
 * Neither state is a "no data yet, check back" panel (rule 6): each names the
 * one thing the user can do now.
 */

/**
 * No dependencies tracked: Blind Spots works from the user's own manifests,
 * so the only useful thing to say is "scan your projects". Same fully-local,
 * click-is-consent `ace_auto_discover` path as the Briefing's personalize
 * nudge (`runAutoDiscovery`, INV-004), with Settings > Projects as the manual
 * route.
 */
export const ScanProjectsPrompt = memo(function ScanProjectsPrompt({ onScanned }: { onScanned: () => void }) {
  const { t } = useTranslation();
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const isScanning = useAppStore(s => s.isScanning);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const setSettingsInitialTab = useAppStore(s => s.setSettingsInitialTab);

  const handleScan = useCallback(async () => {
    await runAutoDiscovery();
    await loadUserContext();
    onScanned();
  }, [runAutoDiscovery, loadUserContext, onScanned]);

  const handleChooseFolders = useCallback(() => {
    setSettingsInitialTab('projects');
    setShowSettings(true);
  }, [setSettingsInitialTab, setShowSettings]);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border px-5 py-5" data-testid="blindspots-scan-prompt">
      <h3 className="text-sm font-medium text-text-primary">{t('blindspots.firstDay.scanTitle')}</h3>
      <p className="text-xs text-text-muted mt-1 mb-3">{t('blindspots.firstDay.scanBody')}</p>
      {isScanning ? (
        <div className="flex items-center gap-2 text-xs text-text-secondary" role="status" aria-live="polite">
          <span className="w-4 h-4 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" aria-hidden="true" />
          {t('onboarding.choice.scanning')}
        </div>
      ) : (
        <div className="flex items-center gap-3 flex-wrap">
          <button
            onClick={() => { void handleScan(); }}
            className="px-3 py-1.5 text-xs bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-lg hover:bg-blue-500/30 transition-all font-medium"
          >
            {t('onboarding.choice.scanProjects')}
          </button>
          <button
            onClick={handleChooseFolders}
            className="text-xs text-text-muted hover:text-text-secondary transition-colors"
          >
            {t('blindspots.firstDay.chooseFolders')}
          </button>
        </div>
      )}
    </div>
  );
});

/**
 * Dependencies are tracked but the report is not computed yet (`score < 0`:
 * under a week of reading, or too few direct dependencies). Say what will
 * appear and where the actionable part lives today — advisories that affect
 * the user reach Preemption immediately — instead of a bare dependency count.
 */
export const AssessingNotice = memo(function AssessingNotice() {
  const { t } = useTranslation();
  const setActiveView = useAppStore(s => s.setActiveView);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border px-5 py-5" data-testid="blindspots-assessing">
      <h3 className="text-sm font-medium text-text-primary">{t('blindspots.firstDay.assessingTitle')}</h3>
      <p className="text-xs text-text-muted mt-1 mb-3">{t('blindspots.firstDay.assessingBody')}</p>
      <button
        onClick={() => setActiveView('preemption')}
        className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-text-primary transition-colors"
      >
        {t('blindspots.firstDay.openPreemption')}
      </button>
    </div>
  );
});
