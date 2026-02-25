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
import { createCommandDeckSlice } from './command-deck-slice';
import { createSovereignProfileSlice } from './sovereign-profile-slice';
import { createSunsSlice } from './suns-slice';
import { createCoachSlice } from './coach-slice';
import { createAutophagySlice } from './autophagy-slice';
import { createDecisionAdvantageSlice } from './decision-advantage-slice';

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
  ...createCommandDeckSlice(...a),
  ...createSovereignProfileSlice(...a),
  ...createSunsSlice(...a),
  ...createCoachSlice(...a),
  ...createAutophagySlice(...a),
  ...createDecisionAdvantageSlice(...a),
}));
