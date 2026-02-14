import { useState, useEffect, useCallback, useMemo, Component, ErrorInfo, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';
import { SplashScreen } from './components/SplashScreen';
import { Onboarding } from './components/Onboarding';
import { SettingsModal } from './components/SettingsModal';
import { VoidEngine } from './components/void-engine/VoidEngine';
import { OllamaStatus } from './components/OllamaStatus';
import { SignalsPanel } from './components/SignalsPanel';
import { NaturalLanguageSearch } from './components/NaturalLanguageSearch';
import { ToastContainer } from './components/Toast';
import { LearningIndicator } from './components/LearningIndicator';
import { BriefingView } from './components/BriefingView';
import { ResultsView } from './components/ResultsView';
import { AudioBriefing } from './components/AudioBriefing';
import { PredictiveIndicator } from './components/PredictiveIndicator';
import { SignalChainsPanel } from './components/SignalChains';
import { KnowledgeGapsPanel } from './components/KnowledgeGapsPanel';
import { ContextHandoff } from './components/ContextHandoff';
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
import { useAppStore } from './store';
import type { SourceRelevance } from './types';

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
  const activeView = useAppStore(s => s.activeView);
  const setActiveView = useAppStore(s => s.setActiveView);
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
    learnedAffinities,
    antiTopics,
    lastLearnedTopic,
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
      if (r.top_score >= 0.72) topCount++;
    }
    return { relevantCount, topCount, total: state.relevanceResults.length };
  }, [state.analysisComplete, state.loading, state.relevanceResults]);

  // AI Briefing (extracted hook)
  const {
    aiBriefing,
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
    onToggleBriefing: () => setActiveView(activeView === 'briefing' ? 'results' : 'briefing'),
    onOpenSettings: () => setShowSettings(true),
    onEscape: () => {
      if (showKeyboardHelp) { setShowKeyboardHelp(false); return; }
      if (showSettings) { setShowSettings(false); return; }
      if (expandedItem !== null) { setExpandedItem(null); return; }
    },
    onHelp: () => setShowKeyboardHelp(true),
    analyzeDisabled: state.loading,
    briefingAvailable: true,
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
            <OllamaStatus provider={settingsForm.provider} />
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
              <AudioBriefing />
              <ContextHandoff onStatus={(msg) => addToast(msg.includes('fail') ? 'error' : 'success', msg)} />
              {/* Export / Share buttons */}
              {state.analysisComplete && (
                <>
                  <button
                    onClick={async () => {
                      try {
                        const md = await invoke<string>('export_results', { format: 'markdown' });
                        await window.navigator.clipboard.writeText(md);
                        addToast('success', 'Results copied to clipboard');
                      } catch (e) {
                        addToast('error', `Export failed: ${e}`);
                      }
                    }}
                    className="w-10 h-10 rounded-lg flex items-center justify-center bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300 transition-all"
                    title="Copy results (Markdown)"
                  >
                    ↗
                  </button>
                  <button
                    onClick={async () => {
                      try {
                        const digest = await invoke<string>('export_results', { format: 'digest' });
                        await window.navigator.clipboard.writeText(digest);
                        addToast('success', 'Shareable digest copied to clipboard');
                      } catch (e) {
                        addToast('error', `Digest export failed: ${e}`);
                      }
                    }}
                    className="w-10 h-10 rounded-lg flex items-center justify-center bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-[#D4AF37] transition-all"
                    title="Copy shareable digest"
                  >
                    📋
                  </button>
                </>
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

        {/* Learning Indicator - Visible Learning Loop */}
        <LearningIndicator
          learnedAffinities={learnedAffinities}
          antiTopics={antiTopics}
          lastLearnedTopic={lastLearnedTopic}
        />

        {/* Predictive Context */}
        <PredictiveIndicator />

        {/* Actionable Signals */}
        {state.analysisComplete && (
          <>
            <SignalsPanel results={state.relevanceResults} />
            <SignalChainsPanel />
            <KnowledgeGapsPanel />
          </>
        )}

        {/* Natural Language Search */}
        <div className="mb-6">
          <NaturalLanguageSearch defaultExpanded={true} />
        </div>

        {/* View Tab Bar */}
        <div className="mb-6 flex items-center gap-1 bg-[#141414] rounded-lg p-1 border border-[#2A2A2A] w-fit">
          <button
            onClick={() => setActiveView('briefing')}
            className={`px-4 py-2 text-sm rounded-md transition-all ${
              activeView === 'briefing'
                ? 'bg-orange-500/20 text-orange-400 font-medium'
                : 'text-gray-500 hover:text-gray-300'
            }`}
          >
            Intelligence
          </button>
          <button
            onClick={() => setActiveView('results')}
            className={`px-4 py-2 text-sm rounded-md transition-all ${
              activeView === 'results'
                ? 'bg-orange-500/20 text-orange-400 font-medium'
                : 'text-gray-500 hover:text-gray-300'
            }`}
          >
            All Results
          </button>
        </div>

        {/* Conditional View Rendering */}
        {activeView === 'briefing' ? (
          <BriefingView
            briefing={aiBriefing}
            results={state.relevanceResults}
            onGenerateBriefing={generateBriefing}
            onRecordInteraction={recordInteraction}
            feedbackGiven={feedbackGiven}
          />
        ) : (
          <ResultsView
            state={state}
            filteredResults={filteredResults}
            feedbackGiven={feedbackGiven}
            discoveredContext={discoveredContext}
            expandedItem={expandedItem}
            setExpandedItem={setExpandedItem}
            loadContextFiles={loadContextFiles}
            clearContext={clearContext}
            indexContext={indexContext}
            recordInteraction={recordInteraction}
            newItemIds={newItemIds}
            focusedIndex={focusedIndex}
            sourceFilters={sourceFilters}
            sortBy={sortBy}
            showOnlyRelevant={showOnlyRelevant}
            showSavedOnly={showSavedOnly}
            searchQuery={searchQuery}
            setSortBy={setSortBy}
            setShowOnlyRelevant={setShowOnlyRelevant}
            setShowSavedOnly={setShowSavedOnly}
            setSearchQuery={setSearchQuery}
            toggleSourceFilter={toggleSourceFilter}
            dismissAllBelow={dismissAllBelow}
            saveAllAbove={saveAllAbove}
            renderLimit={renderLimit}
            setRenderLimit={setRenderLimit}
          />
        )}

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
                  { key: 'B', label: 'Switch view (Intelligence / Results)' },
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
