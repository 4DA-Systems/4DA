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
const SignalsPanel = lazy(() => import('./SignalsPanel').then(m => ({ default: m.SignalsPanel })));
const WhatYouWouldHaveMissed = lazy(() => import('./WhatYouWouldHaveMissed').then(m => ({ default: m.WhatYouWouldHaveMissed })));
const FeedbackLivenessBanner = lazy(() => import('./FeedbackLivenessBanner').then(m => ({ default: m.FeedbackLivenessBanner })));
const PreemptionView = lazy(() => import('./preemption/PreemptionView'));
const ContentGraphView = lazy(() => import('./signals/ContentGraphView'));
const ThemeMapView = lazy(() => import('./signals/ThemeMapView'));

/** Signal's sub-views: List (ranked lanes), Themes (the reading map) and
 *  Graph (how items and themes relate). One table, so the toggle buttons can
 *  never drift from the views they switch. */
const SIGNAL_VIEWS = [
  { mode: 'list', labelKey: 'signals.viewList' },
  { mode: 'themes', labelKey: 'signals.viewThemes' },
  { mode: 'graph', labelKey: 'signals.viewGraph' },
] as const;

const VIEW_LABEL_KEYS: Record<string, string> = {
  briefing: 'nav.briefing.label',
  preemption: 'nav.preemption.label',
  results: 'nav.signal.label',
};

interface ViewRouterProps {
  newItemIds: Set<number>;
  focusedIndex: number;
}

export function ViewRouter({ newItemIds, focusedIndex }: ViewRouterProps) {
  const { t } = useTranslation();
  const { activeView, analysisComplete, relevanceResults, signalViewMode, setSignalViewMode } = useAppStore(
    useShallow(s => ({
      activeView: s.activeView,
      analysisComplete: s.appState.analysisComplete,
      relevanceResults: s.appState.relevanceResults,
      signalViewMode: s.signalViewMode,
      setSignalViewMode: s.setSignalViewMode,
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
          <div role="tabpanel" id="view-panel-briefing" aria-labelledby="tab-briefing">
            <BriefingView />
          </div>
        </ViewErrorBoundary>
      ) : activeView === 'preemption' ? (
        <ViewErrorBoundary viewName="Preemption">
          <PreemptionView />
        </ViewErrorBoundary>
      ) : (
        <div role="tabpanel" id="view-panel-results" aria-labelledby="tab-results">
          {/* The toggle lives OUTSIDE the error boundary so it stays clickable
              even if one view throws — the user can always switch away
              instead of being stranded on an error screen. */}
          <div className="flex justify-end px-4 pt-3 pb-1">
            <div className="inline-flex rounded-lg border border-border bg-bg-secondary p-0.5">
              {SIGNAL_VIEWS.map(({ mode, labelKey }) => (
                <button
                  key={mode}
                  onClick={() => setSignalViewMode(mode)}
                  className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
                    signalViewMode === mode
                      ? 'bg-bg-tertiary text-text-primary'
                      : 'text-text-muted hover:text-text-secondary'
                  }`}
                  aria-pressed={signalViewMode === mode}
                >
                  {t(labelKey)}
                </button>
              ))}
            </div>
          </div>
          {/* resetKey={signalViewMode}: switching views clears any captured
              error so a crash in one view never blocks the others. */}
          <ViewErrorBoundary viewName="Signal" resetKey={signalViewMode}>
            {signalViewMode !== 'list' ? (
              <Suspense fallback={<div className="flex items-center justify-center py-20 text-text-secondary text-sm">{t('action.loading')}</div>}>
                {signalViewMode === 'themes' ? <ThemeMapView /> : <ContentGraphView />}
              </Suspense>
            ) : (
              <>
                {analysisComplete && (
                  <Suspense fallback={null}>
                    <FeedbackLivenessBanner />
                    <WhatYouWouldHaveMissed />
                    <SignalsPanel results={relevanceResults} />
                  </Suspense>
                )}
                <ResultsView
                  newItemIds={newItemIds}
                  focusedIndex={focusedIndex}
                />
              </>
            )}
          </ViewErrorBoundary>
        </div>
      )}
    </Suspense>
    </>
  );
}
