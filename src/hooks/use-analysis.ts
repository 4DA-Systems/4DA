import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { ContextFile, SourceRelevance, AnalysisProgress, AppState } from '../types';
import type { ToastType } from './use-toasts';

const initialState: AppState = {
  contextFiles: [],
  relevanceResults: [],
  status: 'Ready to analyze',
  loading: false,
  analysisComplete: false,
  progress: 0,
  progressMessage: '',
  progressStage: '',
  lastAnalyzedAt: null,
};

export function useAnalysis(addToast?: (type: ToastType, message: string) => void) {
  const [state, setState] = useState<AppState>(initialState);
  const [expandedItem, setExpandedItem] = useState<number | null>(null);
  const [isBrowserMode, setIsBrowserMode] = useState(false);

  // Set up event listeners for background analysis
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenComplete: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let unlistenSourceError: UnlistenFn | null = null;
    let unlistenSourceFetched: UnlistenFn | null = null;
    let unlistenNetworkOffline: UnlistenFn | null = null;
    let unlistenEmbeddingMode: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenProgress = await listen<AnalysisProgress>('analysis-progress', (event) => {
        const { stage, progress, message, items_processed, items_total } = event.payload;
        setState((s) => ({
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
        setState((s) => ({
          ...s,
          relevanceResults: results,
          status: `${relevantCount}/${results.length} items relevant`,
          loading: false,
          analysisComplete: true,
          progress: 1,
          progressStage: 'complete',
          lastAnalyzedAt: new Date(),
        }));
        addToast?.('success', `Analysis complete: ${relevantCount} relevant items found`);
      });

      unlistenError = await listen<string>('analysis-error', (event) => {
        const msg = event.payload;
        setState((s) => ({
          ...s,
          status: `Error: ${msg}`,
          loading: false,
          progress: 0,
          progressStage: 'error',
        }));
        // Categorize and show actionable error toast
        if (msg.includes('API') || msg.includes('key') || msg.includes('401')) {
          addToast?.('error', 'API error - check your API key in Settings');
        } else if (msg.includes('network') || msg.includes('timeout') || msg.includes('connect')) {
          addToast?.('error', 'Network error - check your connection');
        } else {
          addToast?.('error', `Analysis failed: ${msg}`);
        }
      });

      // Per-source error events (Phase 3)
      unlistenSourceError = await listen<{ source: string; error: string; retry_count: number }>('source-error', (event) => {
        const { source, error } = event.payload;
        const sourceLabels: Record<string, string> = {
          hackernews: 'HN', arxiv: 'arXiv', reddit: 'Reddit', github: 'GitHub',
          rss: 'RSS', youtube: 'YouTube', twitter: 'Twitter',
        };
        addToast?.('warning', `${sourceLabels[source] || source}: ${error}`);
      });

      // Per-source success events
      unlistenSourceFetched = await listen<{ source: string; count: number }>('source-fetched', (_event) => {
        // Silent — success is the default. Only toast on errors.
      });

      // Network offline event (Phase 4)
      unlistenNetworkOffline = await listen('network-offline', () => {
        addToast?.('warning', 'Offline - showing cached results only');
      });

      // Embedding mode event (Phase 13: Zero-Config)
      unlistenEmbeddingMode = await listen<{ mode: string; reason?: string }>('embedding-mode', (event) => {
        if (event.payload.mode === 'keyword-only') {
          addToast?.('info', 'Running in keyword-only mode. Add API key for better results.');
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
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- addToast is stable from useToasts
  }, []);

  const loadContextFiles = useCallback(async () => {
    setState((s) => ({ ...s, loading: true, status: 'Loading context files...' }));
    try {
      const files = await invoke<ContextFile[]>('get_context_files');
      setState((s) => ({
        ...s,
        contextFiles: files,
        status: `Loaded ${files.length} context files. Click "Analyze" to compute relevance.`,
        loading: false,
      }));
      setIsBrowserMode(false);
    } catch (error) {
      const errorMsg = String(error);
      if (errorMsg.includes('invoke') || errorMsg.includes('__TAURI__')) {
        setIsBrowserMode(true);
        setState((s) => ({
          ...s,
          status: 'Browser mode detected',
          loading: false,
        }));
      } else {
        setState((s) => ({ ...s, status: `Error: ${error}`, loading: false }));
      }
    }
  }, []);

  const clearContext = useCallback(async () => {
    try {
      const result = await invoke<string>('clear_context');
      const files = await invoke<ContextFile[]>('get_context_files');
      setState((s) => ({
        ...s,
        contextFiles: files || [],
        relevanceResults: [],
        status: `${result}. ${files?.length || 0} files ready to index.`,
      }));
    } catch (error) {
      console.error('Failed to clear context:', error);
      setState((s) => ({ ...s, status: `Error: ${error}` }));
    }
  }, []);

  const indexContext = useCallback(async () => {
    setState((s) => ({ ...s, loading: true, status: 'Indexing project READMEs...' }));
    try {
      // First, index READMEs from all configured project directories
      // This is crucial for relevance scoring when no context files are present
      const readmeResult = await invoke<string>('index_project_readmes');
      setState((s) => ({ ...s, status: `${readmeResult}. Indexing local files...` }));

      // Then, index files from the primary context directory
      const result = await invoke<string>('index_context');
      const files = await invoke<ContextFile[]>('get_context_files');
      setState((s) => ({
        ...s,
        contextFiles: files,
        status: `${readmeResult}. ${result}`,
        loading: false,
      }));
    } catch (error) {
      setState((s) => ({
        ...s,
        status: `Index failed: ${error}`,
        loading: false,
      }));
    }
  }, []);

  const startAnalysis = useCallback(async () => {
    setState((s) => ({
      ...s,
      loading: true,
      status: 'Starting cache-first analysis...',
      analysisComplete: false,
      progress: 0,
      progressMessage: 'Loading cached items...',
      progressStage: 'init',
    }));

    try {
      // Use cache-first analysis by default - INSTANT, no API calls
      // Falls back to fetching internally if cache is empty
      await invoke('run_cached_analysis');
    } catch (error) {
      const errorMsg = String(error);
      if (errorMsg.includes('invoke') || errorMsg.includes('__TAURI__')) {
        setState((s) => ({
          ...s,
          status: 'Cannot analyze in browser mode. Open through Tauri window.',
          loading: false,
        }));
        addToast?.('error', 'Cannot analyze in browser mode');
      } else if (errorMsg.includes('already running')) {
        setState((s) => ({
          ...s,
          status: 'Analysis already in progress...',
        }));
        addToast?.('info', 'Analysis already in progress');
      } else {
        setState((s) => ({ ...s, status: `Error: ${error}`, loading: false }));
        addToast?.('error', `Analysis failed: ${errorMsg}`);
      }
    }
  }, [addToast]);

  const setStatus = useCallback((status: string) => {
    setState(s => ({ ...s, status }));
  }, []);

  useEffect(() => {
    loadContextFiles();
  }, [loadContextFiles]);

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
