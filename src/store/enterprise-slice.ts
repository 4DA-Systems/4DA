// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore } from './types';

// -- Enterprise: Audit Types --

export interface AuditEntry {
  id: number;
  event_id: string;
  team_id: string;
  actor_id: string;
  actor_display_name: string;
  action: string;
  resource_type: string;
  resource_id: string | null;
  details: Record<string, unknown> | null;
  created_at: string;
}

export interface AuditSummary {
  total_events: number;
  events_by_action: [string, number][];
  events_by_actor: [string, number][];
  events_by_day: [string, number][];
}

// -- Enterprise: Webhook Types --

export interface Webhook {
  id: string;
  team_id: string;
  name: string;
  url: string;
  events: string[];
  active: boolean;
  failure_count: number;
  last_fired_at: string | null;
  last_status_code: number | null;
  created_at: string;
}

export interface WebhookDelivery {
  id: string;
  webhook_id: string;
  event_type: string;
  status: string;
  http_status: number | null;
  attempt_count: number;
  created_at: string;
  delivered_at: string | null;
}

// -- Enterprise: Organization Types --

export interface Organization {
  id: string;
  name: string;
  team_count: number;
  total_seats: number;
  created_at: string;
}

export interface OrgTeamSummary {
  team_id: string;
  member_count: number;
  last_active: string | null;
}

export interface RetentionPolicy {
  resource_type: string;
  retention_days: number;
}

export interface CrossTeamCorrelation {
  correlation_id: string;
  signal_type: string;
  teams_affected: [string, number][];
  org_severity: string;
  first_detected: string;
  recommendation: string;
}

// -- Enterprise: Analytics Types --

export interface TeamActivity {
  team_id: string;
  active_members: number;
  signals_this_period: number;
  decisions_this_period: number;
  engagement_score: number;
}

export interface OrgAnalytics {
  period: string;
  active_seats: number;
  total_seats: number;
  signals_detected: number;
  signals_resolved: number;
  decisions_tracked: number;
  briefings_generated: number;
  top_signal_categories: [string, number][];
  team_activity: TeamActivity[];
}

// -- Slice Interface --

export interface EnterpriseSlice {
  // Audit state
  auditEntries: AuditEntry[];
  auditSummary: AuditSummary | null;
  auditLoading: boolean;
  auditActionFilter: string;
  auditResourceFilter: string;

  // Webhook state
  webhooks: Webhook[];
  webhookDeliveries: Record<string, WebhookDelivery[]>;
  webhooksLoading: boolean;

  // Organization state
  organization: Organization | null;
  orgTeams: OrgTeamSummary[];
  retentionPolicies: RetentionPolicy[];
  crossTeamSignals: CrossTeamCorrelation[];
  orgAnalytics: OrgAnalytics | null;
  orgLoading: boolean;

  // Actions - Audit
  loadAuditLog: (actionFilter?: string, resourceFilter?: string, limit?: number, offset?: number) => Promise<void>;
  loadAuditSummary: (days?: number) => Promise<void>;
  exportAuditCsv: (from: string, to: string) => Promise<string>;
  setAuditActionFilter: (filter: string) => void;
  setAuditResourceFilter: (filter: string) => void;

  // Actions - Webhooks
  loadWebhooks: () => Promise<void>;
  registerWebhook: (name: string, url: string, events: string[]) => Promise<{ ok: boolean; error?: string }>;
  deleteWebhook: (webhookId: string) => Promise<void>;
  testWebhook: (webhookId: string) => Promise<boolean>;
  loadWebhookDeliveries: (webhookId: string, limit?: number) => Promise<void>;

  // Actions - Organization
  loadOrganization: () => Promise<void>;
  loadOrgTeams: () => Promise<void>;
  loadRetentionPolicies: () => Promise<void>;
  setRetentionPolicy: (resourceType: string, retentionDays: number) => Promise<void>;
  loadCrossTeamSignals: () => Promise<void>;
  loadOrgAnalytics: (days?: number) => Promise<void>;
  exportOrgAnalytics: (days?: number) => Promise<string>;
}

// -- Slice Creator --

export const createEnterpriseSlice: StateCreator<AppStore, [], [], EnterpriseSlice> = (set, get) => ({
  // Audit state
  auditEntries: [],
  auditSummary: null,
  auditLoading: false,
  auditActionFilter: '',
  auditResourceFilter: '',

  // Webhook state
  webhooks: [],
  webhookDeliveries: {},
  webhooksLoading: false,

  // Organization state
  organization: null,
  orgTeams: [],
  retentionPolicies: [],
  crossTeamSignals: [],
  orgAnalytics: null,
  orgLoading: false,

  // ========================================================================
  // Audit Actions
  // ========================================================================

  loadAuditLog: async (actionFilter?: string, resourceFilter?: string, limit?: number, offset?: number) => {
    set({ auditLoading: true });
    try {
      const entries = await cmd('get_audit_log', {
        actionFilter: actionFilter || undefined,
        resourceTypeFilter: resourceFilter || undefined,
        limit,
        offset,
      }) as unknown as AuditEntry[];
      set({ auditEntries: entries, auditLoading: false });
    } catch (e) {
      console.error('Failed to load audit log:', e);
      set({ auditLoading: false });
    }
  },

  loadAuditSummary: async (days?: number) => {
    try {
      const summary = await cmd('get_audit_summary_cmd', { days }) as unknown as AuditSummary;
      set({ auditSummary: summary });
    } catch (e) {
      console.error('Failed to load audit summary:', e);
    }
  },

  exportAuditCsv: async (from: string, to: string) => {
    try {
      return await cmd('export_audit_csv_cmd', { from, to });
    } catch (e) {
      console.error('Failed to export audit CSV:', e);
      throw e;
    }
  },

  setAuditActionFilter: (filter: string) => {
    set({ auditActionFilter: filter });
    const { auditResourceFilter } = get();
    get().loadAuditLog(filter, auditResourceFilter);
  },

  setAuditResourceFilter: (filter: string) => {
    set({ auditResourceFilter: filter });
    const { auditActionFilter } = get();
    get().loadAuditLog(auditActionFilter, filter);
  },

  // ========================================================================
  // Webhook Actions
  // ========================================================================

  loadWebhooks: async () => {
    set({ webhooksLoading: true });
    try {
      const webhooks = await cmd('list_webhooks_cmd') as unknown as Webhook[];
      set({ webhooks, webhooksLoading: false });
    } catch (e) {
      console.error('Failed to load webhooks:', e);
      set({ webhooksLoading: false });
    }
  },

  registerWebhook: async (name: string, url: string, events: string[]) => {
    try {
      await cmd('register_webhook_cmd', { name, url, events });
      get().loadWebhooks();
      return { ok: true };
    } catch (e) {
      console.error('Failed to register webhook:', e);
      return { ok: false, error: String(e) };
    }
  },

  deleteWebhook: async (webhookId: string) => {
    try {
      await cmd('delete_webhook_cmd', { webhookId });
      // Optimistic removal from local state, then reload
      set((state) => ({
        webhooks: state.webhooks.filter((w) => w.id !== webhookId),
        webhookDeliveries: Object.fromEntries(
          Object.entries(state.webhookDeliveries).filter(([k]) => k !== webhookId),
        ),
      }));
      get().loadWebhooks();
    } catch (e) {
      console.error('Failed to delete webhook:', e);
    }
  },

  testWebhook: async (webhookId: string) => {
    try {
      return await cmd('test_webhook_cmd', { webhookId });
    } catch (e) {
      console.error('Failed to test webhook:', e);
      return false;
    }
  },

  loadWebhookDeliveries: async (webhookId: string, limit?: number) => {
    try {
      const deliveries = await cmd('get_webhook_deliveries_cmd', { webhookId, limit }) as unknown as WebhookDelivery[];
      set((state) => ({
        webhookDeliveries: { ...state.webhookDeliveries, [webhookId]: deliveries },
      }));
    } catch (e) {
      console.error('Failed to load webhook deliveries:', e);
    }
  },

  // ========================================================================
  // Organization Actions
  // ========================================================================

  loadOrganization: async () => {
    set({ orgLoading: true });
    try {
      const organization = await cmd('get_organization_cmd') as unknown as Organization | null;
      set({ organization, orgLoading: false });
    } catch (e) {
      console.error('Failed to load organization:', e);
      set({ orgLoading: false });
    }
  },

  loadOrgTeams: async () => {
    try {
      const orgTeams = await cmd('get_org_teams_cmd') as unknown as OrgTeamSummary[];
      set({ orgTeams });
    } catch (e) {
      console.error('Failed to load org teams:', e);
    }
  },

  loadRetentionPolicies: async () => {
    try {
      const retentionPolicies = await cmd('get_retention_policies_cmd') as unknown as RetentionPolicy[];
      set({ retentionPolicies });
    } catch (e) {
      console.error('Failed to load retention policies:', e);
    }
  },

  setRetentionPolicy: async (resourceType: string, retentionDays: number) => {
    try {
      await cmd('set_retention_policy_cmd', { resourceType, days: retentionDays });
      get().loadRetentionPolicies();
    } catch (e) {
      console.error('Failed to set retention policy:', e);
      throw e;
    }
  },

  loadCrossTeamSignals: async () => {
    try {
      const crossTeamSignals = await cmd('get_cross_team_signals_cmd') as unknown as CrossTeamCorrelation[];
      set({ crossTeamSignals });
    } catch (e) {
      console.error('Failed to load cross-team signals:', e);
    }
  },

  loadOrgAnalytics: async (days?: number) => {
    set({ orgLoading: true });
    try {
      const orgAnalytics = await cmd('get_org_analytics_cmd', { days }) as unknown as OrgAnalytics;
      set({ orgAnalytics, orgLoading: false });
    } catch (e) {
      console.error('Failed to load org analytics:', e);
      set({ orgLoading: false });
    }
  },

  exportOrgAnalytics: async (days?: number) => {
    try {
      return await cmd('export_org_analytics_cmd', { days });
    } catch (e) {
      console.error('Failed to export org analytics:', e);
      throw e;
    }
  },
});
