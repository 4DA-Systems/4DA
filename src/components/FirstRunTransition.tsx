import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

import { useAppStore } from '../store';
import { getSourceNarration } from '../utils/first-run-messages';
import { ErrorState } from './first-run/ErrorState';
import { CelebrationState } from './first-run/CelebrationState';
import { LoadingState } from './first-run/LoadingState';
import { buildStackInsights } from './first-run/utils';
import type { Phase, ScanSummary } from './first-run/utils';

interface FirstRunTransitionProps {
  onComplete: (view: 'briefing' | 'results' | 'playbook') => void;
}

export function FirstRunTransition({ onComplete }: FirstRunTransitionProps) {
  const [phase, setPhase] = useState<Phase>('preparing');
  const [sourceMessages, setSourceMessages] = useState<string[]>([]);
  const [itemCount, setItemCount] = useState(0);
  const [hasError, setHasError] = useState(false);
  const [scanSummary, setScanSummary] = useState<ScanSummary | null>(null);
  const startedRef = useRef(false);

  // Read store state
  const appState = useAppStore(s => s.appState);
  const embeddingMode = useAppStore(s => s.embeddingMode);
  const userContext = useAppStore(s => s.userContext);
  const startAnalysis = useAppStore(s => s.startAnalysis);

  // Derived values from completed analysis — memoized to avoid recomputing on every progress tick
  const relevantCount = useMemo(
    () => appState.analysisComplete ? appState.relevanceResults.filter(r => r.relevant).length : 0,
    [appState.analysisComplete, appState.relevanceResults],
  );
  const totalCount = appState.relevanceResults.length;

  const sourceBreakdown = useMemo(
    () => appState.analysisComplete
      ? Array.from(
          appState.relevanceResults.reduce((map, r) => {
            const src = r.source_type || 'hackernews';
            map.set(src, (map.get(src) || 0) + 1);
            return map;
          }, new Map<string, number>()),
        ).sort((a, b) => b[1] - a[1])
      : [],
    [appState.analysisComplete, appState.relevanceResults],
  );

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

  // Fetch scan summary and trigger analysis on mount
  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;

    const init = async () => {
      // Fetch scan summary BEFORE starting analysis
      try {
        const summary = await invoke<ScanSummary>('ace_get_scan_summary');
        if (summary.has_data) {
          setScanSummary(summary);
          setPhase('intelligence');
          // Show intelligence preview for 3.5s, then start analysis
          setTimeout(() => startAnalysis(), 3500);
          return;
        }
      } catch {
        // Scan summary unavailable — skip intelligence phase
      }
      // No scan data — start analysis directly
      setTimeout(() => startAnalysis(), 300);
    };
    init();
  }, [startAnalysis]);

  // Listen for source-fetched events for real-time narration
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    const setup = async () => {
      unlisten = await listen<{ source: string; count: number }>('source-fetched', (event) => {
        const { source, count } = event.payload;
        setItemCount(prev => prev + count);
        setSourceMessages(prev => [...prev.slice(-4), getSourceNarration(source, count)]);
      });
    };
    setup();
    return () => { if (unlisten) unlisten(); };
  }, []);

  // Phase transitions based on appState changes
  useEffect(() => {
    if (phase === 'fading') return;

    if (appState.progressStage === 'error') {
      setHasError(true);
      return;
    }

    if (appState.analysisComplete) {
      setPhase('celebrating');
      // Auto-render channels in background while user sees celebration
      invoke('auto_render_all_channels').catch(() => {});
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
  }, [appState.loading, appState.progressStage, appState.analysisComplete, phase]);

  // Dismiss handler — fade out then call onComplete
  const handleDismiss = useCallback((view: 'briefing' | 'results' | 'playbook') => {
    setPhase('fading');
    setTimeout(() => onComplete(view), 300);
  }, [onComplete]);

  // Retry handler
  const handleRetry = useCallback(() => {
    setHasError(false);
    setSourceMessages([]);
    setItemCount(0);
    startAnalysis();
  }, [startAnalysis]);

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

    if (phase === 'celebrating') {
      return (
        <CelebrationState
          relevantCount={relevantCount}
          totalCount={totalCount}
          sourceBreakdown={sourceBreakdown}
          topSignal={topSignal ?? null}
          stackInsights={stackInsights}
          embeddingMode={embeddingMode}
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
        phase === 'celebrating' ? `Analysis complete: ${relevantCount} relevant items found` :
        'Completing'
      }
      className={`fixed inset-0 z-40 bg-bg-primary flex flex-col items-center justify-center transition-opacity duration-300 ${
        phase === 'fading' ? 'opacity-0' : 'opacity-100'
      }`}
    >
      {renderContent()}
    </div>
  );
}
