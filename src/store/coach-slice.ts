import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';
import type {
  CoachSession,
  CoachMessage,
  EngineRecommendation,
  LaunchReviewResult,
  CoachNudge,
  CoachTemplate,
  VideoLesson,
  VideoCurriculumStatus,
  StreetsTier,
  CoachSessionType,
} from '../types/coach';

// ============================================================================
// Slice Interface
// ============================================================================

export interface CoachSlice {
  // State
  streetsTier: StreetsTier;
  coachSessions: CoachSession[];
  activeSessionId: string | null;
  coachMessages: CoachMessage[];
  coachLoading: boolean;
  engineRecommendation: EngineRecommendation | null;
  strategyDocument: string | null;
  launchReview: LaunchReviewResult | null;
  coachNudges: CoachNudge[];
  templates: CoachTemplate[];
  videoCurriculum: VideoLesson[];
  videoStatus: VideoCurriculumStatus | null;

  // Actions
  loadStreetsTier: () => Promise<void>;
  activateStreetsLicense: (key: string) => Promise<boolean>;
  loadCoachSessions: () => Promise<void>;
  createCoachSession: (type: CoachSessionType, title?: string) => Promise<string | null>;
  deleteCoachSession: (id: string) => Promise<void>;
  setActiveSession: (id: string | null) => void;
  sendCoachMessage: (content: string) => Promise<void>;
  loadCoachHistory: (sessionId: string) => Promise<void>;
  recommendEngines: () => Promise<void>;
  generateStrategy: () => Promise<void>;
  submitLaunchReview: (description: string) => Promise<void>;
  progressCheckIn: () => Promise<void>;
  loadCoachNudges: () => Promise<void>;
  dismissNudge: (id: number) => Promise<void>;
  loadTemplates: () => Promise<void>;
  loadVideoCurriculum: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

export const createCoachSlice: StateCreator<AppStore, [], [], CoachSlice> = (set, get) => ({
  // Initial state
  streetsTier: 'playbook',
  coachSessions: [],
  activeSessionId: null,
  coachMessages: [],
  coachLoading: false,
  engineRecommendation: null,
  strategyDocument: null,
  launchReview: null,
  coachNudges: [],
  templates: [],
  videoCurriculum: [],
  videoStatus: null,

  loadStreetsTier: async () => {
    try {
      const result = await invoke<{ tier: string; expired?: boolean }>('get_streets_tier');
      set({ streetsTier: (result.expired ? 'playbook' : result.tier) as StreetsTier });
    } catch {
      set({ streetsTier: 'playbook' });
    }
  },

  activateStreetsLicense: async (key: string) => {
    try {
      const result = await invoke<{ success: boolean; streets_tier: string; tier: string }>(
        'activate_streets_license',
        { licenseKey: key },
      );
      if (result.success) {
        set({
          streetsTier: result.streets_tier as StreetsTier,
          // STREETS licenses also grant Pro tier
          tier: result.tier as 'free' | 'pro' | 'team',
        });
        return true;
      }
      return false;
    } catch {
      return false;
    }
  },

  loadCoachSessions: async () => {
    try {
      const sessions = await invoke<CoachSession[]>('coach_list_sessions');
      set({ coachSessions: sessions });
    } catch { /* non-fatal */ }
  },

  createCoachSession: async (type: CoachSessionType, title?: string) => {
    try {
      const session = await invoke<CoachSession>('coach_create_session', {
        sessionType: type,
        title: title || undefined,
      });
      set(s => ({
        coachSessions: [session, ...s.coachSessions],
        activeSessionId: session.id,
        coachMessages: [],
      }));
      return session.id;
    } catch {
      return null;
    }
  },

  deleteCoachSession: async (id: string) => {
    try {
      await invoke('coach_delete_session', { sessionId: id });
      set(s => ({
        coachSessions: s.coachSessions.filter(sess => sess.id !== id),
        activeSessionId: s.activeSessionId === id ? null : s.activeSessionId,
        coachMessages: s.activeSessionId === id ? [] : s.coachMessages,
      }));
    } catch { /* non-fatal */ }
  },

  setActiveSession: (id: string | null) => {
    set({ activeSessionId: id, coachMessages: [] });
    if (id) get().loadCoachHistory(id);
  },

  sendCoachMessage: async (content: string) => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;

    // Optimistic: add user message immediately
    const tempMsg: CoachMessage = {
      id: Date.now(),
      session_id: sessionId,
      role: 'user',
      content,
      token_count: 0,
      cost_cents: 0,
      created_at: new Date().toISOString(),
    };
    set(s => ({ coachMessages: [...s.coachMessages, tempMsg], coachLoading: true }));

    try {
      const response = await invoke<CoachMessage>('coach_send_message', { sessionId, content });
      set(s => ({
        coachMessages: [...s.coachMessages, response],
        coachLoading: false,
      }));
    } catch {
      set({ coachLoading: false });
    }
  },

  loadCoachHistory: async (sessionId: string) => {
    try {
      const messages = await invoke<CoachMessage[]>('coach_get_history', { sessionId });
      set({ coachMessages: messages });
    } catch {
      set({ coachMessages: [] });
    }
  },

  recommendEngines: async () => {
    set({ coachLoading: true, engineRecommendation: null });
    try {
      const rec = await invoke<EngineRecommendation>('coach_recommend_engines');
      set({ engineRecommendation: rec, coachLoading: false });
    } catch {
      set({ coachLoading: false });
    }
  },

  generateStrategy: async () => {
    set({ coachLoading: true, strategyDocument: null });
    try {
      const doc = await invoke<string>('coach_generate_strategy');
      set({ strategyDocument: doc, coachLoading: false });
    } catch {
      set({ coachLoading: false });
    }
  },

  submitLaunchReview: async (description: string) => {
    set({ coachLoading: true, launchReview: null });
    try {
      const review = await invoke<LaunchReviewResult>('coach_launch_review', {
        projectDescription: description,
      });
      set({ launchReview: review, coachLoading: false });
    } catch {
      set({ coachLoading: false });
    }
  },

  progressCheckIn: async () => {
    set({ coachLoading: true });
    try {
      const content = await invoke<string>('coach_progress_check_in');
      set(s => ({
        coachMessages: [...s.coachMessages, {
          id: Date.now(),
          session_id: 'progress',
          role: 'assistant' as const,
          content,
          token_count: 0,
          cost_cents: 0,
          created_at: new Date().toISOString(),
        }],
        coachLoading: false,
      }));
    } catch {
      set({ coachLoading: false });
    }
  },

  loadCoachNudges: async () => {
    try {
      const nudges = await invoke<CoachNudge[]>('get_coach_nudges');
      set({ coachNudges: nudges });
    } catch { /* non-fatal */ }
  },

  dismissNudge: async (id: number) => {
    try {
      await invoke('dismiss_coach_nudge', { nudgeId: id });
      set(s => ({ coachNudges: s.coachNudges.filter(n => n.id !== id) }));
    } catch { /* non-fatal */ }
  },

  loadTemplates: async () => {
    try {
      const templates = await invoke<CoachTemplate[]>('get_templates');
      set({ templates });
    } catch { /* non-fatal */ }
  },

  loadVideoCurriculum: async () => {
    try {
      const [lessons, status] = await invoke<[VideoLesson[], VideoCurriculumStatus]>(
        'get_video_curriculum',
      );
      set({ videoCurriculum: lessons, videoStatus: status });
    } catch { /* non-fatal */ }
  },
});
