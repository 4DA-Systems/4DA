import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { ContextFile, HNRelevance, AnalysisProgress, AppState } from '../types';

const initialState: AppState = {
  contextFiles: [],
  relevanceResults: [],
  status: 'Ready to analyze',
  loading: false,
  analysisComplete: false,
  progress: 0,
  progressMessage: '',
  progressStage: '',
};

export function useAnalysis() {
  const [state, setState] = useState<AppState>(initialState);
  const [expandedItem, setExpandedItem] = useState<number | null>(null);
  const [isBrowserMode, setIsBrowserMode] = useState(false);
  const [enabledSources, setEnabledSources] = useState<string[]>(['hackernews']);

  // Set up event listeners for background analysis
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenComplete: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;

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

      unlistenComplete = await listen<HNRelevance[]>('analysis-complete', (event) => {
        const results = event.payload;
        const relevantCount = results.filter((r) => r.relevant).length;
        setState((s) => ({
          ...s,
          relevanceResults: results,
          status: `Analysis complete: ${relevantCount}/${results.length} items relevant`,
          loading: false,
          analysisComplete: true,
          progress: 1,
          progressStage: 'complete',
        }));
      });

      unlistenError = await listen<string>('analysis-error', (event) => {
        setState((s) => ({
          ...s,
          status: `Error: ${event.payload}`,
          loading: false,
          progress: 0,
          progressStage: 'error',
        }));
      });
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
    };
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
      } else if (errorMsg.includes('already running')) {
        setState((s) => ({
          ...s,
          status: 'Analysis already in progress...',
        }));
      } else {
        setState((s) => ({ ...s, status: `Error: ${error}`, loading: false }));
      }
    }
  }, []);

  const toggleSource = useCallback((source: string) => {
    setEnabledSources(prev => {
      if (prev.includes(source)) {
        if (prev.length === 1) return prev;
        return prev.filter(s => s !== source);
      } else {
        return [...prev, source];
      }
    });
  }, []);

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
    enabledSources,
    loadContextFiles,
    clearContext,
    indexContext,
    startAnalysis,
    toggleSource,
    setStatus,
  };
}
