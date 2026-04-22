// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { lazy, Suspense, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { ViewErrorBoundary } from './ViewErrorBoundary';
import { ResultsView } from './ResultsView';

const BriefingView = lazy(() => import('./BriefingView').then(m => ({ default: m.BriefingView })));
const PlaybookView = lazy(() => import('./PlaybookView').then(m => ({ default: m.PlaybookView })));
const SignalsPanel = lazy(() => import('./SignalsPanel').then(m => ({ default: m.SignalsPanel })));
const KnowledgeGapsPanel = lazy(() => import('./KnowledgeGapsPanel').then(m => ({ default: m.KnowledgeGapsPanel })));
const WhatYouWouldHaveMissed = lazy(() => import('./WhatYouWouldHaveMissed').then(m => ({ default: m.WhatYouWouldHaveMissed })));
const PreemptionView = lazy(() => import('./preemption/PreemptionView'));
const BlindSpotsView = lazy(() => import('./blindspots/BlindSpotsView'));

const VIEW_LABEL_KEYS: Record<string, string> = {
  briefing: 'nav.briefing.label',
  preemption: 'nav.preemption.label',
  blindspots: 'nav.blindspots.label',
  results: 'nav.signal.label',
  playbook: 'nav.playbook',
};

interface ViewRouterProps {
  newItemIds: Set<number>;
  focusedIndex: number;
}

export function ViewRouter({ newItemIds, focusedIndex }: ViewRouterProps) {
  const { t } = useTranslation();
  const { activeView, analysisComplete, relevanceResults } = useAppStore(
    useShallow(s => ({
      activeView: s.activeView,
      analysisComplete: s.appState.analysisComplete,
      relevanceResults: s.appState.relevanceResults,
    })),
  );

  const [viewAnnouncement, setViewAnnouncement] = useState('');
  useEffect(() => {
    const labelKey = VIEW_LABEL_KEYS[activeView];
    if (labelKey !== undefined && labelKey !== '') {
      setViewAnnouncement(t('app.viewChanged', { view: t(labelKey) }));
    }
  }, [activeView, t]);

  return (
    <>
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
      ) : activeView === 'playbook' ? (
        <ViewErrorBoundary viewName="Playbook">
          <PlaybookView />
        </ViewErrorBoundary>
      ) : (
        <ViewErrorBoundary viewName="Signal">
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
