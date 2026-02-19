import { useState, useEffect, useRef, useCallback } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { VoidEngine } from './void-engine/VoidEngine';
import { useAppStore } from '../store';
import { getStageNarration, getSourceNarration, getCelebrationMessage } from '../utils/first-run-messages';
import { getSourceFullName } from '../config/sources';

type Phase = 'preparing' | 'fetching' | 'analyzing' | 'celebrating' | 'fading';

interface FirstRunTransitionProps {
  onComplete: (view: 'briefing' | 'results') => void;
}

export function FirstRunTransition({ onComplete }: FirstRunTransitionProps) {
  const [phase, setPhase] = useState<Phase>('preparing');
  const [sourceMessages, setSourceMessages] = useState<string[]>([]);
  const [itemCount, setItemCount] = useState(0);
  const [hasError, setHasError] = useState(false);
  const startedRef = useRef(false);

  // Read store state
  const appState = useAppStore(s => s.appState);
  const embeddingMode = useAppStore(s => s.embeddingMode);
  const userContext = useAppStore(s => s.userContext);
  const startAnalysis = useAppStore(s => s.startAnalysis);

  // Derived values from completed analysis
  const relevantCount = appState.analysisComplete
    ? appState.relevanceResults.filter(r => r.relevant).length
    : 0;
  const totalCount = appState.relevanceResults.length;

  // Source breakdown for celebration
  const sourceBreakdown = appState.analysisComplete
    ? Array.from(
        appState.relevanceResults.reduce((map, r) => {
          const src = r.source_type || 'hackernews';
          map.set(src, (map.get(src) || 0) + 1);
          return map;
        }, new Map<string, number>()),
      ).sort((a, b) => b[1] - a[1])
    : [];

  // Top signal for celebration highlight
  const topSignal = appState.analysisComplete
    ? appState.relevanceResults.find(r => r.relevant)
    : null;

  // Auto-trigger analysis on mount
  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    // Small delay to let the preparing phase render
    const timer = setTimeout(() => startAnalysis(), 300);
    return () => clearTimeout(timer);
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
  const handleDismiss = useCallback((view: 'briefing' | 'results') => {
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

  return (
    <div
      className={`fixed inset-0 z-40 bg-bg-primary flex flex-col items-center justify-center transition-opacity duration-300 ${
        phase === 'fading' ? 'opacity-0' : 'opacity-100'
      }`}
    >
      {/* Error state */}
      {hasError ? (
        <div className="text-center px-8 max-w-md">
          <div className="w-20 h-20 mx-auto mb-6 bg-red-500/10 rounded-2xl border border-red-500/30 flex items-center justify-center">
            <svg className="w-8 h-8 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
            </svg>
          </div>
          <h2 className="text-xl font-medium text-white mb-2">Analysis Hit a Snag</h2>
          <p className="text-sm text-gray-400 mb-6">{appState.status}</p>
          <button
            onClick={handleRetry}
            className="px-6 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 transition-colors"
          >
            Try Again
          </button>
        </div>
      ) : phase === 'celebrating' ? (
        /* Celebration phase */
        <div className="text-center px-8 max-w-lg">
          <div className="mb-6">
            <VoidEngine size={80} />
          </div>

          {/* Big relevant count */}
          <div className="mb-4">
            <span className="text-6xl font-bold text-white tabular-nums">{relevantCount}</span>
            <p className="text-sm text-gray-400 mt-2">
              {getCelebrationMessage(relevantCount, totalCount)}
            </p>
          </div>

          {/* Source breakdown */}
          {sourceBreakdown.length > 0 && (
            <div className="flex flex-wrap justify-center gap-2 mb-6">
              {sourceBreakdown.map(([src, count]) => (
                <span key={src} className="px-2.5 py-1 text-xs bg-bg-secondary text-gray-300 rounded-lg border border-border">
                  {getSourceFullName(src)} <span className="text-gray-500">{count}</span>
                </span>
              ))}
            </div>
          )}

          {/* Top signal highlight */}
          {topSignal && (
            <div className="mb-6 p-4 bg-bg-secondary rounded-lg border border-orange-500/20 text-left max-w-sm mx-auto">
              <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-1">Top Match</p>
              <p className="text-sm text-white font-medium leading-snug line-clamp-2">{topSignal.title}</p>
              <p className="text-xs text-gray-500 mt-1 truncate">{topSignal.url}</p>
            </div>
          )}

          {/* Keyword-only note */}
          {embeddingMode === 'keyword-only' && (
            <div className="mb-6 px-4 py-2.5 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-400 max-w-sm mx-auto">
              Running in keyword-only mode. Add an Ollama model or API key for semantic matching.
            </div>
          )}

          {/* CTAs */}
          <div className="flex flex-col items-center gap-3">
            <button
              onClick={() => handleDismiss('briefing')}
              className="px-8 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 hover:scale-105 active:scale-95 transition-all"
            >
              See Your Intelligence Briefing
            </button>
            <button
              onClick={() => handleDismiss('results')}
              className="text-sm text-gray-500 hover:text-gray-300 transition-colors"
            >
              Browse all {totalCount} results
            </button>
          </div>
        </div>
      ) : (
        /* Preparing / Fetching / Analyzing phases */
        <div className="text-center px-8 max-w-md">
          <div className="mb-6">
            <VoidEngine size={80} />
          </div>

          <h2 className="text-xl font-medium text-white mb-2">
            {phase === 'preparing' && 'Building your intelligence profile...'}
            {phase === 'fetching' && 'Scanning your sources...'}
            {phase === 'analyzing' && 'Matching against your interests...'}
          </h2>

          {/* Stage narration */}
          <p className="text-sm text-gray-400 mb-6">
            {getStageNarration(appState.progressStage || 'init')}
          </p>

          {/* User interests (preparing phase) */}
          {phase === 'preparing' && interests.length > 0 && (
            <div className="flex flex-wrap justify-center gap-2 mb-6">
              {interests.map(topic => (
                <span key={topic} className="px-2.5 py-1 text-xs bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20">
                  {topic}
                </span>
              ))}
            </div>
          )}

          {/* Progress bar (fetching/analyzing) */}
          {(phase === 'fetching' || phase === 'analyzing') && (
            <div className="max-w-xs mx-auto mb-4">
              <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-500 ease-out rounded-full"
                  style={{ width: `${Math.max(appState.progress * 100, 5)}%` }}
                />
              </div>
              <div className="flex justify-between text-xs text-gray-600 mt-1.5">
                <span>{itemCount > 0 ? `${itemCount} items found` : ''}</span>
                <span>{Math.round(appState.progress * 100)}%</span>
              </div>
            </div>
          )}

          {/* Source-by-source narration (fetching phase) */}
          {phase === 'fetching' && sourceMessages.length > 0 && (
            <div className="space-y-1 max-w-xs mx-auto">
              {sourceMessages.slice(-3).map((msg, i, arr) => (
                <p
                  key={`${msg}-${i}`}
                  className={`text-xs transition-opacity ${
                    i === arr.length - 1 ? 'text-gray-300' : 'text-gray-600'
                  }`}
                >
                  {msg}
                </p>
              ))}
            </div>
          )}

          {/* Analyzing phase — keyword-only note */}
          {phase === 'analyzing' && embeddingMode === 'keyword-only' && (
            <p className="text-xs text-amber-400 mt-4">
              Using keyword matching (no embedding model available)
            </p>
          )}
        </div>
      )}
    </div>
  );
}
