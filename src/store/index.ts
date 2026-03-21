import { create } from 'zustand';
import type { AppStore } from './types';
import { createToastSlice } from './toast-slice';
import { createUiSlice } from './ui-slice';
import { createSettingsSlice } from './settings-slice';
import { createAnalysisSlice } from './analysis-slice';
import { createFiltersSlice } from './filters-slice';
import { createFeedbackSlice } from './feedback-slice';
import { createMonitoringSlice } from './monitoring-slice';
import { createBriefingSlice } from './briefing-slice';
import { createContextDiscoverySlice } from './context-discovery-slice';
import { createUserContextSlice } from './user-context-slice';
import { createSystemHealthSlice } from './system-health-slice';
import { createDecisionsSlice } from './decisions-slice';
import { createAgentSlice } from './agent-slice';
import { createLicenseSlice } from './license-slice';
import { createToolkitSlice } from './toolkit-slice';
import { createPlaybookSlice } from './playbook-slice';
import { createSovereignProfileSlice } from './sovereign-profile-slice';
import { createAutophagySlice } from './autophagy-slice';
import { createDecisionAdvantageSlice } from './decision-advantage-slice';
import { createChannelsSlice } from './channels-slice';
import { createUnifiedProfileSlice } from './unified-profile-slice';
import { createIntelligencePulseSlice } from './intelligence-pulse-slice';
import { createTeamSlice } from './team-slice';
import { createEnterpriseSlice } from './enterprise-slice';
import { createTeamIntelligenceSlice } from './team-intelligence-slice';

// Re-export all types
export type {
  ToastType,
  ToastAction,
  Toast,
  SettingsForm,
  OllamaStatus,
  DiscoveredContext,
  TopicAffinity,
  AntiTopic,
  SimilarTopicResult,
  BriefingState,
  AppState,
  AppStore,
  LicenseSlice,
  ToolkitSlice,
  EmbeddingStatus,
} from './types';

export const useAppStore = create<AppStore>()((...a) => ({
  ...createToastSlice(...a),
  ...createUiSlice(...a),
  ...createSettingsSlice(...a),
  ...createAnalysisSlice(...a),
  ...createFiltersSlice(...a),
  ...createFeedbackSlice(...a),
  ...createMonitoringSlice(...a),
  ...createBriefingSlice(...a),
  ...createContextDiscoverySlice(...a),
  ...createUserContextSlice(...a),
  ...createSystemHealthSlice(...a),
  ...createDecisionsSlice(...a),
  ...createAgentSlice(...a),
  ...createLicenseSlice(...a),
  ...createToolkitSlice(...a),
  ...createPlaybookSlice(...a),
  ...createSovereignProfileSlice(...a),
  ...createAutophagySlice(...a),
  ...createDecisionAdvantageSlice(...a),
  ...createChannelsSlice(...a),
  ...createUnifiedProfileSlice(...a),
  ...createIntelligencePulseSlice(...a),
  ...createTeamSlice(...a),
  ...createEnterpriseSlice(...a),
  ...createTeamIntelligenceSlice(...a),
}));
