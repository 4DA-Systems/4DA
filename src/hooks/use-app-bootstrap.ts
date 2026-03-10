import { useState, useEffect, useCallback, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
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
} from '../hooks';
import { useAppStore } from '../store';
import { useUpdateCheck } from './use-update-check';
import { trackEvent } from './use-telemetry';
import { useDirection } from '../i18n/rtl';
import { cmd } from '../lib/commands';

/**
 * Encapsulates all bootstrap logic for the App component:
 * hook calls, side effects, event listeners, and keyboard shortcuts.
 */
export function useAppBootstrap() {
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

  // Action selectors (stable references)
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

  const { monitoring } = useMonitoring();

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

  const { loadDiscoveredContext } = useContextDiscovery(setSettingsStatus);

  // Feedback hook
  const { learnedAffinities, antiTopics, lastLearnedTopic } = useFeedback();

  // System health hook
  useSystemHealth(setSettingsStatus);

  const { loadUserContext } = useUserContext(setSettingsStatus);

  // Check Ollama status when provider changes to "ollama"
  useEffect(() => {
    if (settingsForm.provider === 'ollama') {
      checkOllamaStatus(settingsForm.baseUrl || undefined);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only re-check when provider changes
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

  // Stabilize onAnalyze callback
  const handleAnalyze = useCallback(() => {
    setNewItemIds(new Set());
    startAnalysis();
  }, [startAnalysis]);

  // Load persisted briefing + source health + license + pro value on mount
  useEffect(() => {
    trackEvent('app_launch');
    loadPersistedBriefing();
    loadSourceHealth();
    loadLicense();
    loadTrialStatus();
    loadProValueReport();
    computeViewTier();
    cmd('prune_personalization_cache', {}).catch(e => console.debug('Prune cache skipped:', e));
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
        const analysisState = await cmd('get_analysis_status', {});

        if (cancelled) return;

        if (analysisState.results && analysisState.results.length > 0) {
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

  return {
    // UI state
    showSplash,
    setShowSplash,
    showSettings,
    setShowSettings,
    showKeyboardHelp,
    setShowKeyboardHelp,
    newItemIds,
    analysisPulse,
    // Data
    activeView,
    filteredResults,
    isPro,
    tier,
    // Hooks
    settings,
    settingsForm,
    showOnboarding,
    setShowOnboarding,
    loadSettings,
    monitoring,
    state,
    isBrowserMode,
    aiBriefing,
    autoBriefingEnabled,
    setAutoBriefingEnabled,
    generateBriefing,
    summaryBadges,
    learnedAffinities,
    antiTopics,
    lastLearnedTopic,
    // Actions
    handleAnalyze,
    setActiveView,
    setIsFirstRun,
    setFirstRunDismissed,
    loadUserContext,
    loadDiscoveredContext,
    loadChannels,
    addToast,
    // First run
    isFirstRun,
    firstRunDismissed,
    // Focused index
    focusedIndex,
    // Toasts
    toasts,
    removeToast,
    // Update
    update,
    installing,
    installUpdate,
    dismissUpdate,
  };
}
