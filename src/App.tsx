// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import './i18n';

import { useState, useEffect, useMemo, useRef, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import './App.css';
import sunLogo from './assets/sun-logo.webp';
import { SplashScreen } from './components/SplashScreen';
// Onboarding — only shown on first launch, lazy-loaded for returning users
const Onboarding = lazy(() => import('./components/Onboarding').then(m => ({ default: m.Onboarding })));
import { UnifiedAppBar } from './components/app/UnifiedAppBar';
import { AppModals } from './components/app/AppModals';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ViewErrorBoundary } from './components/ViewErrorBoundary';
import { ViewTabBar } from './components/ViewTabBar';
import { ViewRouter } from './components/ViewRouter';
import { UpdateBanner } from './components/UpdateBanner';
import { HealthBanner } from './components/HealthBanner';
import { CriticalAlertBanner } from './components/CriticalAlertBanner';
import { LicenseRecoveryBanner } from './components/LicenseRecoveryBanner';

// Lazy-loaded non-critical-path components
const FirstRunTransition = lazy(() => import('./components/FirstRunTransition').then(m => ({ default: m.FirstRunTransition })));

import {
  useSettings,
  useMonitoring,
  useAnalysis,
  useContextDiscovery,
  useFeedback,
  useSystemHealth,
  useUserContext,
  useBriefing,
  useToasts,
  useLicense,
  useUiZoom,
} from './hooks';
import { useAppShortcuts } from './hooks/use-app-shortcuts';
import { ContentTranslationProvider } from './components/ContentTranslationProvider';
import { useShallow } from 'zustand/react/shallow';

import { useAppStore } from './store';
import { cmd } from './lib/commands';
import { useUpdateCheck } from './hooks/use-update-check';
import { trackEvent } from './hooks/use-telemetry';
import { useDirection } from './i18n/rtl';
import { useAppListeners } from './hooks/use-app-listeners';
import { loadSourceMeta } from './config/sources';

function App() {
  const { t } = useTranslation();
  const { zoom, showIndicator } = useUiZoom();
  // Local UI state
  const [showSplash, setShowSplash] = useState(true);
  const showSettings = useAppStore(s => s.showSettings);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [showFramework, setShowFramework] = useState(false);
  const [showComparison, setShowComparison] = useState(false);
  const [newItemIds, setNewItemIds] = useState<Set<number>>(new Set());
  const newItemTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
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
  const setEmbeddingStatus = useAppStore(s => s.setEmbeddingStatus);
  const embeddingStatus = useAppStore(s => s.embeddingStatus);

  // First-run state
  const { isFirstRun, firstRunDismissed, setIsFirstRun, setFirstRunDismissed } = useAppStore(
    useShallow((s) => ({
      isFirstRun: s.isFirstRun,
      firstRunDismissed: s.firstRunDismissed,
      setIsFirstRun: s.setIsFirstRun,
      setFirstRunDismissed: s.setFirstRunDismissed,
    })),
  );

  const feedbackCount = useAppStore(s => Object.keys(s.feedbackGiven).length);
  const { tier, isPro } = useLicense();

  // i18n direction (sets dir/lang on <html>)
  useDirection();

  // Toast notification system
  const { toasts, addToast, removeToast } = useToasts();

  // Auto-update check
  const { update, installing, installUpdate, dismiss: dismissUpdate } = useUpdateCheck();
  // All application state via hooks
  const {
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

  const handleBackgroundItems = (itemIds: number[]) => {
    setNewItemIds(prev => {
      const next = new Set(prev);
      for (const id of itemIds) next.add(id);
      return next;
    });
    if (newItemTimerRef.current) clearTimeout(newItemTimerRef.current);
    newItemTimerRef.current = setTimeout(() => setNewItemIds(new Set()), 60000);
  };

  const {
    state,
    setState,
    expandedItem,
    setExpandedItem,
    isBrowserMode,
    startAnalysis,
  } = useAnalysis(addToast, handleBackgroundItems);

  const { loadDiscoveredContext } = useContextDiscovery(setSettingsStatus);

  useFeedback();
  useSystemHealth(setSettingsStatus);

  const {
    loadUserContext,
  } = useUserContext(setSettingsStatus);

  // Check Ollama status when provider changes to "ollama"
  useEffect(() => {
    if (settingsForm.provider === 'ollama') {
      void checkOllamaStatus(settingsForm.baseUrl || undefined);
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
  } = useBriefing(state.relevanceResults, state.analysisComplete);

  // Global keyboard shortcuts + analyze handler (extracted hook)
  const { focusedIndex, analysisPulse, handleAnalyze } = useAppShortcuts({
    showSettings, setShowSettings,
    showKeyboardHelp, setShowKeyboardHelp,
    expandedItem, setExpandedItem,
    state, startAnalysis,
    showOnlyRelevant, setShowOnlyRelevant,
    activeView, setActiveView,
    filteredResults,
    addToast,
  });

  // Load persisted briefing + source health + license + pro value + game state on mount (instant, from DB)
  useEffect(() => {
    trackEvent('app_launch');
    // Load source metadata from backend (populates dynamic source registry + resets filters)
    void loadSourceMeta().then(() => {
      useAppStore.getState().resetSourceFilters();
    });
    void loadPersistedBriefing();
    void loadSourceHealth();
    void loadLicense();
    void loadTrialStatus();
    void loadProValueReport();
    void cmd('prune_personalization_cache').catch(() => {});
  }, [loadPersistedBriefing, loadSourceHealth, loadLicense, loadTrialStatus, loadProValueReport]);

  // Event listeners: deep-link activation, embedding status, framework/comparison triggers, cached result loading
  useAppListeners({
    addToast,
    setEmbeddingStatus,
    setShowFramework,
    setShowComparison,
    setState,
    startAnalysis: () => { void startAnalysis(); },
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
              void loadSettings();
              void loadUserContext();
              void loadDiscoveredContext();
              void loadChannels();
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

      <div className={`min-h-screen bg-bg-primary text-white p-3 md:p-6 ${showSplash || showOnboarding ? 'hidden' : 'opacity-100 transition-opacity duration-300'}`}>
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-orange-500 focus:text-white focus:rounded-lg focus:text-sm focus:font-medium"
        >
          {t('app.skipToContent')}
        </a>
        {/* Unified App Bar — compact header + status + search + actions */}
        <UnifiedAppBar
          state={state}
          monitoring={monitoring}
          settingsFormProvider={settingsForm.provider}
          isPro={isPro}
          tier={tier}
          summaryBadges={summaryBadges}
          aiBriefing={aiBriefing}
          onAnalyze={handleAnalyze}
          onOpenSettings={() => setShowSettings(true)}
          analysisPulse={analysisPulse}
          embeddingStatus={embeddingStatus}
        />

        {/* Browser Mode Notice */}
        {isBrowserMode && (
          <div className="mb-6 px-4 py-4 bg-bg-secondary border border-border rounded-lg">
            <p className="text-sm font-medium text-white mb-2">{t('browser.title')}</p>
            <p className="text-xs text-gray-400">
              {t('browser.description')}
            </p>
            <p className="text-xs text-gray-500 mt-2">
              {t('browser.hint')}
            </p>
          </div>
        )}

        <main id="main-content">
        <h1 className="sr-only">{t('app.title', '4DA')}</h1>
        {/* License recovery — non-dismissible until tier restored */}
        <LicenseRecoveryBanner />
        {/* Critical security alerts — persistent until acknowledged */}
        <CriticalAlertBanner />
        {/* Health warnings — dismissible, only shows if issues found */}
        <HealthBanner />

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
            onInstall={() => { void installUpdate(); }}
            onDismiss={dismissUpdate}
          />
        )}

        {/* Modals, toasts, and overlays */}
        <AppModals
          toasts={toasts}
          removeToast={removeToast}
          zoom={zoom}
          showZoomIndicator={showIndicator}
          feedbackCount={feedbackCount}
          showKeyboardHelp={showKeyboardHelp}
          setShowKeyboardHelp={setShowKeyboardHelp}
          showFramework={showFramework}
          setShowFramework={setShowFramework}
          showComparison={showComparison}
          setShowComparison={setShowComparison}
          showSettings={showSettings}
          setShowSettings={setShowSettings}
        />

      </div>
    </>
  );
}

// Wrap App with ErrorBoundary
function AppWithErrorBoundary() {
  return (
    <ErrorBoundary>
      <ContentTranslationProvider>
        <App />
      </ContentTranslationProvider>
    </ErrorBoundary>
  );
}

export default AppWithErrorBoundary;
