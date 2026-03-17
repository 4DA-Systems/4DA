import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, AnalysisSlice, AppState } from './types';
import { translateError } from '../utils/error-messages';

const initialAppState: AppState = {
  contextFiles: [],
  relevanceResults: [],
  nearMisses: null,
  status: 'Ready to analyze',
  loading: false,
  analysisComplete: false,
  progress: 0,
  progressMessage: '',
  progressStage: '',
  lastAnalyzedAt: null,
};

export const createAnalysisSlice: StateCreator<AppStore, [], [], AnalysisSlice> = (set, get) => ({
  appState: { ...initialAppState },
  expandedItem: null,
  isBrowserMode: false,

  setExpandedItem: (id) => set({ expandedItem: id }),

  setAppState: (partial) => {
    set(state => ({
      appState: { ...state.appState, ...partial },
    }));
  },

  setAppStateFull: (updaterOrValue) => {
    set(state => ({
      appState: typeof updaterOrValue === 'function'
        ? updaterOrValue(state.appState)
        : updaterOrValue,
    }));
  },

  startAnalysis: async () => {
    const { addToast } = get();
    set(state => ({
      appState: {
        ...state.appState,
        loading: true,
        status: 'Starting cache-first analysis...',
        analysisComplete: false,
        progress: 0,
        progressMessage: 'Loading cached items...',
        progressStage: 'init',
      },
    }));

    try {
      await cmd('run_cached_analysis');
    } catch (error) {
      const errorMsg = String(error);
      if (errorMsg.includes('invoke') || errorMsg.includes('__TAURI__')) {
        set(state => ({
          appState: {
            ...state.appState,
            status: 'Cannot analyze in browser mode. Open through Tauri window.',
            loading: false,
          },
        }));
        addToast('error', 'Cannot analyze in browser mode');
      } else if (errorMsg.includes('already running')) {
        set(state => ({
          appState: {
            ...state.appState,
            status: 'Analysis already in progress...',
          },
        }));
        addToast('info', 'Analysis already in progress');
      } else {
        set(state => ({
          appState: { ...state.appState, status: `Error: ${translateError(error)}`, loading: false },
        }));
        addToast('error', `Analysis failed: ${translateError(error)}`);
      }
    }
  },

  loadContextFiles: async () => {
    set(state => ({
      appState: { ...state.appState, loading: true, status: 'Loading context files...' },
    }));
    try {
      const files = await cmd('get_context_files');
      set(state => ({
        appState: {
          ...state.appState,
          contextFiles: files,
          status: `Loaded ${files.length} context files. Click "Analyze" to compute relevance.`,
          loading: false,
        },
        isBrowserMode: false,
      }));
    } catch (error) {
      const errorMsg = String(error);
      if (errorMsg.includes('invoke') || errorMsg.includes('__TAURI__')) {
        set(state => ({
          isBrowserMode: true,
          appState: {
            ...state.appState,
            status: 'Browser mode detected',
            loading: false,
          },
        }));
      } else {
        set(state => ({
          appState: { ...state.appState, status: `Error: ${translateError(error)}`, loading: false },
        }));
      }
    }
  },

  clearContext: async () => {
    try {
      const result = await cmd('clear_context');
      const files = await cmd('get_context_files');
      set(state => ({
        appState: {
          ...state.appState,
          contextFiles: files || [],
          relevanceResults: [],
          status: `${result}. ${files?.length || 0} files ready to index.`,
        },
      }));
    } catch (error) {
      console.error('Failed to clear context:', error);
      set(state => ({
        appState: { ...state.appState, status: `Error: ${translateError(error)}` },
      }));
    }
  },

  indexContext: async () => {
    set(state => ({
      appState: { ...state.appState, loading: true, status: 'Indexing project READMEs...' },
    }));
    try {
      const readmeResult = await cmd('index_project_readmes');
      set(state => ({
        appState: { ...state.appState, status: `${readmeResult}. Indexing local files...` },
      }));

      const result = await cmd('index_context');

      // Sync AWE wisdom into context (non-blocking, best-effort)
      let aweStatus = '';
      try {
        aweStatus = await cmd('sync_awe_wisdom');
      } catch {
        // AWE sync is optional — don't fail the index
      }

      const files = await cmd('get_context_files');
      set(state => ({
        appState: {
          ...state.appState,
          contextFiles: files,
          status: `${readmeResult}. ${result}${aweStatus ? `. ${aweStatus}` : ''}`,
          loading: false,
        },
      }));
    } catch (error) {
      console.error('Failed to index context:', error);
      set(state => ({
        appState: {
          ...state.appState,
          status: `Index failed: ${translateError(error)}`,
          loading: false,
        },
      }));
    }
  },
});
