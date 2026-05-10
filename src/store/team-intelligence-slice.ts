// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore } from './types';
import type {
  DecisionDetail,
  NotificationSummary,
  SharedSource,
  TeamDecision,
  TeamNotification,
  TeamProfile,
  TeamSignalItem,
  TeamSignalSummary,
} from './team-intelligence-types';

export type {
  DecisionDetail,
  DecisionVote,
  MemberDetection,
  NotificationSummary,
  OverlapZone,
  SharedSource,
  TeamBlindSpot,
  TeamDecision,
  TeamNotification,
  TeamProfile,
  TeamSignalItem,
  TeamSignalSummary,
  TeamTechEntry,
  UniqueStrength,
} from './team-intelligence-types';

export interface TeamIntelligenceSlice {
  // Team Signals (from monitoring)
  teamSignals: TeamSignalItem[];
  teamSignalsLoading: boolean;

  // Team Decisions
  teamDecisions: TeamDecision[];
  selectedDecision: DecisionDetail | null;
  decisionsLoading: boolean;

  // Team Notifications
  notifications: TeamNotification[];
  notificationSummary: NotificationSummary | null;
  notificationsLoading: boolean;

  // Shared Sources
  sharedSources: SharedSource[];
  sharedSourcesLoading: boolean;

  // Actions: Signals
  loadTeamSignals: (includeResolved?: boolean) => Promise<void>;
  resolveTeamSignal: (signalId: string, notes: string) => Promise<void>;

  // Actions: Decisions
  loadTeamDecisions: (statusFilter?: string) => Promise<void>;
  loadDecisionDetail: (decisionId: string) => Promise<void>;
  voteOnDecision: (decisionId: string, stance: string, rationale: string) => Promise<void>;
  resolveDecision: (decisionId: string, newStatus: string) => Promise<void>;

  // Actions: Notifications
  loadNotifications: (unreadOnly?: boolean) => Promise<void>;
  loadNotificationSummary: () => Promise<void>;
  markNotificationRead: (notificationId: string) => Promise<void>;
  markAllNotificationsRead: () => Promise<void>;
  dismissNotification: (notificationId: string) => Promise<void>;

  // Actions: Shared Sources
  loadSharedSources: () => Promise<void>;
  shareSource: (sourceType: string, configSummary: string, recommendation: string) => Promise<void>;
  upvoteSource: (sourceId: string) => Promise<void>;
  removeSharedSource: (sourceId: string) => Promise<void>;

  // Team Intelligence Profile
  teamProfile: TeamProfile | null;
  teamProfileLoading: boolean;

  // Team Signal Summary
  teamSignalSummary: TeamSignalSummary[];
  teamSignalSummaryLoading: boolean;

  // Actions: Intelligence
  loadTeamProfile: () => Promise<void>;
  loadTeamSignalSummary: () => Promise<void>;
  refreshBlindSpots: () => Promise<void>;
  refreshBusFactorReport: () => Promise<void>;
}

export const createTeamIntelligenceSlice: StateCreator<AppStore, [], [], TeamIntelligenceSlice> = (set, _get) => ({
  teamSignals: [],
  teamSignalsLoading: false,
  teamDecisions: [],
  selectedDecision: null,
  decisionsLoading: false,
  notifications: [],
  notificationSummary: null,
  notificationsLoading: false,
  sharedSources: [],
  sharedSourcesLoading: false,
  teamProfile: null,
  teamProfileLoading: false,
  teamSignalSummary: [],
  teamSignalSummaryLoading: false,

  loadTeamSignals: async (includeResolved = false) => {
    set({ teamSignalsLoading: true });
    try {
      const signals = await cmd('get_team_signals_cmd', { includeResolved });
      set({ teamSignals: signals, teamSignalsLoading: false });
    } catch {
      set({ teamSignalsLoading: false });
    }
  },

  resolveTeamSignal: async (signalId: string, notes: string) => {
    try {
      await cmd('resolve_team_signal_cmd', { signalId, notes });
      set(state => ({
        teamSignals: state.teamSignals.map(s =>
          s.id === signalId ? { ...s, resolved: true } : s,
        ),
      }));
    } catch { /* silent */ }
  },

  loadTeamDecisions: async (statusFilter?: string) => {
    set({ decisionsLoading: true });
    try {
      const decisions = await cmd('get_team_decisions', { statusFilter: statusFilter ?? null });
      set({ teamDecisions: decisions, decisionsLoading: false });
    } catch {
      set({ decisionsLoading: false });
    }
  },

  loadDecisionDetail: async (decisionId: string) => {
    try {
      const detail = await cmd('get_decision_detail', { decisionId });
      set({ selectedDecision: detail });
    } catch { /* silent */ }
  },

  voteOnDecision: async (decisionId: string, stance: string, rationale: string) => {
    try {
      await cmd('vote_on_decision', { decisionId, stance, rationale });
      const detail = await cmd('get_decision_detail', { decisionId });
      set({ selectedDecision: detail });
    } catch { /* silent */ }
  },

  resolveDecision: async (decisionId: string, newStatus: string) => {
    try {
      await cmd('resolve_decision', { decisionId, newStatus });
      set(state => ({
        teamDecisions: state.teamDecisions.map(d =>
          d.id === decisionId ? { ...d, status: newStatus } : d,
        ),
        selectedDecision: state.selectedDecision?.id === decisionId
          ? { ...state.selectedDecision, status: newStatus }
          : state.selectedDecision,
      }));
    } catch { /* silent */ }
  },

  loadNotifications: async (unreadOnly = false) => {
    set({ notificationsLoading: true });
    try {
      const notifications = await cmd('get_team_notifications', { limit: 50, unreadOnly });
      set({ notifications, notificationsLoading: false });
    } catch {
      set({ notificationsLoading: false });
    }
  },

  loadNotificationSummary: async () => {
    try {
      const summary = await cmd('get_notification_summary');
      set({ notificationSummary: summary });
    } catch { /* silent */ }
  },

  markNotificationRead: async (notificationId: string) => {
    try {
      await cmd('mark_notification_read', { notificationId });
      set(state => ({
        notifications: state.notifications.map(n =>
          n.id === notificationId ? { ...n, read: true } : n,
        ),
        notificationSummary: state.notificationSummary
          ? { ...state.notificationSummary, total_unread: Math.max(0, state.notificationSummary.total_unread - 1) }
          : null,
      }));
    } catch { /* silent */ }
  },

  markAllNotificationsRead: async () => {
    try {
      await cmd('mark_all_notifications_read');
      set(state => ({
        notifications: state.notifications.map(n => ({ ...n, read: true })),
        notificationSummary: state.notificationSummary
          ? { ...state.notificationSummary, total_unread: 0 }
          : null,
      }));
    } catch { /* silent */ }
  },

  dismissNotification: async (notificationId: string) => {
    try {
      await cmd('dismiss_notification', { notificationId });
      set(state => ({
        notifications: state.notifications.filter(n => n.id !== notificationId),
      }));
    } catch { /* silent */ }
  },

  loadSharedSources: async () => {
    set({ sharedSourcesLoading: true });
    try {
      const sources = await cmd('get_team_sources');
      set({ sharedSources: sources, sharedSourcesLoading: false });
    } catch {
      set({ sharedSourcesLoading: false });
    }
  },

  shareSource: async (sourceType: string, configSummary: string, recommendation: string) => {
    try {
      await cmd('share_source_with_team', { sourceType, configSummary, recommendation });
      const sources = await cmd('get_team_sources');
      set({ sharedSources: sources });
    } catch { /* silent */ }
  },

  upvoteSource: async (sourceId: string) => {
    try {
      await cmd('upvote_team_source', { sourceId });
      set(state => ({
        sharedSources: state.sharedSources.map(s =>
          s.id === sourceId ? { ...s, upvotes: s.upvotes + 1 } : s,
        ),
      }));
    } catch { /* silent */ }
  },

  removeSharedSource: async (sourceId: string) => {
    try {
      await cmd('remove_team_source', { sourceId });
      set(state => ({
        sharedSources: state.sharedSources.filter(s => s.id !== sourceId),
      }));
    } catch { /* silent */ }
  },

  loadTeamProfile: async () => {
    set({ teamProfileLoading: true });
    try {
      const profile = await cmd('get_team_profile_cmd');
      set({ teamProfile: profile, teamProfileLoading: false });
    } catch {
      set({ teamProfileLoading: false });
    }
  },

  loadTeamSignalSummary: async () => {
    set({ teamSignalSummaryLoading: true });
    try {
      const summary = await cmd('get_team_signal_summary_cmd');
      set({ teamSignalSummary: summary, teamSignalSummaryLoading: false });
    } catch {
      set({ teamSignalSummaryLoading: false });
    }
  },

  refreshBlindSpots: async () => {
    try {
      const blindSpots = await cmd('get_team_blind_spots_cmd');
      set(state => ({
        teamProfile: state.teamProfile
          ? { ...state.teamProfile, blind_spots: blindSpots }
          : null,
      }));
    } catch { /* silent */ }
  },

  refreshBusFactorReport: async () => {
    try {
      const strengths = await cmd('get_bus_factor_report_cmd');
      set(state => ({
        teamProfile: state.teamProfile
          ? { ...state.teamProfile, unique_strengths: strengths }
          : null,
      }));
    } catch { /* silent */ }
  },
});
