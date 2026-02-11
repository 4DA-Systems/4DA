import { useState, useEffect, useCallback, useMemo, Component, ErrorInfo, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';
import { SplashScreen } from './components/SplashScreen';
import { Onboarding } from './components/Onboarding';
import { ResultItem } from './components/ResultItem';
import { SettingsModal } from './components/SettingsModal';
import { VoidEngine } from './components/void-engine/VoidEngine';
import { SignalsPanel } from './components/SignalsPanel';
import { NaturalLanguageSearch } from './components/NaturalLanguageSearch';
import { ToastContainer } from './components/Toast';
import {
  useSettings,
  useMonitoring,
  useAnalysis,
  useContextDiscovery,
  useFeedback,
  useSystemHealth,
  useUserContext,
  useResultFilters,
  useBriefing,
  useKeyboardShortcuts,
  useToasts,
} from './hooks';
import { getStageLabel } from './utils/score';
import type { SourceRelevance } from './types';

const SOURCE_LABELS: Record<string, string> = {
  hackernews: 'HN', arxiv: 'arXiv', reddit: 'Reddit',
  github: 'GitHub', rss: 'RSS', youtube: 'YouTube',
  twitter: 'Twitter', producthunt: 'PH',
  lobsters: 'Lobsters', devto: 'Dev.to',
};

// Error Boundary to catch rendering errors
interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('React Error Boundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          backgroundColor: '#0A0A0A',
          color: '#fff',
          minHeight: '100vh',
          padding: '2rem',
          fontFamily: 'Inter, sans-serif',
        }}>
          <h1 style={{ color: '#EF4444' }}>Something went wrong</h1>
          <pre style={{
            backgroundColor: '#141414',
            padding: '1rem',
            borderRadius: '8px',
            overflow: 'auto',
            color: '#A0A0A0',
          }}>
            {this.state.error?.message}
            {'\n\n'}
            {this.state.error?.stack}
          </pre>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '1rem',
              padding: '0.5rem 1rem',
              backgroundColor: '#2A2A2A',
              color: '#fff',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer',
            }}
          >
            Reload App
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

function App() {
  // Local UI state
  const [showSplash, setShowSplash] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [renderLimit, setRenderLimit] = useState(50);
  const [newItemIds, setNewItemIds] = useState<Set<number>>(new Set());
  // Toast notification system
  const { toasts, addToast, removeToast } = useToasts();
  // All application state via hooks
  const {
    settings,
    settingsForm,
    setSettingsStatus,
    showOnboarding,
    setShowOnboarding,
    loadSettings,
    checkOllamaStatus,
  } = useSettings();

  const {
    monitoring,
  } = useMonitoring();

  const handleBackgroundItems = useCallback((itemIds: number[]) => {
    setNewItemIds(prev => {
      const next = new Set(prev);
      for (const id of itemIds) next.add(id);
      return next;
    });
    // Auto-clear "New" badges after 60 seconds
    setTimeout(() => setNewItemIds(new Set()), 60000);
  }, []);

  const {
    state,
    setState,
    expandedItem,
    setExpandedItem,
    isBrowserMode,
    loadContextFiles,
    clearContext,
    indexContext,
    startAnalysis,
  } = useAnalysis(addToast, handleBackgroundItems);

  // Immediate score adjustment for feedback (save boosts, dismiss sinks)
  const handleScoreAdjust = useCallback((itemId: number, delta: number) => {
    setState(s => ({
      ...s,
      relevanceResults: s.relevanceResults
        .map(r => r.id === itemId ? { ...r, top_score: Math.max(0, Math.min(1, r.top_score + delta)) } : r)
        .sort((a, b) => b.top_score - a.top_score),
    }));
  }, [setState]);

  const {
    discoveredContext,
    loadDiscoveredContext,
  } = useContextDiscovery(setSettingsStatus);

  const {
    feedbackGiven,
    recordInteraction,
  } = useFeedback(setSettingsStatus, handleScoreAdjust, addToast);

  // System health hook - data loaded on mount, consumed by SettingsModal via Zustand
  useSystemHealth(setSettingsStatus);

  const {
    loadUserContext,
  } = useUserContext(setSettingsStatus);

  const {
    sourceFilters,
    sortBy,
    setSortBy,
    showOnlyRelevant,
    setShowOnlyRelevant,
    searchQuery,
    setSearchQuery,
    showSavedOnly,
    setShowSavedOnly,
    toggleSourceFilter,
    filteredResults,
    dismissAllBelow,
    saveAllAbove,
  } = useResultFilters(state.relevanceResults, feedbackGiven, recordInteraction, setSettingsStatus);

  // Check Ollama status when provider changes to "ollama"
  useEffect(() => {
    if (settingsForm.provider === 'ollama') {
      checkOllamaStatus(settingsForm.baseUrl || undefined);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only re-check when provider changes, not on every baseUrl keystroke
  }, [settingsForm.provider, checkOllamaStatus]);

  const summaryBadges = useMemo(() => {
    if (!state.analysisComplete || state.loading) return null;
    let relevantCount = 0, topCount = 0;
    for (const r of state.relevanceResults) {
      if (r.relevant) relevantCount++;
      if (r.top_score >= 0.6) topCount++;
    }
    return { relevantCount, topCount, total: state.relevanceResults.length };
  }, [state.analysisComplete, state.loading, state.relevanceResults]);

  // AI Briefing (extracted hook)
  const {
    aiBriefing,
    showBriefing,
    setShowBriefing,
    autoBriefingEnabled,
    setAutoBriefingEnabled,
    generateBriefing,
  } = useBriefing(state.relevanceResults, state.analysisComplete);

  // Reset render limit when new results come in
  useEffect(() => { setRenderLimit(50); }, [state.relevanceResults]);

  // On mount: load cached results from previous session, or auto-analyze
  useEffect(() => {
    let cancelled = false;
    const loadOrAnalyze = async () => {
      try {
        // First, try to load cached results from AnalysisState
        const analysisState = await invoke<{
          running: boolean;
          completed: boolean;
          results: SourceRelevance[] | null;
        }>('get_analysis_status');

        if (cancelled) return;

        if (analysisState.results && analysisState.results.length > 0) {
          // Restore previous results
          const results = analysisState.results;
          const relevantCount = results.filter(r => r.relevant).length;
          setState(s => ({
            ...s,
            relevanceResults: results,
            status: `${relevantCount}/${results.length} items relevant (cached)`,
            analysisComplete: true,
            loading: false,
          }));
          return;
        }

        // No cached results — auto-trigger analysis after 3s
        await new Promise(resolve => setTimeout(resolve, 3000));
        if (cancelled) return;
        await invoke('run_cached_analysis');
      } catch {
        // Silently ignore failures
      }
    };
    loadOrAnalyze();

    return () => { cancelled = true; };
  }, [setState]);

  // Global keyboard shortcuts (extracted hook)
  const visibleResults = filteredResults.slice(0, renderLimit);
  const { focusedIndex } = useKeyboardShortcuts({
    onAnalyze: startAnalysis,
    onToggleFilters: () => setShowOnlyRelevant(prev => !prev),
    onToggleBriefing: () => setShowBriefing(prev => !prev),
    onOpenSettings: () => setShowSettings(true),
    onEscape: () => {
      if (showKeyboardHelp) { setShowKeyboardHelp(false); return; }
      if (showSettings) { setShowSettings(false); return; }
      if (showBriefing) { setShowBriefing(false); return; }
      if (expandedItem !== null) { setExpandedItem(null); return; }
    },
    onHelp: () => setShowKeyboardHelp(true),
    analyzeDisabled: state.loading,
    briefingAvailable: !!aiBriefing.content,
    filtersAvailable: state.analysisComplete,
    resultCount: visibleResults.length,
    onFocusResult: (index: number) => {
      const el = document.getElementById(`result-item-${visibleResults[index]?.id}`);
      el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    },
    onToggleExpandResult: (index: number) => {
      const item = visibleResults[index];
      if (item) setExpandedItem(expandedItem === item.id ? null : item.id);
    },
    onOpenResult: (index: number) => {
      const item = visibleResults[index];
      if (item?.url) window.open(item.url, '_blank', 'noopener,noreferrer');
    },
  });

  return (
    <>
      {/* Splash Screen */}
      {showSplash && (
        <SplashScreen onComplete={() => setShowSplash(false)} minimumDisplayTime={2500} />
      )}

      {/* Onboarding Flow (first run) */}
      {!showSplash && showOnboarding && (
        <Onboarding onComplete={() => {
          setShowOnboarding(false);
          loadSettings();
          loadUserContext();
          loadDiscoveredContext();
        }} />
      )}

      <div className={`min-h-screen bg-[#0A0A0A] text-white p-6 ${showSplash || showOnboarding ? 'hidden' : 'opacity-100 transition-opacity duration-300'}`}>
        {/* Header - Polished */}
        <header className="mb-8 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 flex items-center justify-center flex-shrink-0">
              <VoidEngine size={48} />
            </div>
            <div>
              <h1 className="text-2xl font-semibold tracking-tight text-white">4DA</h1>
              <p className="text-gray-500 text-sm">The internet searches for you</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {monitoring?.enabled && (
              <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/30 rounded-lg">
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                <span className="text-xs text-green-400 font-medium">Live</span>
              </div>
            )}
            <button
              onClick={() => setShowSettings(true)}
              className="px-4 py-2 text-sm bg-[#141414] text-gray-300 border border-[#2A2A2A] rounded-lg hover:bg-[#1F1F1F] hover:border-orange-500/30 transition-all"
            >
              ⚙️ Settings
            </button>
          </div>
        </header>

        {/* Browser Mode Notice */}
        {isBrowserMode && (
          <div className="mb-6 px-4 py-4 bg-[#141414] border border-[#2A2A2A] rounded-lg">
            <p className="text-sm font-medium text-white mb-2">Desktop App Required</p>
            <p className="text-xs text-gray-400">
              4DA runs as a desktop app to access your local files, monitor sources,
              and keep everything private on your machine.
            </p>
            <p className="text-xs text-gray-500 mt-2">
              Run <code className="text-orange-400">npm run tauri dev</code> or launch the installed app.
            </p>
          </div>
        )}

        {/* Action Bar - Polished Card Style */}
        <div className="mb-6 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
          {/* Main Action Row */}
          <div className="px-5 py-4 flex items-center gap-4">
            {/* Status */}
            <div className="flex items-center gap-3 flex-1 min-w-0">
              {state.loading ? (
                <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                </div>
              ) : (
                <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center flex-shrink-0">
                  <span className="text-gray-500">📊</span>
                </div>
              )}
              <div className="min-w-0">
                <p className="text-sm text-white font-medium truncate">
                  {state.loading ? 'Analyzing...' : state.analysisComplete ? 'Analysis Complete' : 'Ready to analyze'}
                </p>
                <p className="text-xs text-gray-500 truncate">
                  {state.status}
                  {state.lastAnalyzedAt && !state.loading && (
                    <span className="ml-2 text-gray-600">
                      · {state.lastAnalyzedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                  )}
                </p>
              </div>
            </div>

            {/* LLM Badge */}
            {settings?.rerank.enabled && settings?.llm.has_api_key && (
              <div className="px-3 py-1.5 bg-orange-500/10 text-orange-400 text-xs rounded-lg border border-orange-500/20">
                🤖 LLM Active
              </div>
            )}

            {/* Summary Badges */}
            {summaryBadges && (
              <div className="flex items-center gap-1.5">
                <span className="px-2 py-1 text-[11px] bg-[#1F1F1F] text-gray-400 rounded-lg font-mono">
                  {summaryBadges.total}
                </span>
                <span className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg font-mono">
                  {summaryBadges.relevantCount} rel
                </span>
                {summaryBadges.topCount > 0 && (
                  <span className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg font-mono">
                    {summaryBadges.topCount} top
                  </span>
                )}
              </div>
            )}

            {/* Actions */}
            <div className="flex items-center gap-2">
              <button
                onClick={() => { setNewItemIds(new Set()); startAnalysis(); }}
                disabled={state.loading}
                className="px-5 py-2.5 text-sm bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 transition-all disabled:opacity-50 disabled:cursor-not-allowed hover:scale-105 active:scale-95"
              >
                {state.loading ? 'Analyzing...' : '🔍 Analyze'}
              </button>
              {state.loading && (
                <button
                  onClick={() => invoke('cancel_analysis')}
                  className="px-3 py-2.5 text-sm bg-[#1F1F1F] text-red-400 border border-red-500/30 font-medium rounded-lg hover:bg-red-500/10 transition-all"
                >
                  Cancel
                </button>
              )}
              <button
                onClick={generateBriefing}
                disabled={aiBriefing.loading || state.relevanceResults.length === 0}
                className="px-4 py-2.5 text-sm bg-[#1F1F1F] text-orange-400 border border-orange-500/30 font-medium rounded-lg hover:bg-orange-500/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed flex items-center gap-2"
                title={state.relevanceResults.length === 0 ? 'Run analysis first' : 'Generate AI insights'}
              >
                {aiBriefing.loading ? (
                  <>
                    <div className="w-3 h-3 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
                    Thinking...
                  </>
                ) : (
                  <>✨ AI Brief</>
                )}
              </button>
              <button
                onClick={() => setAutoBriefingEnabled(!autoBriefingEnabled)}
                className={`w-10 h-10 rounded-lg flex items-center justify-center transition-all ${
                  autoBriefingEnabled
                    ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                    : 'bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300'
                }`}
                title={autoBriefingEnabled ? 'Auto-briefing ON' : 'Auto-briefing OFF'}
              >
                {autoBriefingEnabled ? '⚡' : '○'}
              </button>
              {/* Export button */}
              {state.analysisComplete && (
                <button
                  onClick={async () => {
                    try {
                      const md = await invoke<string>('export_results', { format: 'markdown' });
                      await window.navigator.clipboard.writeText(md);
                      addToast('success', 'Results copied to clipboard (Markdown)');
                    } catch (e) {
                      addToast('error', `Export failed: ${e}`);
                    }
                  }}
                  className="w-10 h-10 rounded-lg flex items-center justify-center bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300 transition-all"
                  title="Copy results to clipboard (Markdown)"
                >
                  ↗
                </button>
              )}
            </div>
          </div>

          {/* Progress Bar */}
          {state.loading && state.progress > 0 && (
            <div className="px-5 pb-4">
              <div className="flex justify-between text-xs text-gray-500 mb-2">
                <span>{getStageLabel(state.progressStage)}</span>
                <span>{Math.round(state.progress * 100)}%</span>
              </div>
              <div className="w-full h-2 bg-[#1F1F1F] rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out rounded-full"
                  style={{ width: `${state.progress * 100}%` }}
                />
              </div>
            </div>
          )}

          {/* AI Briefing Error */}
          {aiBriefing.error && (
            <div className="mx-5 mb-4 p-3 bg-red-900/20 border border-red-500/30 rounded-lg text-red-300 text-sm flex items-center gap-2">
              <span>⚠️</span>
              {aiBriefing.error}
            </div>
          )}
        </div>

        {/* AI Briefing Panel - Polished */}
        {showBriefing && aiBriefing.content && (
          <div className="mb-6 bg-[#141414] rounded-lg border border-orange-500/30 overflow-hidden">
            <div className="px-5 py-4 border-b border-orange-500/20 flex items-center justify-between bg-orange-500/5">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                  <span className="text-orange-400">✨</span>
                </div>
                <div>
                  <h2 className="font-medium text-orange-400">AI Briefing</h2>
                  {aiBriefing.model && (
                    <span className="text-xs text-gray-500">via {aiBriefing.model}</span>
                  )}
                </div>
              </div>
              <button
                onClick={() => setShowBriefing(false)}
                className="w-8 h-8 rounded-lg bg-[#1F1F1F] text-gray-500 hover:text-white hover:bg-[#2A2A2A] flex items-center justify-center transition-all"
                title="Close briefing"
              >
                ×
              </button>
            </div>
            <div className="p-5 text-sm text-gray-300 leading-relaxed whitespace-pre-wrap">
              {aiBriefing.content.split('\n').map((line, i) => {
                // Style headers
                if (line.startsWith('## ')) {
                  return (
                    <h3 key={i} className="text-orange-400 font-medium mt-4 mb-2 first:mt-0">
                      {line.replace('## ', '')}
                    </h3>
                  );
                }
                // Style list items
                if (line.startsWith('- ') || line.startsWith('* ')) {
                  return (
                    <p key={i} className="ml-4 my-1">
                      <span className="text-orange-400 mr-2">•</span>
                      {line.replace(/^[-*] /, '')}
                    </p>
                  );
                }
                // Style numbered items
                if (/^\d+\. /.test(line)) {
                  return (
                    <p key={i} className="ml-4 my-1">
                      <span className="text-orange-400 mr-2">{line.match(/^\d+/)?.[0]}.</span>
                      {line.replace(/^\d+\. /, '')}
                    </p>
                  );
                }
                // Empty lines
                if (!line.trim()) {
                  return <div key={i} className="h-2" />;
                }
                // Regular text
                return <p key={i} className="my-1">{line}</p>;
              })}
            </div>
            {aiBriefing.lastGenerated && (
              <div className="px-5 py-3 border-t border-[#2A2A2A] text-xs text-gray-500">
                Generated {aiBriefing.lastGenerated.toLocaleTimeString()}
              </div>
            )}
          </div>
        )}

        {/* Actionable Signals */}
        {state.analysisComplete && (
          <SignalsPanel results={state.relevanceResults} />
        )}

        {/* Natural Language Search */}
        <div className="mb-6">
          <NaturalLanguageSearch defaultExpanded={true} />
        </div>

        {/* Main Content */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Context Files Panel - Polished */}
          <section className="bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
            <div className="px-5 py-4 border-b border-[#2A2A2A] flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
                  <span className="text-gray-500">📁</span>
                </div>
                <div>
                  <h2 className="font-medium text-white">Context</h2>
                  <p className="text-xs text-gray-500">{state.contextFiles.length} files indexed</p>
                </div>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={loadContextFiles}
                  className="w-8 h-8 flex items-center justify-center text-sm bg-[#1F1F1F] text-gray-400 rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-all"
                  title="Reload files"
                >
                  ↻
                </button>
                {state.contextFiles.length > 0 && (
                  <>
                    <button
                      onClick={indexContext}
                      disabled={state.loading}
                      className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all disabled:opacity-50"
                      title="Index files"
                    >
                      Index
                    </button>
                    <button
                      onClick={clearContext}
                      className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
                      title="Clear"
                    >
                      Clear
                    </button>
                  </>
                )}
              </div>
            </div>
            <div className="p-4 max-h-[calc(100vh-320px)] overflow-y-auto">
              {state.contextFiles.length === 0 ? (
                <div className="text-center py-8 px-4">
                  <div className="w-12 h-12 mx-auto mb-3 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                    <span className="text-2xl">📂</span>
                  </div>
                  <p className="text-gray-400 text-sm mb-1">No context files yet</p>
                  <p className="text-xs text-gray-600">Add files to your context directory to enable personalized analysis.</p>
                </div>
              ) : (
                <ul className="space-y-2">
                  {state.contextFiles.map((file) => (
                    <li
                      key={file.path}
                      className="px-3 py-2 bg-[#1F1F1F] rounded-lg border border-[#2A2A2A] hover:border-orange-500/30 transition-all"
                    >
                      <div className="font-mono text-white text-sm truncate">
                        {file.path.split('/').pop()?.split('\\').pop()}
                      </div>
                      <div className="text-xs text-gray-500 mt-1">{file.lines} lines</div>
                    </li>
                  ))}
                </ul>
              )}

              {/* ACE Discovered Context */}
              {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
                <div className="mt-4 pt-4 border-t border-[#2A2A2A]">
                  <div className="text-xs text-gray-500 mb-3 flex items-center gap-2">
                    <span>🔍 Auto-Discovered</span>
                    <span className="px-1.5 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded" title="Auto Context Engine - score boost from your local project context">ACE</span>
                  </div>
                  {discoveredContext.tech.length > 0 && (
                    <div className="mb-3">
                      <div className="flex flex-wrap gap-1.5">
                        {discoveredContext.tech.slice(0, 6).map((tech) => (
                          <span
                            key={tech.name}
                            className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg border border-green-500/20"
                            title={`${tech.category} - ${Math.round(tech.confidence * 100)}%`}
                          >
                            {tech.name}
                          </span>
                        ))}
                        {discoveredContext.tech.length > 6 && (
                          <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.tech.length - 6}</span>
                        )}
                      </div>
                    </div>
                  )}
                  {discoveredContext.topics.length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {discoveredContext.topics.slice(0, 4).map((topic) => (
                        <span
                          key={topic}
                          className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20"
                        >
                          {topic}
                        </span>
                      ))}
                      {discoveredContext.topics.length > 4 && (
                        <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.topics.length - 4}</span>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          </section>

          {/* Relevance Results Panel - Polished */}
          <section className="lg:col-span-2 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
            <div className="px-5 py-4 border-b border-[#2A2A2A]">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
                    <span className="text-gray-500">📊</span>
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <h2 className="font-medium text-white">Results</h2>
                      {newItemIds.size > 0 && (
                        <span className="px-2 py-0.5 text-[10px] bg-blue-500/20 text-blue-400 rounded-full font-medium animate-pulse">
                          {newItemIds.size} new
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-gray-500">
                      {state.analysisComplete
                        ? `${filteredResults.length} items • ${filteredResults.filter((r) => r.relevant).length} relevant`
                        : 'Click Analyze to find relevant content'}
                    </p>
                    {state.analysisComplete && filteredResults.length > renderLimit && (
                      <span className="text-xs text-[#666666]">
                        Showing {Math.min(renderLimit, filteredResults.length)} of {filteredResults.length}
                      </span>
                    )}
                  </div>
                </div>
                {state.analysisComplete && (
                  <div className="flex items-center gap-2">
                    {filteredResults.filter((r) => r.top_score >= 0.6).length > 0 && (
                      <span className="text-xs px-2 py-1 bg-orange-500/10 text-orange-400 rounded-lg">
                        ⭐ {filteredResults.filter((r) => r.top_score >= 0.6).length} top picks
                      </span>
                    )}
                    <span className="text-xs px-2 py-1 bg-green-500/10 text-green-400 rounded-lg">
                      {filteredResults.filter((r) => r.relevant).length} relevant
                    </span>
                  </div>
                )}
              </div>

              {/* Filter Bar - Polished */}
              {state.analysisComplete && (
                <div className="flex flex-wrap items-center gap-3 pt-3 border-t border-[#2A2A2A]" role="toolbar" aria-label="Filter and sort controls">
                  {/* Search */}
                  <div className="relative">
                    <input
                      type="text"
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      placeholder="Search results..."
                      aria-label="Search results by keyword"
                      className="bg-[#1F1F1F] text-sm text-white placeholder-gray-500 rounded-lg pl-8 pr-3 py-1.5 w-48 border border-transparent focus:border-[#2A2A2A] focus:outline-none transition-all"
                    />
                    <svg className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                    {searchQuery && (
                      <button
                        onClick={() => setSearchQuery('')}
                        className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
                        aria-label="Clear search"
                      >
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    )}
                  </div>

                  {/* Source Filters - dynamic based on results */}
                  <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg flex-wrap" role="group" aria-label="Source filters">
                    <span className="text-xs text-gray-500">Sources:</span>
                    {[...new Set(state.relevanceResults.map(r => r.source_type || 'hackernews'))]
                      .sort((a, b) => (SOURCE_LABELS[a] || a).localeCompare(SOURCE_LABELS[b] || b))
                      .map(id => (
                        <button
                          key={id}
                          onClick={() => toggleSourceFilter(id)}
                          aria-pressed={sourceFilters.has(id)}
                          aria-label={`Filter ${SOURCE_LABELS[id] || id} source`}
                          className={`px-2 py-1 text-xs rounded-lg transition-all ${
                            sourceFilters.has(id)
                              ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                              : 'text-gray-500 hover:text-gray-300'
                          }`}
                        >
                          {SOURCE_LABELS[id] || id}
                        </button>
                      ))}
                  </div>

                  {/* Sort */}
                  <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg" role="group" aria-label="Sort order">
                    <span className="text-xs text-gray-500">Sort:</span>
                    <button
                      onClick={() => setSortBy('score')}
                      aria-pressed={sortBy === 'score'}
                      className={`px-2 py-1 text-xs rounded-lg transition-all ${
                        sortBy === 'score'
                          ? 'bg-white/10 text-white'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    >
                      Score
                    </button>
                    <button
                      onClick={() => setSortBy('date')}
                      aria-pressed={sortBy === 'date'}
                      className={`px-2 py-1 text-xs rounded-lg transition-all ${
                        sortBy === 'date'
                          ? 'bg-white/10 text-white'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    >
                      Recent
                    </button>
                  </div>

                  {/* Relevance Toggle */}
                  <button
                    onClick={() => { setShowOnlyRelevant(!showOnlyRelevant); if (!showOnlyRelevant) setShowSavedOnly(false); }}
                    aria-pressed={showOnlyRelevant}
                    aria-label="Toggle relevant items only"
                    className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                      showOnlyRelevant
                        ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                        : 'bg-[#1F1F1F] text-gray-500 hover:text-gray-300'
                    }`}
                  >
                    {showOnlyRelevant ? '✓ Relevant only' : 'Show all'}
                  </button>

                  {/* Saved Items Toggle */}
                  <button
                    onClick={() => { setShowSavedOnly(!showSavedOnly); if (!showSavedOnly) setShowOnlyRelevant(false); }}
                    aria-pressed={showSavedOnly}
                    aria-label="Show saved items only"
                    className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                      showSavedOnly
                        ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
                        : 'bg-[#1F1F1F] text-gray-500 hover:text-gray-300'
                    }`}
                  >
                    {showSavedOnly ? '✓ Saved' : 'Saved'}
                  </button>

                  {/* Spacer */}
                  <div className="flex-1" />

                  {/* Batch Operations */}
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => dismissAllBelow(0.3)}
                      className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
                      title="Dismiss all items below 30% relevance"
                    >
                      ✕ &lt;30%
                    </button>
                    <button
                      onClick={() => saveAllAbove(0.6)}
                      className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
                      title="Save all items above 60% relevance"
                    >
                      ✓ &gt;60%
                    </button>
                  </div>
                </div>
              )}
            </div>
            <div className="p-4 max-h-[calc(100vh-380px)] overflow-y-auto">
              {!state.analysisComplete ? (
                <div className="text-center py-16">
                  {state.loading ? (
                    <>
                      <div className="w-16 h-16 mx-auto mb-4 bg-orange-500/20 rounded-full flex items-center justify-center">
                        <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
                      </div>
                      <p className="text-lg text-white mb-2">Analyzing...</p>
                      <p className="text-sm text-gray-500">{state.progressMessage}</p>
                    </>
                  ) : (
                    <>
                      <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                        <span className="text-3xl">🔍</span>
                      </div>
                      <p className="text-lg text-white mb-2">Ready to search</p>
                      <p className="text-sm text-gray-500">
                        Click <span className="text-orange-400">Analyze</span> to find relevant content
                      </p>
                    </>
                  )}
                </div>
              ) : filteredResults.length === 0 ? (
                <div className="text-center py-16">
                  <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                    <span className="text-3xl">🤷</span>
                  </div>
                  <p className="text-lg text-white mb-2">No results match</p>
                  <p className="text-sm text-gray-500">
                    Try enabling more sources or showing all items
                  </p>
                </div>
              ) : (
                <>
                  <ul className="space-y-3">
                    {visibleResults.map((item, idx) => {
                      // Score group headers (only when sorting by score)
                      let groupHeader: string | null = null;
                      if (sortBy === 'score' && idx > 0) {
                        const prev = visibleResults[idx - 1];
                        if (prev.top_score >= 0.6 && item.top_score < 0.6) {
                          groupHeader = 'Relevant';
                        } else if (prev.top_score >= 0.35 && item.top_score < 0.35) {
                          groupHeader = 'Below Threshold';
                        }
                      } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.6) {
                        groupHeader = 'Top Picks';
                      } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.35) {
                        groupHeader = 'Relevant';
                      }
                      return (
                        <li key={item.id}>
                          {groupHeader && (
                            <div className="flex items-center gap-3 mb-3 mt-2 first:mt-0">
                              <span className={`text-xs font-medium px-2 py-1 rounded-lg ${
                                groupHeader === 'Top Picks' ? 'bg-orange-500/10 text-orange-400' :
                                groupHeader === 'Relevant' ? 'bg-green-500/10 text-green-400' :
                                'bg-gray-500/10 text-gray-500'
                              }`}>
                                {groupHeader}
                              </span>
                              <div className="flex-1 h-px bg-[#2A2A2A]" />
                            </div>
                          )}
                          <ResultItem
                            item={item}
                            isExpanded={expandedItem === item.id}
                            isFocused={focusedIndex === idx}
                            isNew={newItemIds.has(item.id)}
                            onToggleExpand={() => setExpandedItem(expandedItem === item.id ? null : item.id)}
                            feedbackGiven={feedbackGiven}
                            onRecordInteraction={recordInteraction}
                          />
                        </li>
                      );
                    })}
                  </ul>
                  {filteredResults.length > renderLimit && (
                    <button
                      onClick={() => setRenderLimit(prev => prev + 50)}
                      className="w-full mt-3 py-2 text-sm text-orange-400 bg-[#1A1A1A] border border-[#2A2A2A] rounded-lg hover:bg-[#1F1F1F] transition-colors"
                    >
                      Show more ({filteredResults.length - renderLimit} remaining)
                    </button>
                  )}
                </>
              )}
            </div>
          </section>
        </div>

        {/* Footer - Polished */}
        <footer className="mt-8 text-center space-y-1">
          <p className="text-xs text-gray-600">The internet searches for you</p>
          <p className="text-[10px] text-gray-700">
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">R</kbd> Analyze
            <span className="mx-1.5">·</span>
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">F</kbd> Filter
            <span className="mx-1.5">·</span>
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">B</kbd> Briefing
            <span className="mx-1.5">·</span>
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">,</kbd> Settings
            <span className="mx-1.5">·</span>
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">Esc</kbd> Close
            <span className="mx-1.5">·</span>
            <kbd className="px-1 py-0.5 bg-[#1F1F1F] rounded text-gray-500">?</kbd> Help
          </p>
        </footer>

        {/* Toast Notifications */}
        <ToastContainer toasts={toasts} onDismiss={removeToast} />

        {/* Keyboard Shortcuts Help Modal */}
        {showKeyboardHelp && (
          <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" onClick={() => setShowKeyboardHelp(false)}>
            <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl w-full max-w-sm shadow-2xl" onClick={e => e.stopPropagation()}>
              <div className="px-6 py-4 border-b border-[#2A2A2A] flex items-center justify-between">
                <h2 className="text-lg font-medium text-white">Keyboard Shortcuts</h2>
                <button
                  onClick={() => setShowKeyboardHelp(false)}
                  className="w-8 h-8 rounded-lg bg-[#1F1F1F] text-gray-500 hover:text-white hover:bg-[#2A2A2A] flex items-center justify-center transition-all"
                >
                  &times;
                </button>
              </div>
              <div className="p-6 space-y-3">
                {[
                  { key: 'R', label: 'Run analysis' },
                  { key: 'F', label: 'Toggle relevant-only filter' },
                  { key: 'B', label: 'Toggle AI briefing' },
                  { key: ',', label: 'Open settings' },
                  { key: 'Esc', label: 'Close panel / modal' },
                  { key: '?', label: 'Show this help' },
                ].map(({ key, label }) => (
                  <div key={key} className="flex items-center justify-between">
                    <kbd className="px-2 py-1 bg-[#1F1F1F] border border-[#2A2A2A] rounded text-sm font-mono text-white min-w-[2.5rem] text-center">
                      {key}
                    </kbd>
                    <span className="text-sm text-[#A0A0A0]">{label}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Settings Modal - now self-sufficient via Zustand store */}
        {showSettings && (
          <SettingsModal
            onClose={() => setShowSettings(false)}
          />
        )}
      </div>
    </>
  );
}

// Wrap App with ErrorBoundary
function AppWithErrorBoundary() {
  return (
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  );
}

export default AppWithErrorBoundary;
