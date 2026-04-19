// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { TeamSyncStatus, TeamMember } from '../lib/commands';
import type { AppStore } from './types';

export interface TeamSlice {
  // State
  teamStatus: TeamSyncStatus | null;
  teamMembers: TeamMember[];
  teamLoading: boolean;
  teamError: string | null;
  inviteCode: string | null;
  inviteExpiry: string | null;
  showTeamInviteDialog: boolean;

  // Actions
  loadTeamStatus: () => Promise<void>;
  loadTeamMembers: () => Promise<void>;
  createTeam: (relayUrl: string, displayName: string) => Promise<{ ok: boolean; error?: string }>;
  joinTeam: (relayUrl: string, inviteCode: string, displayName: string) => Promise<{ ok: boolean; error?: string }>;
  createInvite: (role?: string) => Promise<{ ok: boolean; code?: string; expiresAt?: string; error?: string }>;
  shareDna: (primaryStack: string[], interests: string[], blindSpots: string[], identitySummary: string) => Promise<void>;
  shareSignal: (signalId: string, chainName: string, priority: string, techTopics: string[], suggestedAction: string) => Promise<void>;
  proposeDecision: (decisionId: string, title: string, decisionType: string, rationale: string) => Promise<void>;
  setShowTeamInviteDialog: (show: boolean) => void;
}

export const createTeamSlice: StateCreator<AppStore, [], [], TeamSlice> = (set, get) => ({
  teamStatus: null,
  teamMembers: [],
  teamLoading: false,
  teamError: null,
  inviteCode: null,
  inviteExpiry: null,
  showTeamInviteDialog: false,

  loadTeamStatus: async () => {
    try {
      const status = await cmd('get_team_sync_status');
      set({ teamStatus: status, teamError: null });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamError: msg });
    }
  },

  loadTeamMembers: async () => {
    set({ teamLoading: true });
    try {
      const members = await cmd('get_team_members');
      set({ teamMembers: members, teamLoading: false, teamError: null });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamMembers: [], teamLoading: false, teamError: msg });
    }
  },

  createTeam: async (relayUrl: string, displayName: string) => {
    set({ teamLoading: true, teamError: null });
    try {
      const result = await cmd('create_team', { relayUrl, displayName });
      set({
        teamStatus: {
          enabled: true,
          connected: true,
          team_id: result.team_id,
          client_id: result.client_id,
          display_name: displayName,
          role: result.role,
          member_count: 1,
          pending_outbound: 0,
          last_sync_at: null,
          last_relay_seq: 0,
        },
        teamLoading: false,
      });
      return { ok: true };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamLoading: false, teamError: msg });
      return { ok: false, error: msg };
    }
  },

  joinTeam: async (relayUrl: string, inviteCode: string, displayName: string) => {
    set({ teamLoading: true, teamError: null });
    try {
      const result = await cmd('join_team_via_invite', { relayUrl, inviteCode, displayName });
      set({
        teamStatus: {
          enabled: true,
          connected: !result.awaiting_team_key,
          team_id: result.team_id,
          client_id: result.client_id,
          display_name: displayName,
          role: result.role,
          member_count: 0,
          pending_outbound: 0,
          last_sync_at: null,
          last_relay_seq: 0,
        },
        teamLoading: false,
      });
      // Refresh full status and members after joining
      get().loadTeamStatus();
      get().loadTeamMembers();
      return { ok: true };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamLoading: false, teamError: msg });
      return { ok: false, error: msg };
    }
  },

  createInvite: async (role?: string) => {
    try {
      const result = await cmd('create_team_invite', { role });
      set({ inviteCode: result.code, inviteExpiry: result.expires_at });
      return { ok: true, code: result.code, expiresAt: result.expires_at };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamError: msg });
      return { ok: false, error: msg };
    }
  },

  shareDna: async (primaryStack: string[], interests: string[], blindSpots: string[], identitySummary: string) => {
    try {
      await cmd('share_dna_with_team', { primaryStack, interests, blindSpots, identitySummary });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamError: msg });
    }
  },

  shareSignal: async (signalId: string, chainName: string, priority: string, techTopics: string[], suggestedAction: string) => {
    try {
      await cmd('share_signal_with_team', { signalId, chainName, priority, techTopics, suggestedAction });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamError: msg });
    }
  },

  proposeDecision: async (decisionId: string, title: string, decisionType: string, rationale: string) => {
    try {
      await cmd('propose_team_decision', { decisionId, title, decisionType, rationale });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ teamError: msg });
    }
  },

  setShowTeamInviteDialog: (show: boolean) => {
    set({ showTeamInviteDialog: show });
    // Clear stale invite data when closing
    if (!show) {
      set({ inviteCode: null, inviteExpiry: null });
    }
  },
});
