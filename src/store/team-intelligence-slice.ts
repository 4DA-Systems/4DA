import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore } from './types';

// -- Team Decision Types --

export interface TeamDecision {
  id: string;
  team_id: string;
  title: string;
  decision_type: string;
  rationale: string;
  proposed_by: string;
  status: string;
  vote_count: number;
  created_at: string;
  resolved_at: string | null;
}

export interface DecisionVote {
  voter_id: string;
  stance: string;
  rationale: string;
  voted_at: string;
}

export interface DecisionDetail {
  id: string;
  team_id: string;
  title: string;
  decision_type: string;
  rationale: string;
  proposed_by: string;
  status: string;
  vote_count: number;
  votes: DecisionVote[];
  created_at: string;
  resolved_at: string | null;
}

// -- Team Notification Types --

export interface TeamNotification {
  id: string;
  team_id: string;
  notification_type: string;
  title: string;
  body: string | null;
  severity: string;
  read: boolean;
  created_at: string;
  metadata: Record<string, unknown> | null;
}

export interface NotificationSummary {
  total_unread: number;
  by_type: { notification_type: string; count: number }[];
}

// -- Shared Source Types --

export interface SharedSource {
  id: string;
  team_id: string;
  source_type: string;
  config_summary: Record<string, unknown>;
  recommendation: string;
  shared_by: string;
  upvotes: number;
  created_at: string;
}

// -- Slice Interface --

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
}

// Mirror the TeamSignal type from commands.ts
interface TeamSignalItem {
  id: string;
  team_id: string;
  signal_type: string;
  title: string;
  severity: string;
  tech_topics: string[];
  detected_by_count: number;
  first_detected: string;
  last_detected: string;
  resolved: boolean;
  resolved_by: string | null;
  resolved_at: string | null;
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

  // ---- Signals ----
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
      // Refresh signals after resolving
      set(state => ({
        teamSignals: state.teamSignals.map(s =>
          s.id === signalId ? { ...s, resolved: true } : s,
        ),
      }));
    } catch { /* silent */ }
  },

  // ---- Decisions ----
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
      // Refresh the decision detail if viewing it
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

  // ---- Notifications ----
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

  // ---- Shared Sources ----
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
      // Refresh the list
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
});
