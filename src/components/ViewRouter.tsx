// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { lazy, Suspense, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { ViewErrorBoundary } from './ViewErrorBoundary';
import { ResultsView } from './ResultsView';
import { InterestsOffNote } from './signals/InterestsOffNote';
import { StackChangeLane } from './signals/StackChangeLane';
import { isReadingRow, useInterestsOn } from './signals/reading-feed';

const BriefingView = lazy(() => import('./BriefingView').then(m => ({ default: m.BriefingView })));
const SignalsPanel = lazy(() => import('./SignalsPanel').then(m => ({ default: m.SignalsPanel })));
const FeedbackLivenessBanner = lazy(() => import('./FeedbackLivenessBanner').then(m => ({ default: m.FeedbackLivenessBanner })));
const PreemptionView = lazy(() => import('./preemption/PreemptionView'));
const ContentGraphView = lazy(() => import('./signals/ContentGraphView'));
const ThemeMapView = lazy(() => import('./signals/ThemeMapView'));

/** Signal's sub-views: List (the stack-change stream, then the reading feed's
 *  lanes), Themes (the reading map) and Graph (how items and themes relate).
 *  One table, so the toggle buttons can never drift from the views they
 *  switch. Themes and Graph map the reading feed only: a stack change is one
 *  package's fact, with nothing to cluster, and they say so. With no reading
 *  feed (every interest off, AD-054) there is nothing to map, so the toggle
 *  is not offered and List is shown. */
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

  // Below "Your stack", Signal reads the interests the user turned on. Rows
  // already in the list count too, so a failed metadata load never hides
  // items that are there.
  const interestsOn = useInterestsOn();
  const readingResults = useMemo(() => relevanceResults.filter(isReadingRow), [relevanceResults]);
  const readingFeed = interestsOn === true || readingResults.length > 0;
  const viewMode = readingFeed ? signalViewMode : 'list';

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
          {readingFeed && (
            <div className="flex justify-end px-4 pt-3 pb-1">
              <div className="inline-flex rounded-lg border border-border bg-bg-secondary p-0.5">
                {SIGNAL_VIEWS.map(({ mode, labelKey }) => (
                  <button
                    key={mode}
                    onClick={() => setSignalViewMode(mode)}
                    className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
                      viewMode === mode
                        ? 'bg-bg-tertiary text-text-primary'
                        : 'text-text-muted hover:text-text-secondary'
                    }`}
                    aria-pressed={viewMode === mode}
                  >
                    {t(labelKey)}
                  </button>
                ))}
              </div>
            </div>
          )}
          {/* resetKey={viewMode}: switching views clears any captured
              error so a crash in one view never blocks the others. */}
          <ViewErrorBoundary viewName="Signal" resetKey={viewMode}>
            {viewMode !== 'list' ? (
              <>
                <p className="px-4 pt-1 text-[11px] text-text-muted" data-testid="signal-mode-note">{t('signals.stack.modeNote')}</p>
                <Suspense fallback={<div className="flex items-center justify-center py-20 text-text-secondary text-sm">{t('action.loading')}</div>}>
                  {viewMode === 'themes' ? <ThemeMapView /> : <ContentGraphView />}
                </Suspense>
              </>
            ) : (
              <>
                <StackChangeLane />
                {readingFeed ? (
                  <>
                    {analysisComplete && (
                      <Suspense fallback={null}>
                        <FeedbackLivenessBanner />
                        <SignalsPanel results={readingResults} />
                      </Suspense>
                    )}
                    <ResultsView
                      newItemIds={newItemIds}
                      focusedIndex={focusedIndex}
                    />
                  </>
                ) : interestsOn === false ? (
                  <InterestsOffNote />
                ) : null}
              </>
            )}
          </ViewErrorBoundary>
        </div>
      )}
    </Suspense>
    </>
  );
}
