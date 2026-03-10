// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import './i18n';

import { useState, useEffect, useCallback, useMemo, lazy, Suspense } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import './App.css';
import sunLogo from './assets/sun-logo.webp';
import { SplashScreen } from './components/SplashScreen';
// Onboarding — only shown on first launch, lazy-loaded for returning users
const Onboarding = lazy(() => import('./components/Onboarding').then(m => ({ default: m.Onboarding })));
import { VoidEngine } from './components/void-engine/VoidEngine';
import { OllamaStatus } from './components/OllamaStatus';
import { ToastContainer } from './components/Toast';
import { ActionBar } from './components/ActionBar';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ViewErrorBoundary } from './components/ViewErrorBoundary';
import { ViewTabBar } from './components/ViewTabBar';
import { ViewRouter } from './components/ViewRouter';
import { UpdateBanner } from './components/UpdateBanner';

// Lazy-loaded non-critical-path components
const FirstRunTransition = lazy(() => import('./components/FirstRunTransition').then(m => ({ default: m.FirstRunTransition })));
const NaturalLanguageSearch = lazy(() => import('./components/NaturalLanguageSearch').then(m => ({ default: m.NaturalLanguageSearch })));
const LearningIndicator = lazy(() => import('./components/LearningIndicator').then(m => ({ default: m.LearningIndicator })));
const ProValueBadge = lazy(() => import('./components/ProValueBadge').then(m => ({ default: m.ProValueBadge })));

// Lazy-loaded non-critical views and overlays
const SettingsModal = lazy(() => import('./components/SettingsModal').then(m => ({ default: m.SettingsModal })));
const KeyboardShortcutsModal = lazy(() => import('./components/KeyboardShortcutsModal').then(m => ({ default: m.KeyboardShortcutsModal })));
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
import { cmd } from './lib/commands';
import { useUpdateCheck } from './hooks/use-update-check';
import { trackEvent } from './hooks/use-telemetry';
import { useDirection } from './i18n/rtl';
function App() {
  const { t } = useTranslation();
  // Local UI state
  const [showSplash, setShowSplash] = useState(true);
  const showSettings = useAppStore(s => s.showSettings);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [newItemIds, setNewItemIds] = useState<Set<number>>(new Set());
  const [analysisPulse, setAnalysisPulse] = useState(false);
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
  const loadChannels = useAppStore(s => s.loadChannels);
  const computeViewTier = useAppStore(s => s.computeViewTier);

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

  // 5c. Stabilize onAnalyze callback
  const handleAnalyze = useCallback(() => {
    setNewItemIds(new Set());
    startAnalysis();
  }, [startAnalysis]);

  // Load persisted briefing + source health + license + pro value + game state on mount (instant, from DB)
  useEffect(() => {
    trackEvent('app_launch');
    loadPersistedBriefing();
    loadSourceHealth();
    loadLicense();
    loadTrialStatus();
    loadProValueReport();
    // Compute progressive disclosure tier from persisted state
    computeViewTier();
    // Prune stale personalization cache (non-blocking)
    cmd('prune_personalization_cache').catch(e => console.debug('Prune cache skipped:', e));
  }, [loadPersistedBriefing, loadSourceHealth, loadLicense, loadTrialStatus, loadProValueReport, computeViewTier]);

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
        const analysisState = await cmd('get_analysis_status');

        if (cancelled) return;

        if (analysisState.results && analysisState.results.length > 0) {
          // Restore previous results
          const results = analysisState.results;
          const relevantCount = results.filter(r => r.relevant).length;
          setState(s => ({
            ...s,
            relevanceResults: results,
            nearMisses: analysisState.near_misses ?? null,
            status: `${relevantCount}/${results.length} items relevant (cached)`,
            analysisComplete: true,
            loading: false,
          }));
          return;
        }

        // No cached results — auto-trigger full analysis after splash settles
        // (Skip if first-run or onboarding — FirstRunTransition handles analysis trigger)
        if (cancelled || useAppStore.getState().isFirstRun) return;
        autoTimer = setTimeout(() => {
          const s = useAppStore.getState();
          if (!cancelled && !s.isFirstRun && !s.showOnboarding) startAnalysis();
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
    resultCount: filteredResults.length,
    onFocusResult: (index: number) => {
      const el = document.getElementById(`result-item-${filteredResults[index]?.id}`);
      el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    },
    onToggleExpandResult: (index: number) => {
      const item = filteredResults[index];
      if (item) setExpandedItem(expandedItem === item.id ? null : item.id);
    },
    onOpenResult: (index: number) => {
      const item = filteredResults[index];
      if (item?.url) window.open(item.url, '_blank', 'noopener,noreferrer');
    },
    onAnalyzeTriggered: () => {
      addToast('info', t('analysis.keyboardTriggered'));
      setAnalysisPulse(true);
      setTimeout(() => setAnalysisPulse(false), 500);
    },
  });

  return (
    <>
      {/* First-run flow — wrapped in error boundary to contain crashes
           during the user's first impression */}
      <ViewErrorBoundary viewName="First Run">
        {/* Splash Screen */}
        {showSplash && (
          <SplashScreen onComplete={() => setShowSplash(false)} minimumDisplayTime={800} />
        )}

        {/* Onboarding Flow (first run) — lazy-loaded */}
        {!showSplash && showOnboarding && (
          <Suspense fallback={null}>
            <Onboarding onComplete={() => {
              setShowOnboarding(false);
              setIsFirstRun(true);
              loadSettings();
              loadUserContext();
              loadDiscoveredContext();
              loadChannels();
            }} />
          </Suspense>
        )}

        {/* First-Run Transition (bridges onboarding to first analysis) */}
        {!showSplash && !showOnboarding && isFirstRun && !firstRunDismissed && (
          <Suspense fallback={null}>
            <FirstRunTransition onComplete={(view) => {
              setFirstRunDismissed(true);
              setActiveView(view);
            }} />
          </Suspense>
        )}
      </ViewErrorBoundary>

      <div className={`min-h-screen bg-bg-primary text-white p-6 ${showSplash || showOnboarding ? 'hidden' : 'opacity-100 transition-opacity duration-300'}`}>
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-orange-500 focus:text-white focus:rounded-lg focus:text-sm focus:font-medium"
        >
          {t('app.skipToContent')}
        </a>
        {/* Header - Polished */}
        <header className="mb-8 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 flex items-center justify-center flex-shrink-0">
              <VoidEngine size={48} />
            </div>
            <div>
              <h1 className="text-2xl font-semibold tracking-tight text-white">{t('app.title')}</h1>
              <p className="text-gray-500 text-sm">{t('app.tagline')}</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {monitoring?.enabled && (
              <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/30 rounded-lg">
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                <span className="text-xs text-green-400 font-medium">{t('header.live')}</span>
              </div>
            )}
            <OllamaStatus provider={settingsForm.provider} />
            <Suspense fallback={null}><ProValueBadge /></Suspense>
            <span className={`px-2 py-1 text-[10px] font-bold uppercase tracking-wider rounded ${
              isPro
                ? 'bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30'
                : 'bg-bg-tertiary text-gray-500 border border-border'
            }`}>
              {tier}
            </span>
            <button
              data-settings-trigger
              onClick={() => setShowSettings(true)}
              className="px-4 py-2 text-sm bg-bg-secondary text-gray-300 border border-border rounded-lg hover:bg-bg-tertiary hover:border-orange-500/30 transition-all"
            >
              {t('header.settings')}
            </button>
          </div>
        </header>

        {/* Browser Mode Notice */}
        {isBrowserMode && (
          <div className="mb-6 px-4 py-4 bg-bg-secondary border border-border rounded-lg">
            <p className="text-sm font-medium text-white mb-2">{t('browser.title')}</p>
            <p className="text-xs text-gray-400">
              {t('browser.description')}
            </p>
            <p className="text-xs text-gray-500 mt-2" dangerouslySetInnerHTML={{ __html: t('browser.hint') }} />
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
          onAnalyze={handleAnalyze}
          onGenerateBriefing={generateBriefing}
          onToggleAutoBriefing={() => setAutoBriefingEnabled(!autoBriefingEnabled)}
          onToast={addToast}
          analysisPulse={analysisPulse}
        />

        {/* Learning Indicator - Visible Learning Loop */}
        <Suspense fallback={null}>
          <LearningIndicator
            learnedAffinities={learnedAffinities}
            antiTopics={antiTopics}
            lastLearnedTopic={lastLearnedTopic}
          />
        </Suspense>

        {/* Intelligence Console */}
        <div className="mb-6">
          <Suspense fallback={null}>
            <NaturalLanguageSearch defaultExpanded={true} />
          </Suspense>
        </div>

        {/* View Tab Bar */}
        <ViewTabBar />

        {/* Conditional View Rendering */}
        <ViewRouter newItemIds={newItemIds} focusedIndex={focusedIndex} />

        </main>

        {/* Footer */}
        <footer className="mt-8 text-center">
          <div className="flex items-center justify-center gap-2">
            <img src={sunLogo} alt="" className="w-4 h-4 rounded-sm object-cover opacity-40" />
            <p className="text-xs text-gray-600">{t('app.tagline')}</p>
          </div>
        </footer>

        {/* Update Banner */}
        {update && (
          <UpdateBanner
            update={update}
            installing={installing}
            onInstall={installUpdate}
            onDismiss={dismissUpdate}
          />
        )}

        {/* Toast Notifications */}
        <ToastContainer toasts={toasts} onDismiss={removeToast} />

        {/* Keyboard Shortcuts Help Modal */}
        {showKeyboardHelp && (
          <Suspense fallback={null}>
            <ViewErrorBoundary viewName="Keyboard Shortcuts">
              <KeyboardShortcutsModal onClose={() => setShowKeyboardHelp(false)} />
            </ViewErrorBoundary>
          </Suspense>
        )}

        {/* Settings Modal - now self-sufficient via Zustand store */}
        {showSettings && (
          <Suspense fallback={null}>
            <ViewErrorBoundary viewName="Settings" onReset={() => setShowSettings(false)}>
              <SettingsModal
                onClose={() => setShowSettings(false)}
              />
            </ViewErrorBoundary>
          </Suspense>
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
