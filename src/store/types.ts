// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type {
  ContextFile,
  SourceRelevance,
} from '../types';
import type { AutophagySlice } from './autophagy-slice';
import type { DecisionAdvantageSlice } from './decision-advantage-slice';
import type { ChannelsSlice } from './channels-slice';
import type { IntelligencePulseSlice } from './intelligence-pulse-slice';
import type { TeamSlice } from './team-slice';
import type { EnterpriseSlice } from './enterprise-slice';
import type { TeamIntelligenceSlice } from './team-intelligence-slice';
import type { PreemptionSlice } from './preemption-slice';
import type { BlindSpotsSlice } from './blind-spots-slice';
import type { TrustSlice } from './trust-slice';
import type {
  ToastSlice,
  UiSlice,
  SettingsSlice,
  AnalysisSlice,
  FiltersSlice,
  FeedbackSlice,
  MonitoringSlice,
  BriefingSlice,
  ContextDiscoverySlice,
  UserContextSlice,
  SystemHealthSlice,
  DecisionsSlice,
  AgentSlice,
  LicenseSlice,
  ToolkitSlice,
} from './slice-types';

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
  has_embedding_model?: boolean;
  has_llm_model?: boolean;
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
  /** Items that almost passed the relevance threshold (for zero-result guidance) */
  nearMisses: SourceRelevance[] | null;
  status: string;
  loading: boolean;
  analysisComplete: boolean;
  progress: number;
  progressMessage: string;
  progressStage: string;
  lastAnalyzedAt: Date | null;
}

// ============================================================================
// Re-export Slice Types
// ============================================================================

export type {
  ToastSlice,
  EmbeddingStatus,
  ActiveView,
  UiSlice,
  ToolkitSlice,
  ModelRegistryData,
  SettingsSlice,
  AnalysisSlice,
  FiltersSlice,
  FeedbackSlice,
  MonitoringSlice,
  FreeBriefingData,
  InstantBriefingSnapshot,
  MorningBriefData,
  BriefingSlice,
  ContextDiscoverySlice,
  UserContextSlice,
  SystemHealthSlice,
  DecisionsSlice,
  AgentSlice,
  TrialStatus,
  LicenseSlice,
} from './slice-types';

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
  & AgentSlice
  & LicenseSlice
  & ToolkitSlice
  & AutophagySlice
  & DecisionAdvantageSlice
  & ChannelsSlice
  & IntelligencePulseSlice
  & TeamSlice
  & EnterpriseSlice
  & TeamIntelligenceSlice
  & PreemptionSlice
  & BlindSpotsSlice
  & TrustSlice;
