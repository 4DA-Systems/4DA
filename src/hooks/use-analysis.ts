import { useEffect, useCallback } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { SourceRelevance, AnalysisProgress } from '../types';
import { getSourceLabel } from '../config/sources';
import { useAppStore } from '../store';

/**
 * Analysis hook — thin wrapper around Zustand store.
 * Sets up Tauri event listeners that write directly to the store.
 * All state lives in the store; this hook adds no local state.
 */
export function useAnalysis(
  _addToast?: (type: 'success' | 'error' | 'warning' | 'info', message: string) => void,
  onBackgroundItems?: (itemIds: number[]) => void,
) {
  const state = useAppStore(s => s.appState);
  const expandedItem = useAppStore(s => s.expandedItem);
  const isBrowserMode = useAppStore(s => s.isBrowserMode);
  const setExpandedItem = useAppStore(s => s.setExpandedItem);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const loadContextFiles = useAppStore(s => s.loadContextFiles);
  const clearContext = useAppStore(s => s.clearContext);
  const indexContext = useAppStore(s => s.indexContext);

  // Set up event listeners for background analysis — all write to store
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenComplete: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let unlistenSourceError: UnlistenFn | null = null;
    let unlistenSourceFetched: UnlistenFn | null = null;
    let unlistenNetworkOffline: UnlistenFn | null = null;
    let unlistenEmbeddingMode: UnlistenFn | null = null;
    let unlistenBackgroundResults: UnlistenFn | null = null;
    let unlistenTrayAnalyze: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenProgress = await listen<AnalysisProgress>('analysis-progress', (event) => {
        const { stage, progress, message, items_processed, items_total } = event.payload;
        useAppStore.getState().setAppStateFull((s) => ({
          ...s,
          progress,
          progressMessage: message,
          progressStage: stage,
          status: items_total > 0
            ? `${message} (${items_processed}/${items_total})`
            : message,
        }));
      });

      unlistenComplete = await listen<SourceRelevance[]>('analysis-complete', (event) => {
        const results = event.payload;
        const relevantCount = results.filter((r) => r.relevant).length;
        useAppStore.getState().setAppStateFull((s) => ({
          ...s,
          relevanceResults: results,
          status: `${relevantCount}/${results.length} items relevant`,
          loading: false,
          analysisComplete: true,
          progress: 1,
          progressStage: 'complete',
          lastAnalyzedAt: new Date(),
        }));
        useAppStore.getState().addToast('success', `Analysis complete: ${relevantCount} relevant items found`);
      });

      unlistenError = await listen<string>('analysis-error', (event) => {
        const msg = event.payload;
        useAppStore.getState().setAppStateFull((s) => ({
          ...s,
          status: `Error: ${msg}`,
          loading: false,
          progress: 0,
          progressStage: 'error',
        }));
        // Categorize and show actionable error toast
        const { addToast: toast } = useAppStore.getState();
        if (msg.includes('API') || msg.includes('key') || msg.includes('401')) {
          toast('error', 'API error - check your API key in Settings');
        } else if (msg.includes('network') || msg.includes('timeout') || msg.includes('connect')) {
          toast('error', 'Network error - check your connection');
        } else {
          toast('error', `Analysis failed: ${msg}`);
        }
      });

      // Per-source error events
      unlistenSourceError = await listen<{ source: string; error: string; retry_count: number }>('source-error', (event) => {
        const { source, error } = event.payload;
        useAppStore.getState().addToast('warning', `${getSourceLabel(source)}: ${error}`);
      });

      // Per-source success events
      unlistenSourceFetched = await listen<{ source: string; count: number }>('source-fetched', (_event) => {
        // Silent — success is the default. Only toast on errors.
      });

      // Network offline event
      unlistenNetworkOffline = await listen('network-offline', () => {
        useAppStore.getState().addToast('warning', 'Offline - showing cached results only');
      });

      // Embedding mode event
      unlistenEmbeddingMode = await listen<{ mode: string; reason?: string }>('embedding-mode', (event) => {
        if (event.payload.mode === 'keyword-only') {
          useAppStore.getState().addToast('info', 'Running in keyword-only mode. Add API key for better results.');
        }
      });

      // Tray menu "Analyze Now" button
      unlistenTrayAnalyze = await listen('start-analysis-from-tray', () => {
        useAppStore.getState().startAnalysis();
      });

      // Background results from scheduled monitoring (silent merge)
      unlistenBackgroundResults = await listen<SourceRelevance[]>('background-results', (event) => {
        const newItems = event.payload;
        if (newItems.length === 0) return;
        const relevantNew = newItems.filter((r) => r.relevant).length;
        useAppStore.getState().setAppStateFull((s) => {
          const existingIds = new Set(newItems.map((n) => n.id));
          const kept = s.relevanceResults.filter((r) => !existingIds.has(r.id));
          const merged = [...kept, ...newItems].sort((a, b) => b.top_score - a.top_score);
          return {
            ...s,
            relevanceResults: merged,
            analysisComplete: true,
            lastAnalyzedAt: new Date(),
          };
        });
        if (relevantNew > 0) {
          useAppStore.getState().addToast('info', `${relevantNew} new relevant items found`);
          onBackgroundItems?.(newItems.map(n => n.id));
        }
      });
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
      if (unlistenSourceError) unlistenSourceError();
      if (unlistenSourceFetched) unlistenSourceFetched();
      if (unlistenNetworkOffline) unlistenNetworkOffline();
      if (unlistenEmbeddingMode) unlistenEmbeddingMode();
      if (unlistenBackgroundResults) unlistenBackgroundResults();
      if (unlistenTrayAnalyze) unlistenTrayAnalyze();
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable callback ref
  }, []);

  useEffect(() => {
    loadContextFiles();
  }, [loadContextFiles]);

  const setStatus = useCallback((status: string) => {
    useAppStore.getState().setAppState({ status });
  }, []);

  // setState wrapper for App.tsx compatibility (accepts updater function)
  const setState = useCallback((updater: ((s: typeof state) => typeof state) | Partial<typeof state>) => {
    if (typeof updater === 'function') {
      useAppStore.getState().setAppStateFull(updater);
    } else {
      useAppStore.getState().setAppState(updater);
    }
  }, []);

  return {
    state,
    setState,
    expandedItem,
    setExpandedItem,
    isBrowserMode,
    loadContextFiles,
    clearContext,
    indexContext,
    startAnalysis,
    setStatus,
  };
}
