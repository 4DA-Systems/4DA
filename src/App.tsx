// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1). See LICENSE file.

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
import { ActionBar } from './components/ActionBar';
import { PredictiveIndicator } from './components/PredictiveIndicator';
import { SignalChainsPanel } from './components/SignalChains';
import { KnowledgeGapsPanel } from './components/KnowledgeGapsPanel';
import { KeyboardShortcutsModal } from './components/KeyboardShortcutsModal';
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

        // No cached results — auto-trigger analysis immediately
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
    onToggleFilters: () => setShowOnlyRelevant(!showOnlyRelevant),
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
        <SplashScreen onComplete={() => setShowSplash(false)} minimumDisplayTime={800} />
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
              <p className="text-gray-500 text-sm">All signal. No feed.</p>
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

        {/* Action Bar */}
        <ActionBar
          state={state}
          settings={settings}
          aiBriefing={aiBriefing}
          autoBriefingEnabled={autoBriefingEnabled}
          summaryBadges={summaryBadges}
          onAnalyze={() => { setNewItemIds(new Set()); startAnalysis(); }}
          onGenerateBriefing={generateBriefing}
          onToggleAutoBriefing={() => setAutoBriefingEnabled(!autoBriefingEnabled)}
          onToast={addToast}
        />

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
          <p className="text-xs text-gray-600">All signal. No feed.</p>
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
          <KeyboardShortcutsModal onClose={() => setShowKeyboardHelp(false)} />
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
