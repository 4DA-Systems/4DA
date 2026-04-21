// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Analysis-related types

export interface RelevanceMatch {
  source_file: string;
  matched_text: string;
  similarity: number;
}

export interface SourceRelevance {
  id: number;
  title: string;
  url: string | null;
  top_score: number;
  matches: RelevanceMatch[];
  relevant: boolean;
  explanation?: string;
  source_type?: string;
  confidence?: number;
  score_breakdown?: ScoreBreakdown;
  signal_type?: string;
  signal_priority?: string;
  signal_action?: string;
  signal_triggers?: string[];
  seen_on?: string[];
  similar_count?: number;
  similar_titles?: string[];
  serendipity?: boolean;
  /** STREETS revenue engine match (e.g. "Engine 1: Digital Products") */
  streets_engine?: string;
  /** Subject of the decision window that matched this item */
  decision_window_match?: string;
  /** How much score boost was applied from the decision window */
  decision_boost_applied?: number;
  /** When this item was first seen (ISO timestamp) */
  created_at?: string;
  /** Whether this item should display in critical alert banner (verified dependency match) */
  is_critical_alert?: boolean;
  /** Applicability assessment: affected | likely_affected | needs_verification | not_affected */
  applicability?: string;
  /** Advisory ID (e.g. "GHSA-xxxx-yyyy-zzzz" or "CVE-2025-1234") */
  advisory_id?: string;
}

export interface ScoreBreakdown {
  context_score: number;
  interest_score: number;
  keyword_score?: number;
  ace_boost: number;
  affinity_mult: number;
  anti_penalty: number;
  freshness_mult?: number;
  feedback_boost?: number;
  source_quality_boost?: number;
  confidence_by_signal: Record<string, number>;
  signal_count?: number;
  confirmed_signals?: string[];
  confirmation_mult?: number;
  /** Dependency match score (0.0-1.0): how strongly content matches installed packages */
  dep_match_score?: number;
  /** Package names from user's dependency graph that matched this content */
  matched_deps?: string[];
  /** Domain relevance (0.15 off-domain to 1.0 primary stack match) */
  domain_relevance?: number;
  /** Content quality multiplier (0.5 clickbait to 1.2 authoritative) */
  content_quality_mult?: number;
  /** Novelty multiplier (0.6 introductory to 1.15 release) */
  novelty_mult?: number;
  /** Intent boost from recent work topics (0.0 to 0.25) */
  intent_boost?: number;
  /** Content type classification (e.g. "security_advisory", "show_and_tell") */
  content_type?: string;
  /** Content DNA utility multiplier (0.3 hiring to 1.3 security) */
  content_dna_mult?: number;
  /** Competing tech penalty multiplier (0.5 or 1.0) */
  competing_mult?: number;
  /** LLM relevance score (1-5 scale) */
  llm_score?: number;
  /** LLM's one-sentence explanation */
  llm_reason?: string;
  /** Stack intelligence: pain point and keyword boost (0.0-0.20) */
  stack_boost?: number;
  /** Stack intelligence: ecosystem shift multiplier (0.95-1.25, default 1.0) */
  ecosystem_shift_mult?: number;
  /** Stack intelligence: competing tech suppression (0.95 or 1.0) */
  stack_competing_mult?: number;
  /** Decision window boost applied (0.0-0.20) */
  window_boost?: number;
  /** ID of matched decision window */
  matched_window_id?: number;
  /** Skill gap boost from sovereign profile intelligence (0.0-0.20) */
  skill_gap_boost?: number;
  /** Necessity score: "what you'd regret missing" (0.0-1.0) */
  necessity_score?: number;
  /** One-line explanation of why this item is necessary */
  necessity_reason?: string;
  /** Necessity category (security_vulnerability, breaking_change, deprecation_notice, blind_spot, decision_relevant, none) */
  necessity_category?: string;
  /** Necessity urgency (immediate, this_week, awareness, none) */
  necessity_urgency?: string;
  /** Signal strength bonus: ceiling adjustment for strong confirmed signals (0.0-0.08) */
  signal_strength_bonus?: number;
  /** Content analysis multiplier from cached LLM pre-analysis (0.55-1.15, default 1.0) */
  content_analysis_mult?: number;
  /** Intelligence Mesh Phase 3: one entry per advisor that evaluated this item.
   *  Rust: src-tauri/src/types.rs#AdvisorSignal → ts-rs bindings/AdvisorSignal.ts */
  advisor_signals?: AdvisorSignal[];
  /** Intelligence Mesh Phase 2: set when pipeline and advisor(s) disagreed.
   *  The pipeline score is always authoritative — this flag is informative. */
  disagreement?: DisagreementKind | null;
  /** Advisory source (GHSA, RustSec, npm_advisory, OSV) */
  advisory_source?: string;
  /** CVSS score (0.0-10.0) */
  cvss_score?: number;
  /** CVSS severity (critical, high, medium, low) */
  cvss_severity?: string;
  /** Affected version range from advisory (e.g. "< 3.0.0") */
  affected_versions?: string;
  /** Fixed version (e.g. "3.0.1") */
  fixed_version?: string;
  /** User's installed version from lockfile */
  installed_version?: string;
  /** Whether installed version is in the affected range */
  is_version_affected?: boolean;
  /** Dependency path (direct | transitive | dev-only) */
  dependency_path?: string;
  /** Number of user's projects affected */
  affected_project_count?: number;
}

/** Why pipeline and advisor(s) disagreed. Always a UI signal, never a score override. */
export type DisagreementKind =
  | 'AdvisorSkeptical'
  | 'AdvisorEnthusiastic'
  | 'AdvisorsInternal';

/** One advisor's opinion on an item, stamped with provenance.
 *  Mirrors src-tauri/bindings/bindings/AdvisorSignal.ts. */
export interface AdvisorSignal {
  provider: string;
  model: string;
  /** SHA-256 identity hash — used to look up the matching calibration
   *  curve via get_calibration_curve_status. Optional for backward
   *  compat with signals stamped before Phase 7c. */
  identity_hash?: string | null;
  task: string;
  raw_score: number;
  normalized_score: number;
  confidence: number;
  reason: string | null;
  prompt_version: string | null;
  calibration_id: string | null;
}

export interface AnalysisProgress {
  stage: string;
  progress: number;
  message: string;
  items_processed: number;
  items_total: number;
}

export interface ProValueReport {
  period_days: number;
  briefings_generated: number;
  signals_detected: number;
  knowledge_gaps_caught: number;
  predictions_made: number;
  queries_run: number;
  items_surfaced: number;
  attention_insights: number;
  estimated_hours_saved: number;
  data_age_days: number;
  total_feedback_events: number;
  active_since: string | null;
}
