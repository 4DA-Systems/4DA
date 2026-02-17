import type {
  Settings,
  MonitoringStatus,
  UserContext,
  SystemHealth,
  ContextFile,
  SourceRelevance,
  FeedbackAction,
  FeedbackGiven,
  SuggestedInterest,
  SourceHealthStatus,
} from '../types';

// ============================================================================
// Shared Types
// ============================================================================

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  type: ToastType;
  message: string;
  action?: ToastAction;
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
// Slice Interfaces
// ============================================================================

export interface ToastSlice {
  toasts: Toast[];
  addToast: (type: ToastType, message: string, action?: ToastAction) => void;
  removeToast: (id: number) => void;
}

export interface UiSlice {
  showSettings: boolean;
  showSplash: boolean;
  activeView: 'briefing' | 'results' | 'saved';
  setShowSettings: (show: boolean) => void;
  setShowSplash: (show: boolean) => void;
  setActiveView: (view: 'briefing' | 'results' | 'saved') => void;
}

export interface SettingsSlice {
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
}

export interface AnalysisSlice {
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
}

export interface FiltersSlice {
  sourceFilters: Set<string>;
  sortBy: 'score' | 'date';
  showOnlyRelevant: boolean;
  searchQuery: string;
  showSavedOnly: boolean;
  toggleSourceFilter: (source: string) => void;
  setSortBy: (sort: 'score' | 'date') => void;
  setShowOnlyRelevant: (show: boolean) => void;
  setSearchQuery: (q: string) => void;
  setShowSavedOnly: (show: boolean) => void;
}

export interface FeedbackSlice {
  feedbackGiven: FeedbackGiven;
  learnedAffinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  lastLearnedTopic: { topic: string; direction: 'positive' | 'negative'; timestamp: number } | null;
  setLastLearnedTopic: (topic: { topic: string; direction: 'positive' | 'negative'; timestamp: number } | null) => void;
  setFeedbackGivenFull: (updater: FeedbackGiven | ((prev: FeedbackGiven) => FeedbackGiven)) => void;
  loadLearnedBehavior: () => Promise<void>;
  loadPersistedSavedIds: () => Promise<void>;
  recordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => Promise<void>;
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

export interface BriefingSlice {
  aiBriefing: BriefingState;
  showBriefing: boolean;
  autoBriefingEnabled: boolean;
  lastBackgroundResultsAt: Date | null;
  sourceHealth: SourceHealthStatus[];
  setShowBriefing: (show: boolean) => void;
  setAutoBriefingEnabled: (enabled: boolean) => void;
  setLastBackgroundResultsAt: (date: Date) => void;
  generateBriefing: () => Promise<void>;
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
  setSimilarTopicQuery: (q: string) => void;
  loadSystemHealth: () => Promise<void>;
  runAnomalyDetection: () => Promise<void>;
  resolveAnomaly: (anomalyId: number) => Promise<void>;
  findSimilarTopics: () => Promise<void>;
  saveWatcherState: () => Promise<void>;
}

export interface DecisionsSlice {
  decisions: import('./decisions-slice').DeveloperDecision[];
  decisionsLoading: boolean;
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
  updateDecision: (id: number, updates: {
    decision?: string;
    rationale?: string;
    status?: string;
    confidence?: number;
  }) => Promise<void>;
}

export interface AgentSlice {
  agentMemories: import('./agent-slice').AgentMemoryEntry[];
  delegationScores: import('./agent-slice').DelegationScoreEntry[];
  agentDataExists: boolean;
  agentMemoryLoading: boolean;
  loadAgentMemories: () => Promise<void>;
  loadDelegationScores: () => Promise<void>;
  checkAgentDataExists: () => Promise<void>;
  promoteMemoryToDecision: (memoryId: number) => Promise<void>;
}

// ============================================================================
// Combined Store Type
// ============================================================================

export type AppStore =
  & ToastSlice
  & UiSlice
  & SettingsSlice
  & AnalysisSlice
  & FiltersSlice
  & FeedbackSlice
  & MonitoringSlice
  & BriefingSlice
  & ContextDiscoverySlice
  & UserContextSlice
  & SystemHealthSlice
  & DecisionsSlice
  & AgentSlice;
