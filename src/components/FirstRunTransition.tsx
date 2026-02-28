import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { VoidEngine } from './void-engine/VoidEngine';
import { useAppStore } from '../store';
import { getStageNarration, getSourceNarration, getCelebrationMessage } from '../utils/first-run-messages';
import { getSourceFullName } from '../config/sources';

type Phase = 'preparing' | 'intelligence' | 'fetching' | 'analyzing' | 'celebrating' | 'fading';

interface ScanSummary {
  projects_scanned: number;
  total_dependencies: number;
  dependencies_by_ecosystem: { rust: number; npm: number; python: number; other: number };
  languages: string[];
  frameworks: string[];
  primary_stack: string;
  key_packages: string[];
  has_data: boolean;
}

interface FirstRunTransitionProps {
  onComplete: (view: 'briefing' | 'results') => void;
}

export function FirstRunTransition({ onComplete }: FirstRunTransitionProps) {
  const { t } = useTranslation();
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

  // Top signals for celebration — dep matches and skill gap matches first
  const topSignal = appState.analysisComplete
    ? appState.relevanceResults.find(r => r.relevant && r.score_breakdown?.dep_match_score && r.score_breakdown.dep_match_score > 0)
      || appState.relevanceResults.find(r => r.relevant && r.score_breakdown?.skill_gap_boost && r.score_breakdown.skill_gap_boost > 0)
      || appState.relevanceResults.find(r => r.relevant)
    : null;

  // Stack-specific celebration insights
  const stackInsights = appState.analysisComplete ? buildStackInsights(appState.relevanceResults, scanSummary) : [];

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
      {/* Error state */}
      {hasError ? (
        <div className="text-center px-8 max-w-md">
          <div className={`w-20 h-20 mx-auto mb-6 rounded-2xl border flex items-center justify-center ${
            appState.status?.includes('Embedding')
              ? 'bg-amber-500/10 border-amber-500/30'
              : 'bg-red-500/10 border-red-500/30'
          }`}>
            <svg className={`w-8 h-8 ${appState.status?.includes('Embedding') ? 'text-amber-400' : 'text-red-400'}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              {appState.status?.includes('Embedding') ? (
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
              ) : (
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
              )}
            </svg>
          </div>
          <h2 className="text-xl font-medium text-white mb-2">{t('firstRun.errorTitle')}</h2>
          <p className="text-sm text-gray-400 mb-4">
            {appState.status?.includes('Embedding')
              ? t('firstRun.errorEmbedding')
              : appState.status?.includes('fetch')
              ? t('firstRun.errorFetch')
              : t('firstRun.errorGeneric')}
          </p>
          {appState.status?.includes('Embedding') && (
            <p className="text-xs text-gray-500 mb-6 px-4">
              {t('firstRun.basicModeExplainer')}
            </p>
          )}
          {!appState.status?.includes('Embedding') && <div className="mb-6" />}
          <div className="flex flex-col items-center gap-3">
            <div className="flex items-center gap-3">
              <button
                onClick={handleRetry}
                aria-label="Retry analysis"
                className="px-6 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 transition-colors"
              >
                {t('firstRun.tryAgain')}
              </button>
              <button
                onClick={() => handleDismiss('results')}
                className="px-6 py-3 text-gray-400 hover:text-white transition-colors text-sm"
              >
                {t('firstRun.continueAnyway')}
              </button>
            </div>
            <p className="text-xs text-gray-600">
              {t('firstRun.settingsHint')}
            </p>
          </div>
        </div>

      ) : phase === 'intelligence' && scanSummary ? (
        /* Intelligence Preview — "Here's what I found" interstitial */
        <IntelligencePreview summary={scanSummary} />

      ) : phase === 'celebrating' ? (
        /* Celebration phase — "I Already Know You" */
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

          {/* Stack-specific insights — the "I Already Know You" moment */}
          {stackInsights.length > 0 && (
            <div className="mb-6 space-y-2 max-w-sm mx-auto">
              {stackInsights.slice(0, 3).map((insight, i) => (
                <div key={i} className="px-4 py-2.5 bg-bg-secondary rounded-lg border border-border text-left">
                  <p className="text-xs text-gray-300 leading-relaxed">{insight}</p>
                </div>
              ))}
            </div>
          )}

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
              <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-1">
                {topSignal.score_breakdown?.dep_match_score && topSignal.score_breakdown.dep_match_score > 0
                  ? t('firstRun.topMatchStack', 'Matches your stack')
                  : t('firstRun.topMatch')}
              </p>
              <p className="text-sm text-white font-medium leading-snug line-clamp-2">{topSignal.title}</p>
              {topSignal.score_breakdown?.matched_deps && topSignal.score_breakdown.matched_deps.length > 0 && (
                <p className="text-[10px] text-blue-400 mt-1">
                  {topSignal.score_breakdown.matched_deps.slice(0, 3).join(', ')}
                </p>
              )}
              <p className="text-xs text-gray-500 mt-1 truncate">{topSignal.url}</p>
            </div>
          )}

          {/* Keyword-only note */}
          {embeddingMode === 'keyword-only' && (
            <div className="mb-6 px-4 py-2.5 bg-bg-secondary border border-border rounded-lg text-xs text-gray-400 max-w-sm mx-auto">
              {t('firstRun.keywordHint')}
            </div>
          )}

          {/* CTAs */}
          <div className="flex flex-col items-center gap-3">
            <button
              onClick={() => handleDismiss('briefing')}
              className="px-8 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 hover:scale-105 active:scale-95 transition-all"
            >
              {t('firstRun.seeBriefing')}
            </button>
            <button
              onClick={() => handleDismiss('results')}
              className="text-sm text-gray-500 hover:text-gray-300 transition-colors"
            >
              {t('firstRun.browseResults', { count: totalCount })}
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
            {phase === 'preparing' && t('firstRun.preparing')}
            {phase === 'fetching' && t('firstRun.fetching')}
            {phase === 'analyzing' && t('firstRun.analyzing')}
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
                <span>{itemCount > 0 ? t('firstRun.itemsFound', { count: itemCount }) : ''}</span>
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
            <p className="text-xs text-gray-500 mt-4">
              {t('firstRun.keywordMatching')}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Intelligence Preview — "Here's what I found" interstitial
// ============================================================================

function IntelligencePreview({ summary }: { summary: ScanSummary }) {
  const { t } = useTranslation();

  // Build ecosystem breakdown pills
  const ecosystems = [
    summary.dependencies_by_ecosystem.rust > 0 && { label: 'Rust', count: summary.dependencies_by_ecosystem.rust },
    summary.dependencies_by_ecosystem.npm > 0 && { label: 'npm', count: summary.dependencies_by_ecosystem.npm },
    summary.dependencies_by_ecosystem.python > 0 && { label: 'Python', count: summary.dependencies_by_ecosystem.python },
    summary.dependencies_by_ecosystem.other > 0 && { label: 'Other', count: summary.dependencies_by_ecosystem.other },
  ].filter(Boolean) as { label: string; count: number }[];

  return (
    <div className="text-center px-8 max-w-md animate-fade-in">
      <div className="mb-6">
        <VoidEngine size={80} />
      </div>

      <h2 className="text-xl font-medium text-white mb-2">
        {t('firstRun.intelligenceTitle', "Here's what I found")}
      </h2>

      {/* Scan stats */}
      <div className="flex justify-center gap-6 mb-5">
        <div className="text-center">
          <span className="text-2xl font-bold text-white tabular-nums">{summary.projects_scanned}</span>
          <p className="text-[10px] text-gray-500 uppercase tracking-wider mt-0.5">
            {t('firstRun.projects', 'projects')}
          </p>
        </div>
        <div className="text-center">
          <span className="text-2xl font-bold text-white tabular-nums">{summary.total_dependencies}</span>
          <p className="text-[10px] text-gray-500 uppercase tracking-wider mt-0.5">
            {t('firstRun.dependencies', 'dependencies')}
          </p>
        </div>
      </div>

      {/* Primary stack */}
      {summary.primary_stack && (
        <div className="mb-5 px-4 py-3 bg-bg-secondary rounded-lg border border-border">
          <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-1.5">
            {t('firstRun.primaryStack', 'Primary stack')}
          </p>
          <p className="text-sm text-white font-medium">{summary.primary_stack}</p>
        </div>
      )}

      {/* Ecosystem breakdown */}
      {ecosystems.length > 0 && (
        <div className="flex flex-wrap justify-center gap-2 mb-5">
          {ecosystems.map(({ label, count }) => (
            <span key={label} className="px-2.5 py-1 text-xs bg-bg-secondary text-gray-300 rounded-lg border border-border">
              {label} <span className="text-gray-500">{count}</span>
            </span>
          ))}
        </div>
      )}

      {/* What I'll watch for */}
      {summary.key_packages.length > 0 && (
        <div className="mb-5 px-4 py-3 bg-bg-secondary rounded-lg border border-orange-500/10 text-left">
          <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-2">
            {t('firstRun.watchingFor', "I'll watch for")}
          </p>
          <p className="text-xs text-gray-400 leading-relaxed">
            {t('firstRun.watchingDescription', {
              packages: summary.key_packages.slice(0, 5).join(', '),
              defaultValue: `Security advisories, breaking changes, and updates for ${summary.key_packages.slice(0, 5).join(', ')}`,
            })}
          </p>
        </div>
      )}

      {/* Loading indicator */}
      <p className="text-xs text-gray-600 animate-pulse">
        {t('firstRun.startingAnalysis', 'Starting content analysis...')}
      </p>
    </div>
  );
}

// ============================================================================
// Stack-specific celebration insights
// ============================================================================

function buildStackInsights(
  results: Array<{ relevant: boolean; title: string; score_breakdown?: { dep_match_score?: number; matched_deps?: string[]; skill_gap_boost?: number } }>,
  scanSummary: ScanSummary | null,
): string[] {
  const insights: string[] = [];

  // Count dep-matched results
  const depMatches = results.filter(r => r.relevant && r.score_breakdown?.dep_match_score && r.score_breakdown.dep_match_score > 0);
  if (depMatches.length > 0) {
    const uniqueDeps = new Set(depMatches.flatMap(r => r.score_breakdown?.matched_deps || []));
    if (uniqueDeps.size > 0) {
      const depList = Array.from(uniqueDeps).slice(0, 3).join(', ');
      insights.push(`${depMatches.length} articles about your dependencies: ${depList}`);
    }
  }

  // Stack-specific count
  if (scanSummary?.primary_stack) {
    const stackTerms = scanSummary.primary_stack.toLowerCase().split(' + ');
    const stackMatches = results.filter(r => r.relevant && stackTerms.some(term => r.title.toLowerCase().includes(term)));
    if (stackMatches.length > 0) {
      insights.push(`${stackMatches.length} results relevant to your ${scanSummary.primary_stack} stack`);
    }
  }

  // Skill gap matches
  const gapMatches = results.filter(r => r.relevant && r.score_breakdown?.skill_gap_boost && r.score_breakdown.skill_gap_boost > 0);
  if (gapMatches.length > 0) {
    insights.push(`${gapMatches.length} items about dependencies you haven't explored yet`);
  }

  return insights;
}
