// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type {
  Settings,
  MonitoringStatus,
  UserContext,
  SystemHealth,
  SourceRelevance,
  FeedbackAction,
  FeedbackGiven,
  SuggestedInterest,
  SourceHealthStatus,
  ProValueReport,
} from '../types';
import type { DeveloperDecision } from './decisions-slice';
import type { AgentMemoryEntry, DelegationScoreEntry } from './agent-slice';
import type {
  Toast,
  ToastType,
  ToastAction,
  SettingsForm,
  OllamaStatus,
  AppState,
  DiscoveredContext,
  TopicAffinity,
  AntiTopic,
  SimilarTopicResult,
  BriefingState,
} from './types';

// ============================================================================
// Slice Interfaces
// ============================================================================

export interface ToastSlice {
  toasts: Toast[];
  addToast: (type: ToastType, message: string, action?: ToastAction) => void;
  removeToast: (id: number) => void;
}

export type EmbeddingStatus = 'active' | 'degraded' | 'unavailable';

export type ActiveView =
  | 'briefing'
  | 'results'
  | 'preemption'
  | 'blindspots'
  | 'playbook';

export type SignalViewMode = 'list' | 'graph';

export interface UiSlice {
  showSettings: boolean;
  showSplash: boolean;
  activeView: ActiveView;
  signalViewMode: SignalViewMode;
  isFirstRun: boolean;
  firstRunDismissed: boolean;
  embeddingMode: 'semantic' | 'keyword-only' | null;
  embeddingStatus: EmbeddingStatus | undefined;
  /** Item id the command search asked the Signal view to scroll to + expand; null when none pending. */
  searchFocusItemId: number | null;
  setShowSettings: (show: boolean) => void;
  setShowSplash: (show: boolean) => void;
  setActiveView: (view: ActiveView) => void;
  setSignalViewMode: (mode: SignalViewMode) => void;
  setIsFirstRun: (v: boolean) => void;
  setFirstRunDismissed: (v: boolean) => void;
  setEmbeddingMode: (mode: 'semantic' | 'keyword-only' | null) => void;
  setEmbeddingStatus: (status: EmbeddingStatus | undefined) => void;
  setSearchFocusItemId: (id: number | null) => void;
}

export interface ToolkitSlice {
  recentTools: string[];
  pinnedTools: string[];
  addRecentTool: (toolId: string) => void;
  togglePinnedTool: (toolId: string) => void;
}

export interface ModelRegistryData {
  fetched_at: number;
  source: string;
  model_count: number;
  providers: Record<
    string,
    Array<{
      id: string;
      provider: string;
      display_name: string;
      input_cost_per_token: number | null;
      output_cost_per_token: number | null;
      max_input_tokens: number | null;
      max_output_tokens: number | null;
    }>
  >;
}

export interface SettingsSlice {
  settings: Settings | null;
  settingsForm: SettingsForm;
  settingsStatus: string;
  showOnboarding: boolean;
  /** True once the initial loadSettings() resolves (success OR failure). Gates
   *  the splash so the app shell never flashes before the onboarding decision
   *  is known on a slow first-run IPC. */
  settingsLoaded: boolean;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  modelRegistry: ModelRegistryData | null;
  setSettingsForm: (partial: Partial<SettingsForm>) => void;
  setSettingsFormFull: (
    updaterOrValue: SettingsForm | ((prev: SettingsForm) => SettingsForm),
  ) => void;
  setSettingsStatus: (status: string) => void;
  setShowOnboarding: (show: boolean) => void;
  loadSettings: () => Promise<void>;
  saveSettings: () => Promise<void>;
  testConnection: () => Promise<void>;
  checkOllamaStatus: (baseUrl?: string) => Promise<OllamaStatus>;
  refreshModelRegistry: () => Promise<void>;
}

export interface AnalysisSlice {
  appState: AppState;
  expandedItem: number | null;
  isBrowserMode: boolean;
  setExpandedItem: (id: number | null) => void;
  setAppState: (partial: Partial<AppState>) => void;
  setAppStateFull: (
    updaterOrValue: AppState | ((prev: AppState) => AppState),
  ) => void;
  startAnalysis: () => Promise<void>;
  loadContextFiles: () => Promise<void>;
  clearContext: () => Promise<void>;
  indexContext: () => Promise<void>;
}

export interface FiltersSlice {
  sourceFilters: Set<string>;
  sortBy:
    | 'score'
    | 'date'
    | 'priority'
    | 'applicability'
    | 'freshness'
    | 'urgency';
  showOnlyRelevant: boolean;
  searchQuery: string;
  showSavedOnly: boolean;
  toggleSourceFilter: (source: string) => void;
  resetSourceFilters: () => void;
  setSortBy: (
    sort:
      | 'score'
      | 'date'
      | 'priority'
      | 'applicability'
      | 'freshness'
      | 'urgency',
  ) => void;
  setShowOnlyRelevant: (show: boolean) => void;
  setSearchQuery: (q: string) => void;
  setShowSavedOnly: (show: boolean) => void;
}

export interface FeedbackSlice {
  feedbackGiven: FeedbackGiven;
  learnedAffinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  lastLearnedTopic: {
    topic: string;
    direction: 'positive' | 'negative';
    timestamp: number;
  } | null;
  setLastLearnedTopic: (
    topic: {
      topic: string;
      direction: 'positive' | 'negative';
      timestamp: number;
    } | null,
  ) => void;
  setFeedbackGivenFull: (
    updater: FeedbackGiven | ((prev: FeedbackGiven) => FeedbackGiven),
  ) => void;
  loadLearnedBehavior: () => Promise<void>;
  loadPersistedSavedIds: () => Promise<void>;
  recordInteraction: (
    itemId: number,
    actionType: FeedbackAction,
    item: SourceRelevance,
  ) => Promise<void>;
}

export interface MonitoringSlice {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  notificationThreshold: string;
  setMonitoringInterval: (interval: number) => void;
  setNotificationThreshold: (threshold: string) => Promise<void>;
  loadMonitoringStatus: () => Promise<void>;
  toggleMonitoring: () => Promise<string>;
  updateMonitoringInterval: () => Promise<string>;
  testNotification: () => Promise<string>;
}

export interface FreeBriefingData {
  success: boolean;
  empty: boolean;
  message?: string;
  top_items?: Array<{
    title: string;
    url: string | null;
    source: string;
    score: string;
    signal_priority?: string | null;
  }>;
  stack_alerts?: Array<{
    title: string;
    url: string | null;
    source: string;
  }>;
  source_summary?: Record<string, number>;
  signal_priorities?: Record<string, number>;
  knowledge_gaps?: Array<{ topic: string; days_since_last: number }>;
  wisdom_signals?: Array<{
    text: string;
    confidence: number;
    signal_type: string;
  }>;
  total_items?: number;
  generated_at?: string;
}

/**
 * Sovereign Cold Boot — pre-baked briefing snapshot loaded BEFORE React mounts
 * for instant first-paint. The Rust side (briefing_snapshot.rs) writes this
 * file at shutdown and after each morning briefing fires; main.tsx reads it
 * synchronously via the get_briefing_snapshot Tauri command and stashes the
 * result on window.__4DA_INSTANT_SNAPSHOT__ before ReactDOM.createRoot.
 *
 * The frontend renders the snapshot as the first paint with a freshness
 * banner, then transparently swaps in fresh data when the backend catches up.
 */
export interface InstantBriefingSnapshot {
  /** Format version. v1 at present; older versions are silently ignored. */
  version: number;
  /** Unix timestamp when this snapshot was generated. */
  generatedAtUnix: number;
  /** Pre-formatted display string, e.g. "Mon Apr 7, 9:14 AM". */
  generatedAtDisplay: string;
  /** Briefing payload. */
  title: string;
  items: Array<{
    title: string;
    sourceType: string;
    score: number;
    signalType?: string | null;
    url?: string | null;
    itemId?: number | null;
    signalPriority?: string | null;
    description?: string | null;
    matchedDeps?: string[];
  }>;
  totalRelevant: number;
  /** LLM-synthesized narrative — populated when previous session had time to compute it. */
  synthesis?: string | null;
  wisdomSynthesis?: string | null;
}

/** Source data freshness summary — signals when the pipeline has gone silent. */
export interface DataFreshness {
  newest_item_age_hours: number | null;
  items_last_24h: number;
  items_last_72h: number;
  newest_source_check_age_hours: number | null;
  source_checks_last_24h: number;
  source_checks_last_72h: number;
  failing_sources: number;
  stale_sources: number;
  is_stale: boolean;
  no_recent_fetches: boolean;
}

/** Live morning briefing items received from the T+3s startup check.
 *  Fills the render gap between snapshot and analysis completion. */
export interface MorningBriefData {
  title: string;
  totalRelevant: number;
  items: Array<{
    title: string;
    sourceType: string;
    score: number;
    signalType?: string | null;
  }>;
  /** When present and is_stale is true, the data pipeline is silent. */
  dataFreshness?: DataFreshness | null;
}

export interface SynthesisCluster {
  insight: string;
  evidence_ids: number[];
  action: string;
  confidence: number;
}

export interface BriefingSlice {
  aiBriefing: BriefingState;
  autoBriefingEnabled: boolean;
  lastBackgroundResultsAt: Date | null;
  sourceHealth: SourceHealthStatus[];
  freeBriefing: FreeBriefingData | null;
  freeBriefingLoading: boolean;
  morningBriefSynthesis: string | null;
  morningBriefClusters: SynthesisCluster[] | null;
  /** Live morning briefing data from the T+3s startup check. Superseded
   *  once analysis completes and freeBriefing or aiBriefing populates. */
  morningBriefData: MorningBriefData | null;
  /** Pre-baked briefing from the previous session. Populated on app boot
   *  via window.__4DA_INSTANT_SNAPSHOT__ for sub-200ms first paint.
   *  Naturally superseded by the render waterfall — never explicitly cleared. */
  instantSnapshot: InstantBriefingSnapshot | null;
  setMorningBriefSynthesis: (synthesis: string | null) => void;
  setMorningBriefClusters: (clusters: SynthesisCluster[] | null) => void;
  setMorningBriefData: (data: MorningBriefData | null) => void;
  setAutoBriefingEnabled: (enabled: boolean) => void;
  setLastBackgroundResultsAt: (date: Date) => void;
  setInstantSnapshot: (snapshot: InstantBriefingSnapshot | null) => void;
  generateBriefing: () => Promise<void>;
  generateFreeBriefing: () => Promise<void>;
  loadPersistedBriefing: () => Promise<void>;
  loadSourceHealth: () => Promise<void>;
}

export interface ContextDiscoverySlice {
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
}

export interface UserContextSlice {
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
}

export interface SystemHealthSlice {
  systemHealth: SystemHealth | null;
  similarTopicQuery: string;
  similarTopicResults: SimilarTopicResult[];
  startupHealthIssues: Array<{ component: string; severity: 'warning' | 'error'; message: string }> | null;
  setSimilarTopicQuery: (q: string) => void;
  loadStartupHealth: () => Promise<void>;
  loadSystemHealth: () => Promise<void>;
  runAnomalyDetection: () => Promise<void>;
  resolveAnomaly: (anomalyId: number) => Promise<void>;
  findSimilarTopics: () => Promise<void>;
  saveWatcherState: () => Promise<void>;
}

export interface DecisionsSlice {
  decisions: DeveloperDecision[];
  decisionsLoading: boolean;
  decisionsError: string | null;
  loadDecisions: () => Promise<void>;
  recordDecision: (params: {
    decision_type: string;
    subject: string;
    decision: string;
    rationale?: string;
    alternatives_rejected?: string[];
    context_tags?: string[];
    confidence?: number;
  }) => Promise<void>;
  updateDecision: (
    id: number,
    updates: {
      decision?: string;
      rationale?: string;
      status?: string;
      confidence?: number;
    },
  ) => Promise<void>;
  removeTechDecision: (technology: string) => Promise<void>;
}

export interface AgentSlice {
  agentMemories: AgentMemoryEntry[];
  delegationScores: DelegationScoreEntry[];
  agentDataExists: boolean;
  agentMemoryLoading: boolean;
  loadAgentMemories: () => Promise<void>;
  loadDelegationScores: () => Promise<void>;
  checkAgentDataExists: () => Promise<void>;
  promoteMemoryToDecision: (memoryId: number) => Promise<void>;
}

export interface TrialStatus {
  active: boolean;
  days_remaining: number;
  started_at: string | null;
  has_license: boolean;
}

export interface LicenseSlice {
  tier: 'free' | 'pro' | 'signal' | 'team' | 'enterprise';
  licenseKey: string;
  licenseLoading: boolean;
  wasDowngraded: boolean;
  trialStatus: TrialStatus | null;
  expiresAt: string | null;
  daysRemaining: number;
  expired: boolean;
  proValueReport: ProValueReport | null;
  loadLicense: () => Promise<void>;
  activateLicense: (
    key: string,
  ) => Promise<{ ok: boolean; reason?: string }>;
  recoverLicenseByEmail: (
    email: string,
  ) => Promise<{ ok: boolean; reason?: string; tier?: string }>;
  loadTrialStatus: () => Promise<void>;
  startTrial: () => Promise<boolean>;
  isPro: () => boolean;
  loadProValueReport: () => Promise<void>;
}
