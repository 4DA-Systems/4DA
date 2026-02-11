import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  Settings,
  MonitoringStatus,
  UserContext,
  Anomaly,
  SystemHealth,
  ContextFile,
  SourceRelevance,
  FeedbackGiven,
  SuggestedInterest,
} from '../types';

// ============================================================================
// Types
// ============================================================================

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
  id: number;
  type: ToastType;
  message: string;
}

export interface SettingsForm {
  provider: string;
  apiKey: string;
  model: string;
  baseUrl: string;
  rerankEnabled: boolean;
  maxItems: number;
  minScore: number;
  dailyTokenLimit: number;
  dailyCostLimit: number;
}

export interface OllamaStatus {
  running: boolean;
  version: string | null;
  models: string[];
  base_url: string;
  error?: string;
}

export interface DiscoveredContext {
  tech: Array<{ name: string; category: string; confidence: number }>;
  topics: string[];
  lastScan: string | null;
}

export interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  affinity_score: number;
}

export interface AntiTopic {
  topic: string;
  rejection_count: number;
  confidence: number;
  auto_detected: boolean;
}

export interface SimilarTopicResult {
  topic: string;
  similarity: number;
}

export interface BriefingState {
  content: string | null;
  loading: boolean;
  error: string | null;
  model: string | null;
  lastGenerated: Date | null;
}

export interface AppState {
  contextFiles: ContextFile[];
  relevanceResults: SourceRelevance[];
  status: string;
  loading: boolean;
  analysisComplete: boolean;
  progress: number;
  progressMessage: string;
  progressStage: string;
  lastAnalyzedAt: Date | null;
}

// ============================================================================
// Defaults
// ============================================================================

const defaultSettingsForm: SettingsForm = {
  provider: 'anthropic',
  apiKey: '',
  model: 'claude-3-haiku-20240307',
  baseUrl: '',
  rerankEnabled: false,
  maxItems: 15,
  minScore: 0.25,
  dailyTokenLimit: 100000,
  dailyCostLimit: 50,
};

const initialAppState: AppState = {
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

const initialBriefingState: BriefingState = {
  content: null,
  loading: false,
  error: null,
  model: null,
  lastGenerated: null,
};

const ALL_SOURCES = new Set([
  'hackernews', 'arxiv', 'reddit', 'github',
  'rss', 'youtube', 'twitter', 'producthunt',
]);

// ============================================================================
// Store Interface
// ============================================================================

interface AppStore {
  // ======= Toast Slice =======
  toasts: Toast[];
  addToast: (type: ToastType, message: string) => void;
  removeToast: (id: number) => void;

  // ======= UI Slice =======
  showSettings: boolean;
  showSplash: boolean;
  setShowSettings: (show: boolean) => void;
  setShowSplash: (show: boolean) => void;

  // ======= Settings Slice =======
  settings: Settings | null;
  settingsForm: SettingsForm;
  settingsStatus: string;
  showOnboarding: boolean;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  setSettingsForm: (partial: Partial<SettingsForm>) => void;
  setSettingsFormFull: (updaterOrValue: SettingsForm | ((prev: SettingsForm) => SettingsForm)) => void;
  setSettingsStatus: (status: string) => void;
  setShowOnboarding: (show: boolean) => void;
  loadSettings: () => Promise<void>;
  saveSettings: () => Promise<void>;
  testConnection: () => Promise<void>;
  checkOllamaStatus: (baseUrl?: string) => Promise<OllamaStatus>;

  // ======= Analysis Slice =======
  appState: AppState;
  expandedItem: number | null;
  isBrowserMode: boolean;
  setExpandedItem: (id: number | null) => void;
  setAppState: (partial: Partial<AppState>) => void;
  setAppStateFull: (updaterOrValue: AppState | ((prev: AppState) => AppState)) => void;
  startAnalysis: () => Promise<void>;
  loadContextFiles: () => Promise<void>;
  clearContext: () => Promise<void>;
  indexContext: () => Promise<void>;

  // ======= Filters Slice =======
  sourceFilters: Set<string>;
  sortBy: 'score' | 'date';
  showOnlyRelevant: boolean;
  toggleSourceFilter: (source: string) => void;
  setSortBy: (sort: 'score' | 'date') => void;
  setShowOnlyRelevant: (show: boolean) => void;

  // ======= Feedback Slice =======
  feedbackGiven: FeedbackGiven;
  learnedAffinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  loadLearnedBehavior: () => Promise<void>;

  // ======= Monitoring Slice =======
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (interval: number) => void;
  loadMonitoringStatus: () => Promise<void>;
  toggleMonitoring: () => Promise<string>;
  updateMonitoringInterval: () => Promise<string>;
  testNotification: () => Promise<string>;

  // ======= Briefing Slice =======
  aiBriefing: BriefingState;
  showBriefing: boolean;
  autoBriefingEnabled: boolean;
  setShowBriefing: (show: boolean) => void;
  setAutoBriefingEnabled: (enabled: boolean) => void;
  generateBriefing: () => Promise<void>;

  // ======= Context Discovery Slice =======
  scanDirectories: string[];
  newScanDir: string;
  isScanning: boolean;
  discoveredContext: DiscoveredContext;
  setNewScanDir: (dir: string) => void;
  runAutoDiscovery: () => Promise<void>;
  runFullScan: () => Promise<void>;
  addScanDirectory: () => Promise<void>;
  removeScanDirectory: (dir: string) => Promise<void>;
  loadDiscoveredContext: () => Promise<void>;

  // ======= User Context Slice =======
  userContext: UserContext | null;
  suggestedInterests: SuggestedInterest[];
  newInterest: string;
  newExclusion: string;
  newTechStack: string;
  newRole: string;
  setNewInterest: (v: string) => void;
  setNewExclusion: (v: string) => void;
  setNewTechStack: (v: string) => void;
  setNewRole: (v: string) => void;
  loadUserContext: () => Promise<void>;
  loadSuggestedInterests: () => Promise<void>;
  addInterest: () => Promise<void>;
  removeInterest: (topic: string) => Promise<void>;
  addExclusion: () => Promise<void>;
  removeExclusion: (topic: string) => Promise<void>;
  addTechStack: () => Promise<void>;
  removeTechStack: (tech: string) => Promise<void>;
  updateRole: () => Promise<void>;

  // ======= System Health Slice =======
  systemHealth: SystemHealth | null;
  similarTopicQuery: string;
  similarTopicResults: SimilarTopicResult[];
  setSimilarTopicQuery: (q: string) => void;
  loadSystemHealth: () => Promise<void>;
  runAnomalyDetection: () => Promise<void>;
  resolveAnomaly: (anomalyId: number) => Promise<void>;
  findSimilarTopics: () => Promise<void>;
  saveWatcherState: () => Promise<void>;
}

// ============================================================================
// Toast timer management (module-level, outside React)
// ============================================================================

let toastId = 0;
const toastTimers = new Map<number, ReturnType<typeof setTimeout>>();
let onboardingChecked = false;

// ============================================================================
// Store
// ============================================================================

export const useAppStore = create<AppStore>((set, get) => ({
  // =========================================================================
  // Toast Slice
  // =========================================================================
  toasts: [],

  addToast: (type, message) => {
    const id = ++toastId;
    const duration = type === 'error' ? 8000 : 4000;

    set(state => {
      const next = [...state.toasts, { id, type, message }];
      // FIFO: remove oldest if exceeding max of 3
      while (next.length > 3) {
        const removed = next.shift()!;
        const timer = toastTimers.get(removed.id);
        if (timer) {
          clearTimeout(timer);
          toastTimers.delete(removed.id);
        }
      }
      return { toasts: next };
    });

    const timer = setTimeout(() => {
      toastTimers.delete(id);
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
    }, duration);
    toastTimers.set(id, timer);
  },

  removeToast: (id) => {
    const timer = toastTimers.get(id);
    if (timer) {
      clearTimeout(timer);
      toastTimers.delete(id);
    }
    set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
  },

  // =========================================================================
  // UI Slice
  // =========================================================================
  showSettings: false,
  showSplash: true,

  setShowSettings: (show) => set({ showSettings: show }),
  setShowSplash: (show) => set({ showSplash: show }),

  // =========================================================================
  // Settings Slice
  // =========================================================================
  settings: null,
  settingsForm: { ...defaultSettingsForm },
  settingsStatus: '',
  showOnboarding: false,
  ollamaStatus: null,
  ollamaModels: [],

  setSettingsForm: (partial) => {
    set(state => ({
      settingsForm: { ...state.settingsForm, ...partial },
    }));
  },

  setSettingsFormFull: (updaterOrValue) => {
    set(state => ({
      settingsForm: typeof updaterOrValue === 'function'
        ? updaterOrValue(state.settingsForm)
        : updaterOrValue,
    }));
  },

  setSettingsStatus: (status) => set({ settingsStatus: status }),
  setShowOnboarding: (show) => set({ showOnboarding: show }),

  loadSettings: async () => {
    try {
      const s = await invoke<Settings>('get_settings');
      set(state => ({
        settings: s,
        settingsForm: {
          ...state.settingsForm,
          provider: s.llm.provider !== 'none' ? s.llm.provider : 'anthropic',
          model: s.llm.model || 'claude-3-haiku-20240307',
          baseUrl: s.llm.base_url || '',
          rerankEnabled: s.rerank.enabled,
          maxItems: s.rerank.max_items_per_batch,
          minScore: s.rerank.min_embedding_score,
          dailyTokenLimit: s.rerank.daily_token_limit,
          dailyCostLimit: s.rerank.daily_cost_limit_cents,
        },
      }));

      // Check if onboarding is needed (first run) - only check once per session
      if (!onboardingChecked) {
        onboardingChecked = true;
        const rawSettings = await invoke<Record<string, unknown>>('get_settings');
        if (!rawSettings.onboarding_complete) {
          set({ showOnboarding: true });
        }
      }
    } catch (error) {
      console.debug('Settings not available:', error);
    }
  },

  saveSettings: async () => {
    const { settingsForm, loadSettings } = get();
    set({ settingsStatus: 'Saving...' });
    try {
      await invoke('set_llm_provider', {
        provider: settingsForm.provider,
        apiKey: settingsForm.apiKey || '',
        model: settingsForm.model,
        baseUrl: settingsForm.baseUrl || null,
      });

      await invoke('set_rerank_config', {
        enabled: settingsForm.rerankEnabled,
        maxItems: settingsForm.maxItems,
        minScore: settingsForm.minScore,
        dailyTokenLimit: settingsForm.dailyTokenLimit,
        dailyCostLimit: settingsForm.dailyCostLimit,
      });

      set({ settingsStatus: 'Settings saved!' });
      await loadSettings();
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      set({ settingsStatus: `Error: ${error}` });
    }
  },

  testConnection: async () => {
    const { saveSettings, settingsForm } = get();
    const isOllama = settingsForm.provider === 'ollama';
    set({ settingsStatus: isOllama ? 'Testing Ollama connection...' : 'Testing connection...' });
    try {
      await saveSettings();

      // Race against timeout (generous for Ollama cold model loads)
      const timeoutMs = isOllama ? 90_000 : 30_000;
      const testPromise = invoke<{ success: boolean; message: string }>('test_llm_connection');
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(
          isOllama
            ? 'Ollama did not respond in time. Try restarting Ollama or using a smaller model.'
            : 'Connection timed out. Check your internet connection.',
        )), timeoutMs),
      );

      const result = await Promise.race([testPromise, timeoutPromise]);
      set({ settingsStatus: result.message });
    } catch (error) {
      set({ settingsStatus: `Connection failed: ${error}` });
    }
  },

  checkOllamaStatus: async (baseUrl?: string) => {
    try {
      const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl });
      set({ ollamaStatus: status });
      if (status.running && status.models.length > 0) {
        set({ ollamaModels: status.models });
      }
      return status;
    } catch (error) {
      console.error('Failed to check Ollama status:', error);
      const errorStatus: OllamaStatus = {
        running: false,
        version: null,
        models: [],
        base_url: baseUrl || 'http://localhost:11434',
        error: String(error),
      };
      set({ ollamaStatus: errorStatus });
      return errorStatus;
    }
  },

  // =========================================================================
  // Analysis Slice
  // =========================================================================
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
      await invoke('run_cached_analysis');
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
          appState: { ...state.appState, status: `Error: ${error}`, loading: false },
        }));
        addToast('error', `Analysis failed: ${errorMsg}`);
      }
    }
  },

  loadContextFiles: async () => {
    set(state => ({
      appState: { ...state.appState, loading: true, status: 'Loading context files...' },
    }));
    try {
      const files = await invoke<ContextFile[]>('get_context_files');
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
          appState: { ...state.appState, status: `Error: ${error}`, loading: false },
        }));
      }
    }
  },

  clearContext: async () => {
    try {
      const result = await invoke<string>('clear_context');
      const files = await invoke<ContextFile[]>('get_context_files');
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
        appState: { ...state.appState, status: `Error: ${error}` },
      }));
    }
  },

  indexContext: async () => {
    set(state => ({
      appState: { ...state.appState, loading: true, status: 'Indexing project READMEs...' },
    }));
    try {
      const readmeResult = await invoke<string>('index_project_readmes');
      set(state => ({
        appState: { ...state.appState, status: `${readmeResult}. Indexing local files...` },
      }));

      const result = await invoke<string>('index_context');
      const files = await invoke<ContextFile[]>('get_context_files');
      set(state => ({
        appState: {
          ...state.appState,
          contextFiles: files,
          status: `${readmeResult}. ${result}`,
          loading: false,
        },
      }));
    } catch (error) {
      set(state => ({
        appState: {
          ...state.appState,
          status: `Index failed: ${error}`,
          loading: false,
        },
      }));
    }
  },

  // =========================================================================
  // Filters Slice
  // =========================================================================
  sourceFilters: new Set(ALL_SOURCES),
  sortBy: 'score',
  showOnlyRelevant: false,

  toggleSourceFilter: (source) => {
    set(state => {
      const next = new Set(state.sourceFilters);
      if (next.has(source)) {
        if (next.size > 1) next.delete(source);
      } else {
        next.add(source);
      }
      return { sourceFilters: next };
    });
  },

  setSortBy: (sort) => set({ sortBy: sort }),
  setShowOnlyRelevant: (show) => set({ showOnlyRelevant: show }),

  // =========================================================================
  // Feedback Slice
  // =========================================================================
  feedbackGiven: {},
  learnedAffinities: [],
  antiTopics: [],

  loadLearnedBehavior: async () => {
    try {
      const affinityResult = await invoke<{
        affinities: TopicAffinity[];
        count: number;
      }>('ace_get_topic_affinities');

      if (affinityResult.affinities) {
        const sorted = [...affinityResult.affinities].sort(
          (a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score),
        );
        set({ learnedAffinities: sorted });
      }

      const antiResult = await invoke<{
        anti_topics: AntiTopic[];
        count: number;
      }>('ace_get_anti_topics', { min_rejections: 2 });

      if (antiResult.anti_topics) {
        set({ antiTopics: antiResult.anti_topics });
      }
    } catch (error) {
      console.debug('Learned behavior not available:', error);
    }
  },

  // =========================================================================
  // Monitoring Slice
  // =========================================================================
  monitoring: null,
  monitoringInterval: 30,

  setMonitoringInterval: (interval) => set({ monitoringInterval: interval }),

  loadMonitoringStatus: async () => {
    try {
      const status = await invoke<MonitoringStatus>('get_monitoring_status');
      set({ monitoring: status, monitoringInterval: status.interval_minutes });
    } catch (error) {
      console.debug('Monitoring status not available:', error);
    }
  },

  toggleMonitoring: async () => {
    const { monitoring, loadMonitoringStatus } = get();
    if (!monitoring) return 'Monitoring not available';
    const newEnabled = !monitoring.enabled;
    await invoke('set_monitoring_enabled', { enabled: newEnabled });
    await loadMonitoringStatus();
    return newEnabled ? 'Monitoring enabled' : 'Monitoring disabled';
  },

  updateMonitoringInterval: async () => {
    const { monitoringInterval, loadMonitoringStatus } = get();
    await invoke('set_monitoring_interval', { minutes: monitoringInterval });
    await loadMonitoringStatus();
    return `Interval set to ${monitoringInterval} minutes`;
  },

  testNotification: async () => {
    await invoke('trigger_notification_test');
    return 'Test notification sent!';
  },

  // =========================================================================
  // Briefing Slice
  // =========================================================================
  aiBriefing: { ...initialBriefingState },
  showBriefing: false,
  autoBriefingEnabled: true,

  setShowBriefing: (show) => set({ showBriefing: show }),
  setAutoBriefingEnabled: (enabled) => set({ autoBriefingEnabled: enabled }),

  generateBriefing: async () => {
    set(state => ({
      aiBriefing: { ...state.aiBriefing, loading: true, error: null },
    }));
    try {
      const result = await invoke<{
        success: boolean;
        briefing: string | null;
        error?: string;
        model?: string;
        item_count?: number;
        latency_ms?: number;
      }>('generate_ai_briefing');

      if (result.success && result.briefing) {
        set({
          aiBriefing: {
            content: result.briefing,
            loading: false,
            error: null,
            model: result.model || null,
            lastGenerated: new Date(),
          },
          showBriefing: true,
        });
      } else {
        set(state => ({
          aiBriefing: {
            ...state.aiBriefing,
            loading: false,
            error: result.error || 'Failed to generate briefing',
          },
        }));
      }
    } catch (error) {
      set(state => ({
        aiBriefing: {
          ...state.aiBriefing,
          loading: false,
          error: `Error: ${error}`,
        },
      }));
    }
  },

  // =========================================================================
  // Context Discovery Slice
  // =========================================================================
  scanDirectories: [],
  newScanDir: '',
  isScanning: false,
  discoveredContext: { tech: [], topics: [], lastScan: null },

  setNewScanDir: (dir) => set({ newScanDir: dir }),

  loadDiscoveredContext: async () => {
    try {
      const dirs = await invoke<string[]>('get_context_dirs');
      if (dirs && dirs.length > 0) {
        set({ scanDirectories: dirs });
      }

      const techResult = await invoke<{
        detected_tech: Array<{ name: string; category: string; confidence: number }>;
      }>('ace_get_detected_tech');

      if (techResult.detected_tech && techResult.detected_tech.length > 0) {
        set(state => ({
          discoveredContext: { ...state.discoveredContext, tech: techResult.detected_tech },
        }));
      }

      const topicsResult = await invoke<{
        topics: Array<{ topic: string; weight: number }>;
      }>('ace_get_active_topics');

      if (topicsResult.topics && topicsResult.topics.length > 0) {
        set(state => ({
          discoveredContext: {
            ...state.discoveredContext,
            topics: topicsResult.topics.map(t => t.topic),
          },
        }));
      }
    } catch (error) {
      console.debug('No discovered context yet:', error);
    }
  },

  runAutoDiscovery: async () => {
    const { setSettingsStatus } = get();
    set({ isScanning: true });
    setSettingsStatus('Auto-discovering your development context...');

    try {
      const result = await invoke<{
        success: boolean;
        directories_found: number;
        projects_found: number;
        directories_added: number;
        directories: string[];
        scan_result: {
          manifest_scan: { detected_tech: number; confidence: number };
          git_scan: { repos_analyzed: number; total_commits: number };
          combined: { total_topics: number; topics: string[] };
        };
      }>('ace_auto_discover');

      if (result.success) {
        set({ scanDirectories: result.directories || [] });

        const techResult = await invoke<{
          detected_tech: Array<{ name: string; category: string; confidence: number }>;
        }>('ace_get_detected_tech');

        set({
          discoveredContext: {
            tech: techResult.detected_tech || [],
            topics: result.scan_result?.combined?.topics || [],
            lastScan: new Date().toISOString(),
          },
        });

        setSettingsStatus(
          `Auto-discovered ${result.directories_found} dev directories, ${result.projects_found} projects, ${techResult.detected_tech?.length || 0} technologies`,
        );
        setTimeout(() => set({ settingsStatus: '' }), 5000);
      } else {
        setSettingsStatus('No development directories found. Add directories manually below.');
        setTimeout(() => set({ settingsStatus: '' }), 3000);
      }
    } catch (error) {
      console.error('Auto-discovery failed:', error);
      setSettingsStatus(`Auto-discovery failed: ${error}`);
    } finally {
      set({ isScanning: false });
    }
  },

  runFullScan: async () => {
    const { scanDirectories, runAutoDiscovery, setSettingsStatus } = get();

    if (scanDirectories.length === 0) {
      return runAutoDiscovery();
    }

    set({ isScanning: true });
    setSettingsStatus('Scanning directories for context...');

    try {
      const result = await invoke<{
        success: boolean;
        manifest_scan: { detected_tech: number; confidence: number };
        git_scan: { repos_analyzed: number; total_commits: number };
        combined: { total_topics: number; topics: string[] };
      }>('ace_full_scan', { paths: scanDirectories });

      const techResult = await invoke<{
        detected_tech: Array<{ name: string; category: string; confidence: number }>;
      }>('ace_get_detected_tech');

      set({
        discoveredContext: {
          tech: techResult.detected_tech || [],
          topics: result.combined?.topics || [],
          lastScan: new Date().toISOString(),
        },
      });

      setSettingsStatus(
        `Scan complete: ${techResult.detected_tech?.length || 0} technologies, ${result.combined?.total_topics || 0} topics discovered`,
      );
      setTimeout(() => set({ settingsStatus: '' }), 3000);
    } catch (error) {
      console.error('Full scan failed:', error);
      setSettingsStatus(`Scan failed: ${error}`);
    } finally {
      set({ isScanning: false });
    }
  },

  addScanDirectory: async () => {
    const { newScanDir, scanDirectories, setSettingsStatus } = get();
    const dirToAdd = newScanDir.trim();
    if (!dirToAdd) {
      setSettingsStatus('Please enter a directory path');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      return;
    }
    if (scanDirectories.includes(dirToAdd)) {
      setSettingsStatus('Directory already added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      return;
    }

    const newDirs = [...scanDirectories, dirToAdd];

    try {
      await invoke('set_context_dirs', { dirs: newDirs });
      set({ scanDirectories: newDirs, newScanDir: '' });
      setSettingsStatus(`Added: ${dirToAdd}`);
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setSettingsStatus(`Error: ${errorMsg}`);
      console.error('Failed to add directory:', error);
    }
  },

  removeScanDirectory: async (dir) => {
    const { scanDirectories, setSettingsStatus } = get();
    const newDirs = scanDirectories.filter(d => d !== dir);
    try {
      await invoke('set_context_dirs', { dirs: newDirs });
      set({ scanDirectories: newDirs });
      setSettingsStatus(`Removed: ${dir}`);
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setSettingsStatus(`Error removing: ${errorMsg}`);
      console.error('Failed to remove directory:', error);
    }
  },

  // =========================================================================
  // User Context Slice
  // =========================================================================
  userContext: null,
  suggestedInterests: [],
  newInterest: '',
  newExclusion: '',
  newTechStack: '',
  newRole: '',

  setNewInterest: (v) => set({ newInterest: v }),
  setNewExclusion: (v) => set({ newExclusion: v }),
  setNewTechStack: (v) => set({ newTechStack: v }),
  setNewRole: (v) => set({ newRole: v }),

  loadUserContext: async () => {
    try {
      const ctx = await invoke<UserContext>('get_user_context');
      set({ userContext: ctx });
      if (ctx.role) set({ newRole: ctx.role });
    } catch (error) {
      console.debug('Context not available:', error);
    }
  },

  loadSuggestedInterests: async () => {
    try {
      const suggestions = await invoke<SuggestedInterest[]>('ace_get_suggested_interests');
      set({ suggestedInterests: suggestions });
    } catch (error) {
      console.debug('Suggested interests not available:', error);
    }
  },

  addInterest: async () => {
    const { newInterest, loadUserContext, setSettingsStatus } = get();
    if (!newInterest.trim()) return;
    try {
      await invoke('add_interest', { topic: newInterest.trim() });
      set({ newInterest: '' });
      await loadUserContext();
      setSettingsStatus('Interest added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeInterest: async (topic) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_interest', { topic });
      await loadUserContext();
      setSettingsStatus('Interest removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  addExclusion: async () => {
    const { newExclusion, loadUserContext, setSettingsStatus } = get();
    if (!newExclusion.trim()) return;
    try {
      await invoke('add_exclusion', { topic: newExclusion.trim() });
      set({ newExclusion: '' });
      await loadUserContext();
      setSettingsStatus('Exclusion added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeExclusion: async (topic) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_exclusion', { topic });
      await loadUserContext();
      setSettingsStatus('Exclusion removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  addTechStack: async () => {
    const { newTechStack, loadUserContext, setSettingsStatus } = get();
    if (!newTechStack.trim()) return;
    try {
      await invoke('add_tech_stack', { technology: newTechStack.trim() });
      set({ newTechStack: '' });
      await loadUserContext();
      setSettingsStatus('Technology added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeTechStack: async (technology) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_tech_stack', { technology });
      await loadUserContext();
      setSettingsStatus('Technology removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  updateRole: async () => {
    const { newRole, loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('set_user_role', { role: newRole.trim() || null });
      await loadUserContext();
      setSettingsStatus('Role updated');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  // =========================================================================
  // System Health Slice
  // =========================================================================
  systemHealth: null,
  similarTopicQuery: '',
  similarTopicResults: [],

  setSimilarTopicQuery: (q) => set({ similarTopicQuery: q }),

  loadSystemHealth: async () => {
    let anomalies: Anomaly[] = [];
    let anomalyCount = 0;
    let embeddingOperational = false;
    let rateLimitStatus = null;
    let accuracyMetrics = null;

    try {
      const result = await invoke<{ anomalies: Anomaly[]; count: number }>('ace_get_unresolved_anomalies');
      anomalies = result.anomalies || [];
      anomalyCount = result.count || 0;
    } catch (error) {
      console.debug('Anomalies not available:', error);
    }

    try {
      const result = await invoke<{ operational: boolean }>('ace_embedding_status');
      embeddingOperational = result.operational ?? false;
    } catch (error) {
      console.error('Embedding status error:', error);
    }

    try {
      const result = await invoke<{ global_remaining: number; source_remaining: number; is_limited: boolean }>(
        'ace_get_rate_limit_status',
        { source: 'global' },
      );
      rateLimitStatus = result;
    } catch (error) {
      console.error('Rate limit status error:', error);
    }

    try {
      const result = await invoke<{ precision: number; engagement_rate: number; calibration_error: number }>(
        'ace_get_accuracy_metrics',
      );
      accuracyMetrics = result;
    } catch (error) {
      console.debug('Accuracy metrics not available:', error);
    }

    set({
      systemHealth: {
        anomalies,
        anomalyCount,
        embeddingOperational,
        rateLimitStatus,
        accuracyMetrics,
      },
    });
  },

  runAnomalyDetection: async () => {
    const { loadSystemHealth, setSettingsStatus } = get();
    try {
      setSettingsStatus('Running anomaly detection...');
      const result = await invoke<{ anomalies: Anomaly[]; count: number }>('ace_detect_anomalies');
      await loadSystemHealth();
      setSettingsStatus(`Found ${result.count} anomalies`);
      setTimeout(() => set({ settingsStatus: '' }), 3000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  resolveAnomaly: async (anomalyId) => {
    const { loadSystemHealth, setSettingsStatus } = get();
    try {
      await invoke('ace_resolve_anomaly', { anomalyId });
      setSettingsStatus('Anomaly resolved');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      await loadSystemHealth();
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  findSimilarTopics: async () => {
    const { similarTopicQuery, setSettingsStatus } = get();
    if (!similarTopicQuery.trim()) return;
    try {
      const result = await invoke<{ query: string; results: SimilarTopicResult[] }>(
        'ace_find_similar_topics',
        { query: similarTopicQuery.trim(), topK: 5 },
      );
      set({ similarTopicResults: result.results || [] });
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  saveWatcherState: async () => {
    const { setSettingsStatus } = get();
    try {
      await invoke('ace_save_watcher_state');
      setSettingsStatus('Watcher state saved');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },
}));
