// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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
