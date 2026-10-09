// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { cmd } from '../lib/commands';
import { safeListen, type UnlistenFn } from '../lib/tauri-events';

import { useAppStore } from '../store';
import { getSourceNarration } from '../utils/first-run-messages';
import { isProfileEmpty } from '../utils/profile-empty';
import { isSurfacedSignal } from '../utils/score';
import { ErrorState } from './first-run/ErrorState';
import { CelebrationState } from './first-run/CelebrationState';
import { LoadingState } from './first-run/LoadingState';
import { buildStackInsights } from './first-run/utils';
import type { Phase, ScanSummary, StackInsight } from './first-run/utils';
import type { SourceRelevance } from '../types';

/** How often to re-check a pass that started before Finish has ended. */
const PASS_POLL_MS = 3000;
/** ~3 minutes of waiting on a pre-Finish pass before requesting anyway. */
const MAX_PASS_POLLS = 60;

/** What the celebration shows, captured once when this overlay's pass completes. */
interface CelebrationSnapshot {
  relevantCount: number;
  totalCount: number;
  topSignal: SourceRelevance | null;
  stackInsights: StackInsight[];
  profileEmpty: boolean;
}

interface FirstRunTransitionProps {
  onComplete: (view: 'briefing' | 'results') => void;
}

export function FirstRunTransition({ onComplete }: FirstRunTransitionProps) {
  const [phase, setPhase] = useState<Phase>('preparing');
  const [sourceMessages, setSourceMessages] = useState<string[]>([]);
  const [itemCount, setItemCount] = useState(0);
  const [hasError, setHasError] = useState(false);
  const [scanSummary, setScanSummary] = useState<ScanSummary | null>(null);
  const [estimatedSeconds, setEstimatedSeconds] = useState(240);
  const [celebration, setCelebration] = useState<CelebrationSnapshot | null>(null);
  const startedRef = useRef(false);
  // When THIS overlay asked for its own analysis pass (ms epoch). Null until the
  // intelligence hold ends. A completion only counts if it landed at/after this
  // moment — see the phase effect below.
  const ownPassRequestedAtRef = useRef<number | null>(null);
  const passPollRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Read store state
  const appState = useAppStore(s => s.appState);
  const embeddingMode = useAppStore(s => s.embeddingMode);
  const userContext = useAppStore(s => s.userContext);
  const detectedTech = useAppStore(s => s.discoveredContext?.tech);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const loadingRef = useRef(appState.loading);
  useEffect(() => { loadingRef.current = appState.loading; }, [appState.loading]);

  // Derived values from completed analysis — memoized to avoid recomputing on
  // every progress tick. `isSurfacedSignal` is THE definition of "relevant" the
  // header chip and the Signal tab count with; `r.relevant` alone also counted
  // exclusion-demoted rows, so the overlay said 21 where the app then said 20.
  const relevantCount = useMemo(
    () => appState.analysisComplete ? appState.relevanceResults.filter(isSurfacedSignal).length : 0,
    [appState.analysisComplete, appState.relevanceResults],
  );
  const totalCount = appState.relevanceResults.length;

  const topSignal = useMemo(
    () => appState.analysisComplete
      ? appState.relevanceResults.find(r => r.relevant && r.score_breakdown?.dep_match_score && r.score_breakdown.dep_match_score > 0)
        || appState.relevanceResults.find(r => r.relevant && r.score_breakdown?.skill_gap_boost && r.score_breakdown.skill_gap_boost > 0)
        || appState.relevanceResults.find(r => r.relevant)
      : null,
    [appState.analysisComplete, appState.relevanceResults],
  );

  const stackInsights = useMemo(
    () => appState.analysisComplete ? buildStackInsights(appState.relevanceResults, scanSummary) : [],
    [appState.analysisComplete, appState.relevanceResults, scanSummary],
  );

  // A profileless first run (setup skipped, no project folder, no ACE context)
  // has NO signal to rank against, so the confirmation gate caps every item
  // below the relevance threshold — 0 relevant is structurally guaranteed, not a
  // failure. Detect it so the celebration shows an honest "add a signal to start
  // ranking" framing instead of a banned vanity "0 RELEVANT" headline
  // (intelligence-doctrine rules 3 + 6).
  const profileEmpty = useMemo(
    () => isProfileEmpty(detectedTech?.length ?? 0, userContext?.interests?.length ?? 0, relevantCount > 0),
    [detectedTech, userContext, relevantCount],
  );

  // Request the pass this overlay celebrates. A pass already in flight was
  // started BEFORE Finish (the background run during onboarding) and scores the
  // old profile — wait for it to end, then start our own.
  // Bounded: a `loading` flag that never clears must not strand the overlay —
  // after MAX_PASS_POLLS the request goes out regardless.
  const requestOwnPass = useCallback(() => {
    let polls = 0;
    const attempt = async () => {
      passPollRef.current = null;
      let running = loadingRef.current;
      try {
        const status = await cmd('get_analysis_status');
        running = running || status.running === true;
      } catch { /* status unavailable — fall through and request */ }
      if (running && polls < MAX_PASS_POLLS) {
        polls += 1;
        passPollRef.current = setTimeout(() => void attempt(), PASS_POLL_MS);
        return;
      }
      ownPassRequestedAtRef.current = Date.now();
      void startAnalysis();
    };
    void attempt();
  }, [startAnalysis]);

  useEffect(() => () => {
    if (passPollRef.current) clearTimeout(passPollRef.current);
  }, []);

  // Anything to rank against while the pass runs? Same inputs as
  // `profileEmpty`, plus a project scan the store may not have folded in yet.
  const hasRankingSignal =
    (detectedTech?.length ?? 0) > 0 || (userContext?.interests?.length ?? 0) > 0 || scanSummary !== null;

  // Fetch scan summary and trigger analysis on mount
  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;

    const init = async () => {
      // Estimate time based on enabled source count
      try {
        const sources = await cmd('get_sources') as Array<{ enabled: boolean }>;
        const enabledCount = sources.filter(s => s.enabled).length;
        // Sources are fetched concurrently, so the cost scales sub-linearly.
        // The old 120 + n*10 over-promised (~5 min for ~11 sources when real
        // runs land near 2). Bias the estimate toward observed reality.
        setEstimatedSeconds(75 + enabledCount * 6);
      } catch { /* default 240s */ }

      // Fetch scan summary BEFORE starting analysis
      let hasScanData = false;
      try {
        const summary = await cmd('ace_get_scan_summary') as unknown as ScanSummary;
        if (summary.has_data) {
          setScanSummary(summary);
          hasScanData = true;
        }
      } catch {
        // Scan summary unavailable — proceed with discovery state
      }

      setPhase('intelligence');
      const holdMs = hasScanData ? 3500 : 2000;
      passPollRef.current = setTimeout(requestOwnPass, holdMs);
    };
    void init();
  }, [requestOwnPass]);

  // Narration events from backend analysis
  const [narrationEvents, setNarrationEvents] = useState<Array<{
    type: string;
    message: string;
    timestamp: number;
  }>>([]);

  // Listen for source-fetched events for real-time narration
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    const setup = async () => {
      unlisten = await safeListen<{ source: string; count: number }>('source-fetched', (event) => {
        const { source, count } = event.payload;
        setItemCount(prev => prev + count);
        setSourceMessages(prev => [...prev.slice(-4), getSourceNarration(source, count)]);
      });
    };
    void setup();
    return () => { if (unlisten) unlisten(); };
  }, []);

  // Listen for analysis-narration events for live narration feed
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    const setup = async () => {
      unlisten = await safeListen<{
        narration_type: string;
        message: string;
        source: string | null;
        relevance: number | null;
      }>('analysis-narration', (event) => {
        setNarrationEvents(prev => [...prev.slice(-20), {
          type: event.payload.narration_type,
          message: event.payload.message,
          timestamp: Date.now(),
        }]);
      });
    };
    void setup();
    return () => { if (unlisten) unlisten(); };
  }, []);

  // Phase transitions based on appState changes.
  //
  // Celebration is one-way and belongs to OUR pass (fresh-profile E2E
  // 2026-10-09): `analysisComplete` was already true from the background pass
  // that ran during onboarding, so the overlay celebrated that pass's numbers
  // (552 / 21) seconds after Finish, fell back to "Matching…" when our pass
  // reset the flag, then celebrated again (812 / 20). Now a completion counts
  // only when it landed at/after our request and the run is no longer loading
  // (the background-results merge flips `analysisComplete` mid-run without
  // clearing `loading`), and once celebrating nothing moves the phase back.
  useEffect(() => {
    if (phase === 'fading' || phase === 'celebrating') return;

    if (appState.progressStage === 'error') {
      setHasError(true);
      return;
    }

    const requestedAt = ownPassRequestedAtRef.current;
    const completedAt = appState.lastAnalyzedAt ? new Date(appState.lastAnalyzedAt).getTime() : null;
    if (
      appState.analysisComplete && !appState.loading &&
      requestedAt !== null && completedAt !== null && completedAt >= requestedAt
    ) {
      // Freeze what this pass found: a later run (scheduled, background
      // refresh) resets `analysisComplete` and would otherwise blank the
      // numbers and the top signal under a celebration that stays on screen.
      setCelebration({ relevantCount, totalCount, topSignal: topSignal ?? null, stackInsights, profileEmpty });
      setPhase('celebrating');
      // Auto-render content digests in background while user sees celebration
      void cmd('auto_render_all_channels').catch(() => {});
      return;
    }

    if (appState.loading) {
      const stage = appState.progressStage;
      if (stage === 'fetch' || stage === 'scrape') {
        setPhase('fetching');
      } else if (stage === 'embed' || stage === 'relevance' || stage === 'rerank') {
        setPhase('analyzing');
      }
    }
  }, [
    appState.loading, appState.progressStage, appState.analysisComplete, appState.lastAnalyzedAt, phase,
    relevantCount, totalCount, topSignal, stackInsights, profileEmpty,
  ]);

  // Dismiss handler — fade out then call onComplete
  const handleDismiss = useCallback((view: 'briefing' | 'results') => {
    setPhase('fading');
    setTimeout(() => onComplete(view), 300);
  }, [onComplete]);

  // Retry handler
  const handleRetry = useCallback(() => {
    setHasError(false);
    setSourceMessages([]);
    setItemCount(0);
    requestOwnPass();
  }, [requestOwnPass]);

  // User's interests for the preparing phase
  const interests = userContext?.interests?.map(i => i.topic).slice(0, 5) ?? [];

  // Render the appropriate phase content
  const renderContent = () => {
    if (hasError) {
      return (
        <ErrorState
          status={appState.status || ''}
          onRetry={handleRetry}
          onContinue={() => handleDismiss('results')}
        />
      );
    }

    if ((phase === 'celebrating' || phase === 'fading') && celebration) {
      return (
        <CelebrationState
          relevantCount={celebration.relevantCount}
          totalCount={celebration.totalCount}
          topSignal={celebration.topSignal}
          stackInsights={celebration.stackInsights}
          embeddingMode={embeddingMode}
          detectedTech={detectedTech}
          profileEmpty={celebration.profileEmpty}
          onDismiss={handleDismiss}
        />
      );
    }

    return (
      <LoadingState
        phase={phase}
        progress={appState.progress}
        progressStage={appState.progressStage || 'init'}
        itemCount={itemCount}
        sourceMessages={sourceMessages}
        interests={interests}
        embeddingMode={embeddingMode}
        scanSummary={scanSummary}
        narrationEvents={narrationEvents}
        estimatedSeconds={estimatedSeconds}
        hasProfile={hasRankingSignal}
        onSkipAhead={() => handleDismiss('results')}
      />
    );
  };

  return (
    <div
      role="status"
      aria-busy={phase !== 'celebrating' && phase !== 'fading'}
      aria-label={
        hasError ? 'Analysis error' :
        phase === 'preparing' ? 'Preparing analysis' :
        phase === 'intelligence' ? 'Showing project intelligence' :
        phase === 'fetching' ? 'Scanning sources' :
        phase === 'analyzing' ? 'Analyzing results' :
        phase === 'celebrating' ? (celebration?.profileEmpty ? 'Scan complete' : `Analysis complete: ${celebration?.relevantCount ?? 0} relevant items found`) :
        'Completing'
      }
      className={`fixed inset-0 z-40 bg-bg-primary overflow-y-auto transition-opacity duration-300 ${
        phase === 'fading' ? 'opacity-0' : 'opacity-100'
      }`}
    >
      {/* min-h-full + the parent's overflow-y-auto: centered when the content
          fits, scrollable when it doesn't (the celebration step overflows on
          short windows — same idiom as Onboarding.tsx). Centering on the
          scroll container itself would clip the top edge unreachably. */}
      <div className="min-h-full flex flex-col items-center justify-center py-8">
        {renderContent()}
      </div>
    </div>
  );
}
