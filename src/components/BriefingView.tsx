// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useState, useEffect, useRef, memo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { BriefingSkeleton } from './briefing/BriefingSkeleton';
import { BriefingContentPanel } from './briefing/BriefingContentPanel';
import { FreeBriefingPanel } from './briefing/FreeBriefingPanel';
import { InstantSnapshotPanel } from './briefing/InstantSnapshotPanel';
import { MorningBriefPanel } from './briefing/MorningBriefPanel';
import { PersonalizeNudge } from './briefing/PersonalizeNudge';
import { BriefingLoadingState, BriefingReadyState } from './BriefingEmptyStates';
import { BriefingWarmupState } from './BriefingWarmupState';
import { useLicense } from '../hooks/use-license';
import { useBriefingDerived } from '../hooks/use-briefing-derived';
import type { SourceRelevance } from '../types';

export const BriefingView = memo(function BriefingView() {
  const { t } = useTranslation();

  const {
    briefing, results, isLoading, analysisComplete, feedbackGiven,
    lastBackgroundResultsAt, sourceHealth,
    freeBriefing, freeBriefingLoading, morningBriefSynthesis, morningBriefClusters, morningBriefData, instantSnapshot,
  } = useAppStore(
    useShallow((s) => ({
      briefing: s.aiBriefing,
      results: s.appState.relevanceResults,
      isLoading: s.appState.loading,
      analysisComplete: s.appState.analysisComplete,
      feedbackGiven: s.feedbackGiven,
      lastBackgroundResultsAt: s.lastBackgroundResultsAt,
      sourceHealth: s.sourceHealth,
      freeBriefing: s.freeBriefing,
      freeBriefingLoading: s.freeBriefingLoading,
      morningBriefSynthesis: s.morningBriefSynthesis,
      morningBriefClusters: s.morningBriefClusters,
      morningBriefData: s.morningBriefData,
      instantSnapshot: s.instantSnapshot,
    })),
  );

  const generateBriefing = useAppStore(s => s.generateBriefing);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const setActiveView = useAppStore(s => s.setActiveView);
  const addToast = useAppStore(s => s.addToast);
  const generateFreeBriefing = useAppStore(s => s.generateFreeBriefing);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const setShowSettings = useAppStore(s => s.setShowSettings);

  // First-run personalization nudge
  const isFirstRun = useAppStore(s => s.isFirstRun);
  const userContext = useAppStore(s => s.userContext);
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const isScanning = useAppStore(s => s.isScanning);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const [personalizeCardDismissed, setPersonalizeCardDismissed] = useState(false);
  const showPersonalizeNudge = isFirstRun
    && !personalizeCardDismissed
    && (!userContext?.interests || userContext.interests.length === 0);

  const { isPro } = useLicense();

  const handleSave = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'save', it); }, [recordInteraction]);
  const handleDismiss = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'dismiss', it); }, [recordInteraction]);
  const handleRecordClick = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'click', it); }, [recordInteraction]);

  // One-click recovery for users who skipped the onboarding scan: run the same
  // fully-local ace_auto_discover, then refresh context (the nudge auto-hides once
  // interests populate) and re-score the feed against the freshly-discovered profile.
  const handleScanProjects = useCallback(async () => {
    await runAutoDiscovery();
    await loadUserContext();
    void startAnalysis();
  }, [runAutoDiscovery, loadUserContext, startAnalysis]);

  // Listen for standing query matches
  useEffect(() => {
    const unlisten = listen<Array<{ query_id: number; query_text: string; new_matches: number; example_title: string | null }>>(
      'standing-query-matches',
      (event) => {
        const alerts = event.payload.filter(a => a.new_matches > 0);
        if (alerts.length > 0) {
          const msg = alerts.length === 1
            ? t('standingQueries.singleMatch', { query: alerts[0]!.query_text, count: alerts[0]!.new_matches })
            : t('standingQueries.multiMatch', { count: alerts.length });
          addToast('info', msg);
        }
      },
    );
    return () => { void unlisten.then(fn => fn()); };
  }, [addToast, t]);

  // Auto-generate free briefing when analysis completes
  useEffect(() => {
    if (analysisComplete && results.length > 0 && !freeBriefing && !freeBriefingLoading) {
      void generateFreeBriefing();
    }
  }, [analysisComplete, results.length, freeBriefing, freeBriefingLoading, generateFreeBriefing]);

  // Cold-boot freshen: when we paint the cached snapshot but have no live
  // results yet, kick off one cache-first analysis so "fresh intelligence
  // loading…" is actually true. This is the reliable trigger for the case the
  // mount-only auto-analysis in use-app-listeners misses — it can fire at T+0
  // against a cold engine, then its 15s cooldown blocks the only retry, leaving
  // the feed empty. startAnalysis() self-guards on appState.loading, so this
  // composes safely: whichever path fires first wins, the other no-ops. Runs at
  // most once per mount; never for first-run users (who haven't been set up yet).
  const coldBootFreshenedRef = useRef(false);
  useEffect(() => {
    if (
      !coldBootFreshenedRef.current &&
      instantSnapshot &&
      results.length === 0 &&
      !analysisComplete &&
      !isLoading &&
      !isFirstRun
    ) {
      coldBootFreshenedRef.current = true;
      void startAnalysis();
    }
  }, [instantSnapshot, results.length, analysisComplete, isLoading, isFirstRun, startAnalysis]);

  const { signalItems, topItems } =
    useBriefingDerived(results, sourceHealth, briefing, lastBackgroundResultsAt);

  // Loading skeleton
  if (briefing.loading) {
    return <BriefingSkeleton />;
  }

  // Sovereign Cold Boot — instant first paint of yesterday's briefing, held
  // until LIVE analysis results arrive.
  //
  // This was previously gated on `!briefing.content`. But loadPersistedBriefing()
  // (App.tsx mount) sets aiBriefing.content a few hundred ms after first paint,
  // which flipped that gate and EVICTED the snapshot — dropping the user into the
  // empty, results-driven main view ("No intelligence gathered yet. Run an
  // analysis to get started.") even though fresh persisted intelligence existed
  // and `get_briefing_snapshot` was already on screen. The narrative in
  // briefing.content is never rendered by the main view, so letting it win here
  // showed *less*, not more.
  //
  // Gate on the absence of live results instead: the snapshot survives the
  // briefing-content load and is superseded only when real analysis output
  // (results) or completion replaces it — which is exactly when the 3-zone
  // hierarchy has something to render.
  if (instantSnapshot && results.length === 0 && !analysisComplete) {
    return <InstantSnapshotPanel snapshot={instantSnapshot} />;
  }

  // Empty state: no briefing content and not generating
  if (!briefing.content) {
    if (isLoading) return <BriefingLoadingState />;

    // Free briefing for non-Pro users
    if (!isPro && freeBriefing && !freeBriefing.empty) {
      return (
        <FreeBriefingPanel
          freeBriefing={freeBriefing}
          morningBriefSynthesis={morningBriefSynthesis}
          morningBriefClusters={morningBriefClusters}
          showPersonalizeNudge={showPersonalizeNudge}
          onScanProjects={() => { void handleScanProjects(); }}
          isScanningProjects={isScanning}
          onOpenSettings={() => setShowSettings(true)}
          onDismissPersonalize={() => setPersonalizeCardDismissed(true)}
          onGenerateBriefing={() => { void generateBriefing(); }}
        />
      );
    }

    // Morning briefing items — fills the gap between startup and analysis completion.
    // The T+3s morning check produces scored items from the DB; render them while
    // the full analysis runs in the background.
    // Also render when data is stale (0 items but staleness flag set) so the user
    // sees the problem instead of silence masquerading as "all clear."
    if (morningBriefData && (morningBriefData.items.length > 0 || morningBriefData.dataFreshness?.is_stale)) {
      return (
        <MorningBriefPanel
          morningBriefData={morningBriefData}
          morningBriefSynthesis={morningBriefSynthesis}
          morningBriefClusters={morningBriefClusters}
        />
      );
    }

    if (analysisComplete && results.length > 0) return <BriefingReadyState />;
    return <BriefingWarmupState onAnalyze={() => { void startAnalysis(); }} />;
  }

  // Main view: Intelligence Hierarchy (3 zones)
  return (
    <section aria-label={t('briefing.intelligenceBriefing')} className="bg-bg-primary rounded-lg space-y-5">
      <h2 className="sr-only">{t('briefing.intelligenceBriefing')}</h2>
      {showPersonalizeNudge && (
        <PersonalizeNudge
          onScanProjects={() => { void handleScanProjects(); }}
          isScanning={isScanning}
          onOpenSettings={() => setShowSettings(true)}
          onDismiss={() => setPersonalizeCardDismissed(true)}
        />
      )}

      <BriefingContentPanel
        briefing={briefing}
        results={results}
        feedbackGiven={feedbackGiven}
        sourceHealth={sourceHealth}
        signalItems={signalItems}
        topItems={topItems}
        onSave={handleSave}
        onDismiss={handleDismiss}
        onRecordClick={handleRecordClick}
        setActiveView={setActiveView}
      />

      {/* Error display */}
      {briefing.error && (
        <div role="alert" className="p-4 bg-red-900/20 border border-red-500/30 rounded-lg">
          <div className="flex flex-col items-center justify-center gap-3 text-center">
            <p className="text-text-secondary text-sm">{t('error.generic')}</p>
            <button
              onClick={() => { void generateBriefing(); }}
              className="px-3 py-1.5 text-xs bg-bg-tertiary hover:bg-text-primary/10 rounded transition-colors text-text-secondary"
              aria-label="Retry generating briefing"
            >
              {t('action.retry')}
            </button>
          </div>
        </div>
      )}
    </section>
  );
});
