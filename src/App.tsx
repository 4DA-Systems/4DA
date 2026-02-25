// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import './i18n';

import { useState, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './App.css';
import sunLogo from './assets/sun-logo.jpg';
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
import { SavedItemsView } from './components/SavedItemsView';
import { ActionBar } from './components/ActionBar';
import { PredictiveIndicator } from './components/PredictiveIndicator';
import { SignalChainsPanel } from './components/SignalChains';
import { KnowledgeGapsPanel } from './components/KnowledgeGapsPanel';
import { KeyboardShortcutsModal } from './components/KeyboardShortcutsModal';
import { ErrorBoundary } from './components/ErrorBoundary';
import { TechRadar } from './components/TechRadar';
import { DecisionMemory } from './components/DecisionMemory';
import { DelegationDashboard } from './components/DelegationDashboard';
import { AgentMemoryPanel } from './components/AgentMemoryPanel';
import { AutophagyInsights } from './components/AutophagyInsights';
import { ToolkitView } from './components/toolkit/ToolkitView';
import { PlaybookView } from './components/PlaybookView';
import { CoachView } from './components/coach/CoachView';
import { CommandDeck } from './components/command-deck/CommandDeck';
import { FirstRunTransition } from './components/FirstRunTransition';
import { ViewTabBar } from './components/ViewTabBar';
import { ProValueBadge } from './components/ProValueBadge';
import {
  useSettings,
  useMonitoring,
  useAnalysis,
  useContextDiscovery,
  useFeedback,
  useSystemHealth,
  useUserContext,
  useBriefing,
  useKeyboardShortcuts,
  useToasts,
  useLicense,
} from './hooks';
import { useShallow } from 'zustand/react/shallow';

import { useAppStore } from './store';
import { useUpdateCheck } from './hooks/use-update-check';
import { useDirection } from './i18n/rtl';
import type { SourceRelevance } from './types';

function App() {
  // Local UI state
  const [showSplash, setShowSplash] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [renderLimit, setRenderLimit] = useState(50);
  const [newItemIds, setNewItemIds] = useState<Set<number>>(new Set());
  // Data selectors (may change, use useShallow)
  const { activeView, showOnlyRelevant, filteredResults } = useAppStore(
    useShallow((s) => ({
      activeView: s.activeView,
      showOnlyRelevant: s.showOnlyRelevant,
      filteredResults: s.appState.relevanceResults,
    })),
  );

  // Action selectors (stable references, no need for useShallow)
  const setActiveView = useAppStore(s => s.setActiveView);
  const setShowOnlyRelevant = useAppStore(s => s.setShowOnlyRelevant);
  const loadPersistedBriefing = useAppStore(s => s.loadPersistedBriefing);
  const loadSourceHealth = useAppStore(s => s.loadSourceHealth);
  const loadLicense = useAppStore(s => s.loadLicense);
  const loadTrialStatus = useAppStore(s => s.loadTrialStatus);
  const loadProValueReport = useAppStore(s => s.loadProValueReport);

  // First-run state
  const isFirstRun = useAppStore(s => s.isFirstRun);
  const firstRunDismissed = useAppStore(s => s.firstRunDismissed);
  const setIsFirstRun = useAppStore(s => s.setIsFirstRun);
  const setFirstRunDismissed = useAppStore(s => s.setFirstRunDismissed);

  const { tier, isPro } = useLicense();

  // i18n direction (sets dir/lang on <html>)
  useDirection();

  // Toast notification system
  const { toasts, addToast, removeToast } = useToasts();

  // Auto-update check
  const { update, installing, installUpdate, dismiss: dismissUpdate } = useUpdateCheck();
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
    startAnalysis,
  } = useAnalysis(addToast, handleBackgroundItems);

  const {
    loadDiscoveredContext,
  } = useContextDiscovery(setSettingsStatus);

  // Feedback hook — all state and recordInteraction live in the store
  const {
    learnedAffinities,
    antiTopics,
    lastLearnedTopic,
  } = useFeedback();

  // System health hook - data loaded on mount, consumed by SettingsModal via Zustand
  useSystemHealth(setSettingsStatus);

  const {
    loadUserContext,
  } = useUserContext(setSettingsStatus);

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

  // Load persisted briefing + source health + license + pro value on mount (instant, from DB)
  useEffect(() => {
    loadPersistedBriefing();
    loadSourceHealth();
    loadLicense();
    loadTrialStatus();
    loadProValueReport();
  }, [loadPersistedBriefing, loadSourceHealth, loadLicense, loadTrialStatus, loadProValueReport]);

  // Deep-link handler: 4da://activate?key=...
  const activateLicense = useAppStore(s => s.activateLicense);
  const activateStreetsLicense = useAppStore(s => s.activateStreetsLicense);
  useEffect(() => {
    const unlisten = listen<string>('deep-link-activate', async (event) => {
      try {
        const url = new URL(event.payload);
        if (url.hostname === 'activate' || url.pathname === '/activate') {
          const key = url.searchParams.get('key');
          if (key) {
            // Try both activation paths — the backend figures out the tier
            const proOk = await activateLicense(key);
            const streetsOk = await activateStreetsLicense(key);
            if (proOk || streetsOk) {
              addToast('success', 'License activated successfully');
            } else {
              addToast('error', 'Invalid license key');
            }
          }
        }
      } catch {
        // Ignore malformed URLs
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, [activateLicense, activateStreetsLicense, addToast]);

  // On mount: load cached results from previous session, or auto-analyze
  useEffect(() => {
    let cancelled = false;
    let autoTimer: ReturnType<typeof setTimeout>;
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

        // No cached results — auto-trigger full analysis after splash settles
        // (Skip if first-run — FirstRunTransition handles analysis trigger)
        if (cancelled || useAppStore.getState().isFirstRun) return;
        autoTimer = setTimeout(() => {
          if (!cancelled) startAnalysis();
        }, 2000);
      } catch {
        // Silently ignore failures
      }
    };
    loadOrAnalyze();

    return () => { cancelled = true; clearTimeout(autoTimer); };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, []);

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
          setIsFirstRun(true);
          loadSettings();
          loadUserContext();
          loadDiscoveredContext();
        }} />
      )}

      {/* First-Run Transition (bridges onboarding to first analysis) */}
      {!showSplash && !showOnboarding && isFirstRun && !firstRunDismissed && (
        <FirstRunTransition onComplete={(view) => {
          setFirstRunDismissed(true);
          setActiveView(view);
        }} />
      )}

      <div className={`min-h-screen bg-bg-primary text-white p-6 ${showSplash || showOnboarding ? 'hidden' : 'opacity-100 transition-opacity duration-300'}`}>
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-orange-500 focus:text-white focus:rounded-lg focus:text-sm focus:font-medium"
        >
          Skip to main content
        </a>
        {/* Header - Polished */}
        <header className="mb-8 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <img
              src={sunLogo}
              alt="4DA"
              className="w-10 h-10 rounded-lg object-cover flex-shrink-0"
            />
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
            <ProValueBadge />
            <span className={`px-2 py-1 text-[10px] font-bold uppercase tracking-wider rounded ${
              isPro
                ? 'bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30'
                : 'bg-bg-tertiary text-gray-500 border border-border'
            }`}>
              {tier}
            </span>
            <button
              onClick={() => setShowSettings(true)}
              className="px-4 py-2 text-sm bg-bg-secondary text-gray-300 border border-border rounded-lg hover:bg-bg-tertiary hover:border-orange-500/30 transition-all"
            >
              ⚙️ Settings
            </button>
          </div>
        </header>

        {/* Browser Mode Notice */}
        {isBrowserMode && (
          <div className="mb-6 px-4 py-4 bg-bg-secondary border border-border rounded-lg">
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

        <main id="main-content">
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
        <ViewTabBar />

        {/* Conditional View Rendering */}
        {activeView === 'briefing' ? (
          <BriefingView />
        ) : activeView === 'insights' ? (
          <div className="space-y-6">
            <TechRadar />
            <DecisionMemory />
            <AutophagyInsights />
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <DelegationDashboard />
              <AgentMemoryPanel />
            </div>
          </div>
        ) : activeView === 'saved' ? (
          <SavedItemsView />
        ) : activeView === 'toolkit' ? (
          <ToolkitView />
        ) : activeView === 'playbook' ? (
          <PlaybookView />
        ) : activeView === 'coach' ? (
          <CoachView />
        ) : (
          <ResultsView
            newItemIds={newItemIds}
            focusedIndex={focusedIndex}
            renderLimit={renderLimit}
            setRenderLimit={setRenderLimit}
          />
        )}

        </main>

        {/* Footer */}
        <footer className="mt-8 text-center">
          <div className="flex items-center justify-center gap-2">
            <img src={sunLogo} alt="" className="w-4 h-4 rounded-sm object-cover opacity-40" />
            <p className="text-xs text-gray-600">All signal. No feed.</p>
          </div>
        </footer>

        {/* Update Banner */}
        {update && (
          <div className="fixed bottom-4 right-4 z-50 bg-bg-secondary border border-[#D4AF37]/40 rounded-xl px-5 py-4 shadow-lg max-w-sm">
            <div className="flex items-start gap-3">
              <div className="flex-1">
                <p className="text-sm font-medium text-white">Update available: v{update.version}</p>
                <p className="text-xs text-gray-400 mt-1">
                  {update.body ? update.body.slice(0, 100) : 'A new version is ready to install.'}
                </p>
              </div>
              <button onClick={dismissUpdate} aria-label="Dismiss update notification" className="text-gray-500 hover:text-white text-lg leading-none">&times;</button>
            </div>
            <div className="flex gap-2 mt-3">
              <button
                onClick={installUpdate}
                disabled={installing}
                className="px-4 py-1.5 text-xs font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
              >
                {installing ? 'Installing...' : 'Install & Restart'}
              </button>
              <button
                onClick={dismissUpdate}
                className="px-4 py-1.5 text-xs text-gray-400 hover:text-white transition-colors"
              >
                Later
              </button>
            </div>
          </div>
        )}

        {/* Command Deck (slide-up panel) */}
        <CommandDeck />

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
