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
}));
