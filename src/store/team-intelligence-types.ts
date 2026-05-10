// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// -- Team Intelligence Profile Types --

export interface TeamProfile {
  team_id: string;
  member_count: number;
  collective_stack: TeamTechEntry[];
  stack_coverage: number;
  blind_spots: TeamBlindSpot[];
  overlap_zones: OverlapZone[];
  unique_strengths: UniqueStrength[];
  generated_at: string;
}

export interface TeamTechEntry { tech: string; members: string[]; team_confidence: number; }
export interface TeamBlindSpot { topic: string; related_to: string[]; severity: string; }
export interface OverlapZone { topic: string; members: string[]; member_count: number; }
export interface UniqueStrength { tech: string; sole_expert: string; risk_level: string; }

export interface TeamSignalSummary {
  signal_id: string;
  chain_name: string;
  priority: string;
  tech_topics: string[];
  detected_by: MemberDetection[];
  team_confidence: number;
  first_detected_at: string;
  suggested_action: string;
  resolved: boolean;
}

export interface MemberDetection { client_id: string; display_name: string; detected_at: string; }

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

// -- Team Signal Item --

export interface TeamSignalItem {
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
