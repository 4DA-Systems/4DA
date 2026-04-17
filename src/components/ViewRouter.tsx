// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { lazy, Suspense, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { ViewErrorBoundary } from './ViewErrorBoundary';
import { ResultsView } from './ResultsView';

// Lazy-loaded views — each only loads when navigated to
const BriefingView = lazy(() => import('./BriefingView').then(m => ({ default: m.BriefingView })));
const SavedItemsView = lazy(() => import('./SavedItemsView').then(m => ({ default: m.SavedItemsView })));
const SovereignDeveloperProfile = lazy(() => import('./SovereignDeveloperProfile').then(m => ({ default: m.SovereignDeveloperProfile })));
const ToolkitView = lazy(() => import('./toolkit/ToolkitView').then(m => ({ default: m.ToolkitView })));
const PlaybookView = lazy(() => import('./PlaybookView').then(m => ({ default: m.PlaybookView })));
const CalibrationView = lazy(() => import('./CalibrationView').then(m => ({ default: m.CalibrationView })));
const SignalsPanel = lazy(() => import('./SignalsPanel').then(m => ({ default: m.SignalsPanel })));
const KnowledgeGapsPanel = lazy(() => import('./KnowledgeGapsPanel').then(m => ({ default: m.KnowledgeGapsPanel })));
const WhatYouWouldHaveMissed = lazy(() => import('./WhatYouWouldHaveMissed').then(m => ({ default: m.WhatYouWouldHaveMissed })));
const IntelligenceConsole = lazy(() => import('./IntelligenceConsole'));
const PreemptionView = lazy(() => import('./preemption/PreemptionView'));
const BlindSpotsView = lazy(() => import('./blindspots/BlindSpotsView'));
const EvidenceView = lazy(() => import('./evidence/EvidenceView'));

interface ViewRouterProps {
  newItemIds: Set<number>;
  focusedIndex: number;
}

const VIEW_LABEL_KEYS: Record<string, string> = {
  briefing: 'nav.briefing.label',
  preemption: 'nav.preemption.label',
  blindspots: 'nav.blindspots.label',
  evidence: 'nav.evidence',
  results: 'nav.results',
  profile: 'nav.profile',
  saved: 'nav.saved',
  toolkit: 'nav.toolkit',
  playbook: 'nav.playbook',
  calibrate: 'nav.calibrate',
  console: 'nav.console',
};

export function ViewRouter({ newItemIds, focusedIndex }: ViewRouterProps) {
  const { t } = useTranslation();
  const { activeView, analysisComplete, relevanceResults } = useAppStore(
    useShallow(s => ({
      activeView: s.activeView,
      analysisComplete: s.appState.analysisComplete,
      relevanceResults: s.appState.relevanceResults,
    })),
  );

  // Screen reader announcement for view changes
  const [viewAnnouncement, setViewAnnouncement] = useState('');
  useEffect(() => {
    const labelKey = VIEW_LABEL_KEYS[activeView];
    if (labelKey !== undefined && labelKey !== '') {
      setViewAnnouncement(t('app.viewChanged', { view: t(labelKey) }));
    }
  }, [activeView, t]);

  return (
    <>
    {/* Screen reader announcement for view changes */}
    <div className="sr-only" aria-live="polite" aria-atomic="true" role="status">
      {viewAnnouncement}
    </div>
    <Suspense fallback={<div className="flex items-center justify-center py-20 text-text-secondary text-sm">{t('action.loading')}</div>}>
      {activeView === 'briefing' ? (
        <ViewErrorBoundary viewName="Briefing">
          <BriefingView />
        </ViewErrorBoundary>
      ) : activeView === 'preemption' ? (
        <ViewErrorBoundary viewName="Preemption">
          <PreemptionView />
        </ViewErrorBoundary>
      ) : activeView === 'blindspots' ? (
        <ViewErrorBoundary viewName="BlindSpots">
          <BlindSpotsView />
        </ViewErrorBoundary>
      ) : activeView === 'evidence' ? (
        <ViewErrorBoundary viewName="Evidence">
          <EvidenceView />
        </ViewErrorBoundary>
      ) : activeView === 'profile' ? (
        <ViewErrorBoundary viewName="Profile">
          <SovereignDeveloperProfile />
        </ViewErrorBoundary>
      ) : activeView === 'saved' ? (
        <ViewErrorBoundary viewName="Saved">
          <SavedItemsView />
        </ViewErrorBoundary>
      ) : activeView === 'toolkit' ? (
        <ViewErrorBoundary viewName="Toolkit">
          <ToolkitView />
        </ViewErrorBoundary>
      ) : activeView === 'playbook' ? (
        <ViewErrorBoundary viewName="Playbook">
          <PlaybookView />
        </ViewErrorBoundary>
      ) : activeView === 'console' ? (
        <ViewErrorBoundary viewName="Console">
          <IntelligenceConsole />
        </ViewErrorBoundary>
      ) : activeView === 'calibrate' ? (
        <ViewErrorBoundary viewName="System">
          <CalibrationView />
        </ViewErrorBoundary>
      ) : (
        <ViewErrorBoundary viewName="Results">
          {analysisComplete && (
            <Suspense fallback={null}>
              <WhatYouWouldHaveMissed />
              <SignalsPanel results={relevanceResults} />
              <KnowledgeGapsPanel />
            </Suspense>
          )}
          <ResultsView
            newItemIds={newItemIds}
            focusedIndex={focusedIndex}
          />
        </ViewErrorBoundary>
      )}
    </Suspense>
    </>
  );
}
