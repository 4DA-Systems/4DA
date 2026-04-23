// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Type-safe Tauri IPC command layer.
 *
 * Every invoke() call in the app should use `commands.commandName(params)`
 * instead of raw `invoke('command_name', { ... })`.
 *
 * Benefits:
 * - Compile-time checking of command names, parameter types, and return types
 * - IDE autocomplete for all 211 commands
 * - Single source of truth for the IPC contract
 *
 * Generated from Rust #[tauri::command] signatures + frontend usage analysis.
 */

import { invoke } from '@tauri-apps/api/core';

import type {
  SourceRelevance,
  ScoreBreakdown,
} from '../types/analysis';
import type { CalibrationResult, CurveFitReport, CurveStatus, TasteTestStepResult, TasteProfileSummary } from '../types/calibration';
import type {
  Settings,
  MonitoringStatus,
  UserContext,
  Anomaly,
} from '../types/settings';
import type {
  ContextFile,
  VoidSignal,
  SuggestedInterest,
  IndexedStats,
} from '../types/common';
import type {
  IndexedDocumentsResponse,
  DocumentContentResponse,
  DocumentSearchResult,
  SourceHealthStatus,
  SourceQualityReport,
  SavedItem,
  ItemContent,
  ItemSummary,
} from '../types/sources';
import type {
  SignalChain,
  ProjectHealth,
  AttentionReport,
  DeveloperDna,
} from '../types/innovation';
import type {
  AutophagyCycleResult,
  AutophagyStatus,
  DataHealth,
  MaintenanceResult,
  DecisionWindow,
} from '../types/autophagy';
import type {
  PlaybookModule,
  PlaybookContent,
  PlaybookProgress,
} from '../types/playbook';
import type { ParsedCommand, CommandExecutionResult } from '../types/streets';
import type { StackProfileSummary, StackDetection } from '../types/stacks';
import type {
  InfrastructureDimension,
  StackDimension,
  SkillsDimension,
  PreferencesDimension,
  ContextDimension,
  IntelligenceReport,
  CompletenessReport,
} from '../types';
import type { PersonalizedLesson as PersonalizedLessonType } from '../types/personalization';
import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';

// ============================================================================
// Preemption & Intelligence Types — Intelligence Reconciliation Phase 3
// ============================================================================
//
// 2026-04-17: `get_preemption_alerts` now returns the canonical `EvidenceFeed`
// of `EvidenceItem`s (imported from the Rust bindings). The legacy local
// declarations of `PreemptionAlert`, `PreemptionFeed`, `AlertEvidence`,
// `SuggestedAction`, `PreemptionType`, and `AlertUrgency` were deleted;
// Rust-side `PreemptionAlert` is still used by `monitoring_briefing.rs` and
// its own ts-rs binding file remains for that path. Doctrine rule 1: one
// canonical type per concept.

// BlindSpotReport + 4-shape decomposition (UncoveredDep, StaleTopic,
// MissedSignal, BlindSpotRecommendation) removed from the TS layer
// 2026-04-17 per Intelligence Reconciliation Phase 4. `get_blind_spots`
// now returns `EvidenceFeed` of `EvidenceItem`s with the coverage index
// riding on `feed.score`. Rust-side struct is retained internally for
// `monitoring_briefing.rs` but no longer crosses the IPC boundary.

interface TrustSummary {
  period_days: number;
  total_surfaced: number;
  acted_on: number;
  dismissed: number;
  false_positives: number;
  precision: number;
  action_conversion_rate: number;
  preemption_wins: number;
  avg_lead_time_hours: number | null;
  trend: string;
}

interface DomainPrecision {
  domain: string;
  precision: number;
  total_surfaced: number;
  acted_on: number;
  false_positives: number;
}

interface FalsePositiveAnalysis {
  total_fp: number;
  by_source: Array<{ source_type: string; total: number; fp_count: number; fp_rate: number }>;
  by_topic: Array<{ topic: string; total: number; fp_count: number; fp_rate: number }>;
  recommendations: string[];
}

// ============================================================================
// Command Map — maps every command name to { params, result }
// ============================================================================

/**
 * Sovereign Cold Boot — pre-baked briefing snapshot returned by
 * `get_briefing_snapshot`. Loaded by `main.tsx` BEFORE React mounts so the
 * first paint after a cold boot is yesterday's briefing instead of a black
 * window. Mirrors the Rust `BriefingSnapshot` shape in `briefing_snapshot.rs`.
 */
interface BriefingSnapshotItem {
  title: string;
  source_type: string;
  score: number;
  signal_type?: string | null;
  url?: string | null;
  item_id?: number | null;
  signal_priority?: string | null;
  description?: string | null;
  matched_deps?: string[];
}

interface BriefingSnapshotPayload {
  title: string;
  items: BriefingSnapshotItem[];
  total_relevant: number;
  synthesis?: string | null;
  wisdom_synthesis?: string | null;
}

interface BriefingSnapshotResult {
  /** Format version. v1 at present; older versions are silently ignored. */
  version: number;
  /** Unix timestamp when this snapshot was generated. */
  generated_at_unix: number;
  /** Pre-formatted display string, e.g. "Mon Apr 7, 9:14 AM". */
  generated_at_display: string;
  /** The actual briefing payload — same shape as the live morning brief. */
  briefing: BriefingSnapshotPayload;
}

/** Full IPC contract: command name → parameter type & return type. */
interface CommandMap {
  // -- Analysis & Core --
  get_analysis_status: { params: Record<string, never>; result: { running: boolean; completed: boolean; error: string | null; results: SourceRelevance[] | null; started_at: number | null; last_completed_at: string | null; near_misses: SourceRelevance[] | null } };
  run_cached_analysis: { params: Record<string, never>; result: void };
  run_deep_initial_scan: { params: Record<string, never>; result: void };
  cancel_analysis: { params: Record<string, never>; result: void };
  get_scoring_stats: { params: Record<string, never>; result: ScoringStats };
  get_context_files: { params: Record<string, never>; result: ContextFile[] };
  clear_context: { params: Record<string, never>; result: string };
  index_context: { params: Record<string, never>; result: string };
  index_project_readmes: { params: Record<string, never>; result: string };
  sync_awe_wisdom: { params: Record<string, never>; result: string };
  get_awe_summary: { params: Record<string, never>; result: string };
  run_awe_transmute: { params: { query: string; mode: string }; result: string };
  run_awe_quick_check: { params: { query: string }; result: string };
  run_awe_consequence_scan: { params: { query: string }; result: string };
  run_awe_feedback: { params: { decisionId: string; outcome: string; details: string }; result: string };
  run_awe_recall: { params: { domain: string }; result: string };
  run_awe_calibration: { params: { domain: string }; result: string };
  // AWE page-specific commands
  get_awe_pattern_match: { params: { query: string; domain: string }; result: string };
  get_awe_decision_history: { params: { domain: string; limit: number }; result: string };
  get_awe_pending_decisions: { params: { limit: number }; result: string };
  get_awe_wisdom_well: { params: { domain: string }; result: string };
  get_awe_growth_trajectory: { params: { domain: string }; result: string };
  submit_awe_batch_feedback: { params: { feedbacks: Array<{ decision_id: string; outcome: string; details: string }> }; result: string };
  run_awe_auto_feedback: { params: Record<string, never>; result: string };
  run_awe_autonomous_now: { params: Record<string, never>; result: string };
  run_awe_purge: { params: { dryRun: boolean }; result: string };
  get_awe_candidates: { params: { domain: string }; result: string };
  record_awe_interaction_feedback: { params: { itemTitle: string; interaction: string; sourceType: string }; result: string };
  // AWE Synthesis (behavioral data bridge)
  get_behavioral_context: { params: Record<string, never>; result: string };
  synthesize_wisdom: { params: Record<string, never>; result: string };
  synthesize_topic_context: { params: { topics: string[] }; result: string };
  refresh_awe_context: { params: Record<string, never>; result: string };
  export_results: { params: { format: string }; result: string };
  get_diagnostics: { params: Record<string, never>; result: DiagnosticsSnapshot };

  // -- Settings & Configuration --
  get_privacy_config: { params: Record<string, never>; result: { llm_content_level: string; proxy_url: string | null; cloud_llm_disclosure_accepted: boolean; crash_reporting_opt_in: boolean; activity_tracking_opt_in: boolean } };
  set_privacy_config: { params: { llm_content_level?: string; cloud_llm_disclosure_accepted?: boolean; crash_reporting_opt_in?: boolean; activity_tracking_opt_in?: boolean }; result: void };
  get_settings: { params: Record<string, never>; result: Settings };
  get_llm_usage: { params: Record<string, never>; result: { used: number; limit: number; limit_reached: boolean; unlimited: boolean; cost_used_cents: number; cost_limit_cents: number; cost_limit_reached: boolean } };
  set_llm_provider: { params: { provider: string; apiKey: string; model: string; baseUrl: string | null; openaiApiKey?: string | null }; result: void };
  set_rerank_config: { params: { enabled: boolean; maxItems: number; minScore: number; dailyTokenLimit: number; dailyCostLimit: number }; result: void };
  set_llm_limits: { params: { dailyTokenLimit: number; dailyCostLimitCents: number }; result: void };
  test_llm_connection: { params: Record<string, never>; result: { success: boolean; message: string } };
  check_ollama_status: { params: { baseUrl: string | null }; result: { operational: boolean; models: string[]; error: string | null } };
  mark_onboarding_complete: { params: Record<string, never>; result: void };
  pull_ollama_model: { params: { model: string; baseUrl: string | null }; result: void };
  cancel_ollama_pull: { params: Record<string, never>; result: string };
  list_provider_models: { params: { provider: string; baseUrl: string | null; apiKey: string | null }; result: { models: string[]; error?: string } };
  detect_local_servers: { params: Record<string, never>; result: { servers: Array<{ name: string; base_url: string; model_count: number; running: boolean }> } };
  get_llm_key_for_mcp: { params: Record<string, never>; result: { provider: string; api_key_masked: string; has_api_key: boolean; model: string; base_url: string | null } };
  detect_environment: { params: Record<string, never>; result: { has_anthropic_env: boolean; anthropic_env_preview: string; has_openai_env: boolean; openai_env_preview: string; ollama_running: boolean; ollama_url: string | null } };
  import_env_key: { params: { provider: string }; result: string };
  validate_api_key: { params: { provider: string; key: string; baseUrl?: string | null }; result: { valid: boolean; format_ok: boolean; connection_ok: boolean; error: string | null; model_access: string[] } };
  run_calibration: { params: Record<string, never>; result: CalibrationResult };
  // Intelligence Mesh Phase 5b.2 — per-model calibration curve fitter.
  // Scans persisted advisor signals, pairs with InteractionPattern +
  // feedback, fits an equal-width bucket curve per (model, task), saves
  // to disk. Skipped pairs surface a reason in the returned report.
  fit_calibration_curves_now: { params: Record<string, never>; result: CurveFitReport };
  // Look up the calibration curve status for a specific (model, task)
  // pair. Null means no curve on disk yet (advisor is pre-mesh).
  get_calibration_curve_status: { params: { identityHash: string; task: string; currentPromptVersion: string }; result: CurveStatus | null };
  set_close_to_tray: { params: { enabled: boolean }; result: void };
  set_launch_at_startup: { params: { enabled: boolean }; result: { launch_at_startup: boolean; registration_failed?: boolean; message: string } };
  get_launch_at_startup: { params: Record<string, never>; result: boolean };
  notification_clicked: { params: { item_id?: number | null }; result: void };
  briefing_item_clicked: { params: { item_id?: number | null }; result: void };
  briefing_open_url: { params: { url: string }; result: void };
  // Sovereign Cold Boot — instant first-paint of yesterday's briefing snapshot.
  // Returns null if no fresh (≤24h, version-matched) snapshot is on disk.
  get_briefing_snapshot: { params: Record<string, never>; result: BriefingSnapshotResult | null };
  set_morning_briefing_enabled: { params: { enabled: boolean }; result: { morning_briefing: boolean; message: string } };
  get_morning_briefing_config: { params: Record<string, never>; result: { enabled: boolean; time: string } };
  set_briefing_time: { params: { time: string }; result: { briefing_time: string; message: string } };
  trigger_briefing_preview: { params: Record<string, never>; result: { message: string } };
  trigger_morning_briefing: { params: Record<string, never>; result: string };
  record_interaction: { params: { sourceItemId: number; action: string }; result: { success: boolean } };
  snooze_item: { params: { sourceItemId: number; days: number }; result: { success: boolean; snooze_days: number } };
  watch_item: { params: { sourceItemId: number; topic: string; title: string }; result: { success: boolean; topic: string } };
  unwatch_item: { params: { sourceItemId: number }; result: { success: boolean } };
  get_watched_items: { params: Record<string, never>; result: { watched_items: Array<{ source_item_id: number; topic: string; title: string; created_at: string }> } };
  set_blind_spot_sensitivity: { params: { sensitivity: string }; result: { success: boolean; sensitivity: string } };
  get_blind_spot_sensitivity: { params: Record<string, never>; result: { sensitivity: string } };

  // -- Taste Test Calibration --
  taste_test_start: { params: Record<string, never>; result: TasteTestStepResult };
  taste_test_respond: { params: { itemSlot: number; response: string; responseTimeMs?: number }; result: TasteTestStepResult };
  taste_test_finalize: { params: Record<string, never>; result: TasteProfileSummary };
  taste_test_is_calibrated: { params: Record<string, never>; result: boolean };
  taste_test_get_profile: { params: Record<string, never>; result: TasteProfileSummary | null };

  // -- User Context & Interests --
  get_user_context: { params: Record<string, never>; result: UserContext };
  add_interest: { params: { topic: string }; result: void };
  remove_interest: { params: { topic: string }; result: void };
  add_exclusion: { params: { topic: string }; result: void };
  remove_exclusion: { params: { topic: string }; result: void };
  add_tech_stack: { params: { technology: string }; result: void };
  remove_tech_stack: { params: { technology: string }; result: void };
  set_user_role: { params: { role: string | null }; result: void };
  set_experience_level: { params: { level: string | null }; result: void };
  set_selected_stacks: { params: { profileIds: string[] }; result: void };
  get_selected_stacks: { params: Record<string, never>; result: string[] };
  get_stack_profiles: { params: Record<string, never>; result: StackProfileSummary[] };
  detect_stack_profiles: { params: Record<string, never>; result: StackDetection[] };
  get_composed_stack: { params: Record<string, never>; result: ComposedStackSummary };

  // -- ACE (Autonomous Context Engine) --
  ace_record_interaction: { params: { item_id: number; action_type: string; action_data: string | null; item_topics: string[]; item_source: string }; result: void };
  ace_record_accuracy_feedback: { params: { item_id: number; predicted_score: number; feedback_type: string }; result: void };
  record_item_feedback: { params: { item_id: number; relevant: boolean }; result: void };
  triage_alert: { params: { itemId: number; action: string; advisoryId: string | null; reason: string | null; expiresAt: string | null }; result: void };
  get_triage_states: { params: { itemIds: number[] }; result: Array<{ item_id: number; advisory_id: string | null; action: string; reason: string | null; resolved_at: string; expires_at: string | null }> };
  clear_expired_triage: { params: Record<string, never>; result: number };
  ace_get_topic_affinities: { params: Record<string, never>; result: { affinities: Array<{ topic: string; positive_signals: number; negative_signals: number; affinity_score: number }>; count: number } };
  ace_get_anti_topics: { params: { min_rejections: number }; result: { anti_topics: Array<{ topic: string; rejection_count: number; last_rejected: string }>; count: number } };
  ace_get_single_affinity: { params: { topic: string }; result: { affinity: { topic: string; positive_signals: number; negative_signals: number; affinity_score: number } | null } };
  ace_get_suggested_interests: { params: Record<string, never>; result: SuggestedInterest[] };
  ace_get_detected_tech: { params: Record<string, never>; result: { detected_tech: Array<{ name: string; category: string; confidence: number }> } };
  ace_get_active_topics: { params: Record<string, never>; result: { topics: Array<{ topic: string; weight: number }> } };
  ace_auto_discover: { params: Record<string, never>; result: { success: boolean; directories_found: number; projects_found: number; directories_added: number; directories: string[]; scan_result: { manifest_scan: { detected_tech: number; confidence: number }; git_scan: { repos_analyzed: number; total_commits: number }; combined: { total_topics: number; topics: string[] } } } };
  ace_full_scan: { params: { paths: string[] }; result: { success: boolean; manifest_scan: { detected_tech: number; confidence: number }; git_scan: { repos_analyzed: number; total_commits: number }; combined: { total_topics: number; topics: string[] } } };
  ace_get_unresolved_anomalies: { params: Record<string, never>; result: { anomalies: Anomaly[]; count: number } };
  ace_detect_anomalies: { params: Record<string, never>; result: { anomalies: Anomaly[]; count: number } };
  ace_resolve_anomaly: { params: { anomalyId: number }; result: void };
  ace_embedding_status: { params: Record<string, never>; result: { operational: boolean } };
  ace_get_rate_limit_status: { params: { source: string }; result: { global_remaining: number; source_remaining: number; is_limited: boolean } };
  ace_get_accuracy_metrics: { params: Record<string, never>; result: { precision: number; engagement_rate: number; calibration_error: number } };
  ace_find_similar_topics: { params: { query: string; topK: number }; result: { query: string; results: Array<{ topic: string; similarity: number }> } };
  ace_save_watcher_state: { params: Record<string, never>; result: void };
  ace_get_scan_summary: { params: Record<string, never>; result: ScanSummary };

  // -- Context Directories --
  get_context_dirs: { params: Record<string, never>; result: string[] };
  set_context_dirs: { params: { dirs: string[] }; result: void };

  // -- Monitoring --
  get_monitoring_status: { params: Record<string, never>; result: MonitoringStatus };
  set_monitoring_enabled: { params: { enabled: boolean }; result: void };
  set_monitoring_interval: { params: { minutes: number }; result: void };
  set_notification_threshold: { params: { threshold: string }; result: void };
  set_notification_style: { params: { style: string }; result: { notification_style: string } };
  trigger_notification_test: { params: Record<string, never>; result: void };
  trigger_notification_preview: { params: { priority: string }; result: { success: boolean; priority: string } };

  // -- Briefing --
  get_latest_briefing: { params: Record<string, never>; result: { content: string; model: string | null; item_count: number; created_at: string } | null };
  generate_ai_briefing: { params: Record<string, never>; result: { success: boolean; briefing: string | null; error?: string; model?: string; item_count?: number; latency_ms?: number } };
  generate_free_briefing: { params: Record<string, never>; result: { content: string; item_count: number; created_at: string } };
  generate_cli_briefing: { params: Record<string, never>; result: string };
  get_source_health_status: { params: Record<string, never>; result: SourceHealthStatus[] };
  get_source_quality: { params: Record<string, never>; result: SourceQualityReport[] };
  reset_source_circuit_breaker: { params: { source_type: string }; result: string };

  // -- Decisions --
  get_decisions: { params: { limit?: number; decisionType?: string; status?: string }; result: DeveloperDecision[] };
  record_developer_decision: { params: { decisionType: string; subject: string; decision: string; rationale: string | null; alternativesRejected: string[]; contextTags: string[]; confidence: number }; result: DeveloperDecision };
  update_developer_decision: { params: { id: number; decision: string | null; rationale: string | null; status: string | null; confidence: number | null }; result: DeveloperDecision };
  remove_tech_decision: { params: { technology: string }; result: void };

  // -- Agent Memory --
  store_agent_memory: { params: { sessionId: string; agentType: string; memoryType: string; subject: string; content: string; contextTags?: string[]; expiresAt?: string }; result: number };
  recall_agent_memories: { params: { subject: string; limit: number }; result: AgentMemoryEntry[] };
  generate_agent_brief: { params: { agentType?: string; since?: string }; result: AgentSessionBrief };
  get_delegation_score: { params: { subject: string }; result: DelegationScoreResult };
  get_all_delegation_scores: { params: Record<string, never>; result: DelegationScoreEntry[] };
  promote_memory_to_decision: { params: { memoryId: number }; result: number };

  // -- Decision Advantage --
  get_decision_windows: { params: Record<string, never>; result: DecisionWindow[] };
  act_on_decision_window: { params: { windowId: number; outcome: string | null }; result: string };
  close_decision_window: { params: { windowId: number; outcome: string | null }; result: string };
  get_advantage_history: { params: { period?: string; limit?: number }; result: number[] };

  // -- Autophagy --
  get_autophagy_status: { params: Record<string, never>; result: AutophagyStatus };
  get_autophagy_history: { params: { limit: number }; result: AutophagyCycleResult[] };
  trigger_autophagy_cycle: { params: Record<string, never>; result: AutophagyCycleResult };
  get_intelligence_pulse: { params: Record<string, never>; result: IntelligencePulseData };

  // -- Data Health --
  get_data_health: { params: Record<string, never>; result: DataHealth };
  run_deep_clean: { params: Record<string, never>; result: MaintenanceResult };
  set_cleanup_retention: { params: { days: number }; result: null };

  // -- Model Registry --
  get_model_registry: { params: Record<string, never>; result: { fetched_at: number; source: string; model_count: number; providers: Record<string, Array<{ id: string; provider: string; display_name: string; input_cost_per_token: number | null; output_cost_per_token: number | null; max_input_tokens: number | null; max_output_tokens: number | null }>> } };
  refresh_model_registry: { params: Record<string, never>; result: { success: boolean; model_count: number; source: string } };

  // -- License & Trial --
  get_license_tier: { params: Record<string, never>; result: { tier: string; has_key: boolean; activated_at: string | null; expires_at: string | null; days_remaining: number; expired: boolean } };
  activate_license: { params: { licenseKey: string }; result: { success: boolean; tier: string; expires_at?: string } };
  validate_license: { params: Record<string, never>; result: { validated: boolean; tier: string; cached?: boolean; detail: string } };
  recover_license_by_email: { params: { email: string }; result: { success: boolean; license_key?: string; tier?: string; expires_at?: string; status?: string; reason?: string; detail?: string } };
  get_trial_status: { params: Record<string, never>; result: { active: boolean; days_remaining: number; started_at: string | null } };
  start_trial: { params: Record<string, never>; result: { success: boolean; days_remaining?: number } };
  get_pro_value_report: { params: Record<string, never>; result: ProValueReport };

  // -- Templates --
  get_templates: { params: Record<string, never>; result: Array<{ id: string; title: string; description: string; category: string; content: string }> };
  get_template_content: { params: { templateId: string }; result: { id: string; title: string; description: string; category: string; content: string } };

  // -- Playbook (STREETS) --
  get_playbook_modules: { params: { lang?: string }; result: PlaybookModule[] };
  get_playbook_content: { params: { moduleId: string; lang?: string }; result: PlaybookContent };
  get_playbook_progress: { params: Record<string, never>; result: PlaybookProgress };
  mark_lesson_complete: { params: { moduleId: string; lessonIdx: number }; result: void };
  translate_playbook_module: { params: { moduleId: string; lang: string }; result: string };
  get_lesson_translation_status: { params: { lang: string }; result: Record<string, boolean> };
  parse_lesson_commands: { params: { moduleId: string; lessonIdx: number }; result: ParsedCommand[] };
  execute_streets_command: { params: { commandId: string; command: string; riskLevel: string }; result: CommandExecutionResult };
  execute_lesson_commands: { params: { moduleId: string; lessonIdx: number; maxRisk: string }; result: CommandExecutionResult[] };
  get_personalized_lesson: { params: { moduleId: string; lessonIdx: number }; result: PersonalizedLesson };
  get_personalized_lessons_batch: { params: { requests: Array<[string, number]> }; result: PersonalizedLesson[] };
  get_personalization_context_summary: { params: Record<string, never>; result: PersonalizationContextSummary };
  prune_personalization_cache: { params: Record<string, never>; result: { deleted: number; remaining: number; read_states: number; cache_size_bytes: number } };
  hydrate_lesson_with_llm: { params: { moduleId: string; lessonIdx: number }; result: { upgraded: number; total_blocks?: number; reason?: string } };

  // -- Content Integrity --
  check_content_integrity: { params: Record<string, never>; result: ContentIntegrityReport };
  audit_content_integrity: { params: Record<string, never>; result: ContentIntegrityReport };

  // -- First-Run Simulation Audit --
  run_first_run_simulation: { params: Record<string, never>; result: FirstRunAuditReport };

  // -- STREETS Health --
  get_street_health: { params: Record<string, never>; result: StreetHealthScore };
  get_streets_suggestion: { params: Record<string, never>; result: { module_id: string; module_title: string; reason: string; match_strength: number } | null };

  // -- Sovereign Profile --
  get_sovereign_profile: { params: Record<string, never>; result: SovereignProfileData };
  get_sovereign_profile_completeness: { params: Record<string, never>; result: ProfileCompleteness };
  save_sovereign_fact: { params: { category: string; key: string; value: string }; result: void };
  generate_sovereign_stack_document: { params: Record<string, never>; result: string };
  get_execution_log: { params: { moduleId: string; lessonIdx?: number }; result: ExecutionLogEntry[] };

  // -- Sovereign Developer Profile (Unified) --
  get_sovereign_developer_profile: { params: Record<string, never>; result: SovereignDeveloperProfileData };
  export_sovereign_profile_markdown: { params: Record<string, never>; result: string };
  export_sovereign_profile_json: { params: Record<string, never>; result: string };

  // -- Saved Items & Feedback --
  get_saved_items: { params: Record<string, never>; result: SavedItem[] };
  remove_saved_item: { params: { itemId: number }; result: void };

  // -- Item Content & Summary --
  get_item_content: { params: { itemId: number }; result: ItemContent };
  get_item_summary: { params: { itemId: number }; result: ItemSummary };
  generate_item_summary: { params: { itemId: number }; result: ItemSummary };

  // -- Natural Language Query --
  natural_language_query: { params: { queryText: string }; result: NLQResult };

  // -- Indexed Documents --
  get_indexed_documents: { params: { limit: number; offset: number; fileType: string | null }; result: IndexedDocumentsResponse };
  get_indexed_stats: { params: Record<string, never>; result: IndexedStats };
  search_documents: { params: { query: string; limit: number }; result: { results: DocumentSearchResult[] } };
  get_document_content: { params: { documentId: number }; result: DocumentContentResponse };

  // -- Knowledge Gaps --
  // Phase 5 (2026-04-17): returns canonical EvidenceFeed. Legacy KnowledgeGap
  // type is still imported by a handful of pre-Phase-5 consumers; items now
  // flow via EvidenceItem with kind=Gap.
  get_knowledge_gaps: { params: Record<string, never>; result: EvidenceFeed };

  // -- Git Decision Miner (Phase 7 — Cold Start Layer 1) --
  // Scans user's configured context_dirs for decision-shaped commits.
  // Returns a JSON string: { summary: MineSummary, jsonl_path: string }.
  mine_git_decisions: { params: Record<string, never>; result: string };

  // -- Seed Corpus (Phase 8 — Cold Start Layer 2) --
  // Returns CorpusStats as JSON { total, by_outcome, domains_covered }.
  get_seed_corpus_stats: { params: Record<string, never>; result: string };

  // -- Commitment Contracts (Phase 11) --
  create_commitment_contract: { params: { decisionStatement: string; refutationCondition: string; subject: string }; result: number };
  get_commitment_contracts: { params: Record<string, never>; result: string };
  dismiss_commitment_contract: { params: { contractId: number }; result: void };
  check_refutations: { params: { hours: number }; result: string };

  // -- Signal Chains --
  // Phase 5 (2026-04-17): get_signal_chains_predicted returns canonical
  // EvidenceFeed (kind=Chain). Raw get_signal_chains still returns the
  // legacy SignalChain[] for lower-level accessors not yet migrated.
  get_signal_chains: { params: Record<string, never>; result: SignalChain[] };
  get_signal_chains_predicted: { params: Record<string, never>; result: EvidenceFeed };
  resolve_signal_chain: { params: { chainId: string; resolution: string }; result: void };

  // -- Score Autopsy --
  mcp_score_autopsy: { params: { itemId: number; sourceType: string; synthesize: boolean; compact: boolean }; result: ScoreAutopsyResult };

  // -- Engagement & Attention --
  get_engagement_summary: { params: Record<string, never>; result: EngagementData };
  get_attention_report: { params: { periodDays: number }; result: AttentionReport };

  // -- Preemption & Blind Spots --
  // Phase 3 & 4 (2026-04-17): both return the canonical EvidenceFeed
  // envelope. Preemption's feed.score is None; Blind Spots' feed.score
  // carries the 0-100 coverage index.
  get_preemption_alerts: { params: Record<string, never>; result: EvidenceFeed };
  get_blind_spots: { params: Record<string, never>; result: EvidenceFeed };

  // -- Trust Ledger --
  get_trust_dashboard: { params: { days?: number }; result: TrustSummary };
  record_intelligence_feedback: { params: { eventType: string; signalId?: string; alertId?: string; sourceType?: string; topic?: string; notes?: string }; result: null };
  get_domain_precision_report: { params: { days?: number }; result: DomainPrecision[] };
  get_false_positive_analysis: { params: { days?: number }; result: FalsePositiveAnalysis };

  // -- Developer DNA --
  get_developer_dna: { params: Record<string, never>; result: DeveloperDna };
  export_developer_dna_markdown: { params: Record<string, never>; result: string };
  export_developer_dna_svg: { params: Record<string, never>; result: string };
  export_developer_dna_card: { params: Record<string, never>; result: string };

  // -- Tech Radar --
  get_tech_radar: { params: Record<string, never>; result: TechRadarData };
  get_radar_entry: { params: { name: string }; result: RadarEntry | null };
  get_radar_at_snapshot: { params: { snapshotDate: string }; result: TechRadarData };
  get_radar_entry_detail: { params: { name: string }; result: RadarEntryDetail };
  get_radar_snapshots: { params: Record<string, never>; result: Array<{ date: string }> };

  // -- Project Health --
  get_project_health: { params: Record<string, never>; result: ProjectHealth[] };

  // -- Semantic Shifts --
  get_semantic_shifts: { params: { lookbackDays?: number }; result: SemanticShift[] };

  // -- Void Engine --
  get_void_signal: { params: Record<string, never>; result: VoidSignal };

  // -- Source Configuration --
  get_rss_feeds: { params: Record<string, never>; result: { feeds: string[]; count: number } };
  set_rss_feeds: { params: { feeds: string[] }; result: void };
  get_youtube_channels: { params: Record<string, never>; result: { channels: string[]; count: number } };
  set_youtube_channels: { params: { channels: string[] }; result: void };
  get_twitter_handles: { params: Record<string, never>; result: { handles: string[]; count: number } };
  set_twitter_handles: { params: { handles: string[] }; result: void };
  has_x_api_key: { params: Record<string, never>; result: boolean };
  set_x_api_key: { params: { key: string }; result: void };
  get_github_languages: { params: Record<string, never>; result: { languages: string[]; count: number } };
  set_github_languages: { params: { languages: string[] }; result: void };
  get_sources: { params: Record<string, never>; result: SourceInfo[] };

  // -- Locale & i18n --
  get_locale: { params: Record<string, never>; result: { country: string; language: string; currency: string } };
  set_locale: { params: { country: string; language: string; currency: string }; result: void };
  format_currency: { params: { amount: number }; result: string };
  get_translation_status: { params: { lang: string }; result: TranslationStatus };
  get_all_translations: { params: { lang: string }; result: Record<string, TranslationEntry> };
  save_translation_override: { params: { lang: string; namespace: string; key: string; value: string }; result: void };
  get_translation_overrides: { params: { lang: string }; result: Record<string, string> };
  delete_translation_override: { params: { lang: string; namespace: string; key: string }; result: void };
  trigger_translation: { params: { lang: string }; result: void };

  // -- Content Translation (real-time feed/briefing translation) --
  translate_content: { params: { id: string; text: string; source_lang?: string }; result: ContentTranslationResult };
  translate_content_batch: { params: { items: ContentTranslationRequest[] }; result: ContentTranslationResult[] };
  get_content_translation_settings: { params: Record<string, never>; result: ContentTranslationSettings };
  get_translation_cache_stats: { params: Record<string, never>; result: TranslationCacheStats };
  purge_translation_cache: { params: Record<string, never>; result: number };
  get_translation_config: { params: Record<string, never>; result: TranslationConfig };
  set_translation_config: { params: { config: TranslationConfig }; result: null };

  // -- Embedding Model --
  get_embedding_model_info: { params: Record<string, never>; result: { model: string; reembed_in_progress: boolean; multilingual_model: string } };

  // -- STREETS Localization --
  get_regional_data: { params: Record<string, never>; result: RegionalData };
  calculate_electricity_cost: { params: { watts: number; hoursPerDay: number }; result: ElectricityCostResult };

  // -- Digest --
  get_digest_config: { params: Record<string, never>; result: DigestConfig };
  set_digest_config: { params: { enabled: boolean }; result: void };
  test_digest_email: { params: Record<string, never>; result: string };
  set_digest_email_config: { params: {
    email?: string;
    smtp_host?: string;
    smtp_port?: number;
    smtp_username?: string;
    smtp_password?: string;
    smtp_from?: string;
    smtp_use_tls?: boolean;
  }; result: string };

  // -- Toolkit --
  toolkit_list_ports: { params: Record<string, never>; result: ListeningPort[] };
  toolkit_kill_process: { params: { pid: number }; result: string };
  toolkit_http_request: { params: { request: HttpProbeRequest }; result: HttpProbeResponse };
  toolkit_get_http_history: { params: { limit: number }; result: HttpHistoryEntry[] };
  toolkit_test_feed: { params: { url: string }; result: FeedTestResult };
  toolkit_score_sandbox: { params: { title: string; content: string | null; sourceType: string }; result: SandboxScoreResult };
  toolkit_generate_export_pack: { params: Record<string, never>; result: ExportPackResult };
  toolkit_env_snapshot: { params: { workingDir: string | null }; result: EnvSnapshot };

  // -- Channels --
  list_channels: { params: Record<string, never>; result: ChannelSummary[] };
  get_channel: { params: { channelId: number }; result: Channel };
  get_channel_content: { params: { channelId: number }; result: ChannelRender | null };
  render_channel_now: { params: { channelId: number }; result: ChannelRender };
  get_channel_provenance: { params: { renderId: number }; result: RenderProvenance[] };
  get_channel_changelog: { params: { channelId: number }; result: ChannelChangelog | null };
  get_channel_sources: { params: { channelId: number; limit?: number }; result: ChannelSourceMatch[] };
  refresh_channel_sources: { params: { channelId: number }; result: number };
  auto_render_all_channels: { params: Record<string, never>; result: void };
  create_custom_channel: { params: { slug: string; title: string; description: string; topicQuery: string[] }; result: number };
  preview_channel_sources: { params: { topics: string[] }; result: { count: number; topTitles: string[] } };
  delete_channel: { params: { channelId: number }; result: void };

  // -- Startup Health --
  get_startup_health: { params: Record<string, never>; result: StartupHealthIssue[] };
  get_diagnostic_report: { params: Record<string, never>; result: DiagnosticReport };
  get_recent_logs: { params: { lines: number }; result: string };

  // -- Capabilities --
  get_capability_states: { params: Record<string, never>; result: Record<string, { state: string; reason?: string; since?: string; fallback?: string; remediation?: string }> };
  get_capability_summary: { params: Record<string, never>; result: { full: number; degraded: number; unavailable: number; total: number } };

  // -- Tech Narratives (LLM-synthesized) --
  generate_tech_narratives: { params: Record<string, never>; result: { narratives: Record<string, string> } };

  // -- Error Telemetry --
  get_error_telemetry: { params: { limit?: number }; result: { id: number; category: string; message: string; context: string | null; count: number; first_seen: string; last_seen: string }[] };
  get_error_summary_cmd: { params: Record<string, never>; result: { total_errors: number; total_occurrences: number; by_category: { category: string; unique_errors: number; total_occurrences: number }[]; top_errors: { id: number; category: string; message: string; context: string | null; count: number; first_seen: string; last_seen: string }[] } };
  clear_error_telemetry: { params: { days?: number }; result: number };
  get_security_audit_log: { params: { limit?: number; event_filter?: string }; result: { entries: { id: number; timestamp: string; event_type: string; details: string; severity: string }[]; total: number } };

  // -- Scoring Validation --
  run_scoring_validation: { params: Record<string, never>; result: { timestamp: string; personas: { persona: string; precision_at_20: number; anti_topic_leaks: number; avg_score_relevant: number; avg_score_irrelevant: number; separation: number; top_20_titles: string[]; items_scored: number }[]; overall_precision: number; worst_persona: string; best_persona: string; separation_score: number; recommendations: string[]; total_items_in_db: number } };

  // -- Splash Probes --
  get_context_stats: { params: Record<string, never>; result: ContextStats };

  // -- Natural Language Search (Pro) --
  synthesize_search: { params: { queryText: string }; result: SynthesisResponse };

  // -- Weekly Intelligence Digest (Pro) --
  generate_weekly_digest: { params: Record<string, never>; result: WeeklyDigest };
  get_latest_digest: { params: Record<string, never>; result: WeeklyDigest };

  // -- Decision Impact Tracking (Pro) --
  get_decision_signals: { params: Record<string, never>; result: DecisionSignals[] };
  get_decision_health_report: { params: Record<string, never>; result: DecisionHealthReport[] };

  // -- Standing Queries (Pro) --
  create_standing_query: { params: { queryText: string }; result: number };
  list_standing_queries: { params: Record<string, never>; result: StandingQuery[] };
  delete_standing_query: { params: { id: number }; result: void };
  get_standing_query_matches: { params: { id: number; limit?: number }; result: StandingQueryMatch[] };
  get_standing_query_suggestions: { params: Record<string, never>; result: StandingQuerySuggestion[] };

  // -- Intelligence Packs --
  list_intelligence_packs: { params: Record<string, never>; result: IntelligencePack[] };
  activate_intelligence_pack: { params: { packId: string }; result: null };
  deactivate_intelligence_pack: { params: { packId: string }; result: null };
  suggest_intelligence_packs: { params: Record<string, never>; result: PackSuggestion[] };

  // -- Intelligence History --
  get_intelligence_growth: { params: Record<string, never>; result: IntelligenceGrowthData };
  get_session_diff: { params: Record<string, never>; result: { new_items: number; new_relevant: number; hours_since_last: number; has_previous: boolean } };

  // -- Community Intelligence --
  get_community_status: { params: Record<string, never>; result: CommunityStatus };
  set_community_intelligence_enabled: { params: { enabled: boolean }; result: void };
  set_community_frequency: { params: { frequency: string }; result: void };

  // -- Stack Health --
  get_stack_health: { params: Record<string, never>; result: StackHealthData };
  get_missed_intelligence: { params: { days?: number }; result: MissedIntelligence };

  // -- Telemetry (Local, Privacy-First) --
  track_event: { params: { eventType: string; viewId?: string; metadata?: string }; result: void };
  get_usage_analytics: { params: { days?: number }; result: UsageReport };
  clear_telemetry: { params: Record<string, never>; result: void };

  // -- GAME Engine --
  get_achievement_state: { params: Record<string, never>; result: ActivitySnapshot };
  get_achievements: { params: Record<string, never>; result: AchievementUnlocked[] };
  check_daily_streak: { params: Record<string, never>; result: AchievementUnlocked[] };

  // -- Team Sync (AD-023) --
  get_team_sync_status: { params: Record<string, never>; result: TeamSyncStatus };
  get_team_members: { params: Record<string, never>; result: TeamMember[] };
  share_dna_with_team: { params: { primaryStack: string[]; interests: string[]; blindSpots: string[]; identitySummary: string }; result: string };
  share_signal_with_team: { params: { signalId: string; chainName: string; priority: string; techTopics: string[]; suggestedAction: string }; result: string };
  propose_team_decision: { params: { decisionId: string; title: string; decisionType: string; rationale: string }; result: string };
  create_team: { params: { relayUrl: string; displayName: string }; result: { team_id: string; client_id: string; role: string } };
  create_team_invite: { params: { role?: string; email?: string }; result: { code: string; expires_at: string } };
  join_team_via_invite: { params: { relayUrl: string; inviteCode: string; displayName: string }; result: { team_id: string; client_id: string; role: string; awaiting_team_key: boolean } };

  // -- Team Intelligence --
  get_team_profile_cmd: { params: Record<string, never>; result: TeamProfile };
  get_team_blind_spots_cmd: { params: Record<string, never>; result: TeamBlindSpot[] };
  get_bus_factor_report_cmd: { params: Record<string, never>; result: UniqueStrength[] };
  get_team_signal_summary_cmd: { params: Record<string, never>; result: TeamSignalSummary[] };

  // -- Team Monitoring --
  get_team_signals_cmd: { params: { includeResolved?: boolean }; result: TeamSignal[] };
  resolve_team_signal_cmd: { params: { signalId: string; notes: string }; result: void };
  get_alert_policy_cmd: { params: Record<string, never>; result: AlertPolicy };
  set_alert_policy_cmd: { params: { minSeats?: number; autoResolveHours?: number; notifyOnCorroboration?: boolean }; result: void };
  get_monitoring_summary_cmd: { params: Record<string, never>; result: TeamMonitoringSummary };

  // -- Team Decisions --
  vote_on_decision: { params: { decisionId: string; stance: string; rationale: string }; result: string };
  get_team_decisions: { params: { statusFilter: string | null }; result: TeamDecision[] };
  get_decision_detail: { params: { decisionId: string }; result: DecisionDetail };
  resolve_decision: { params: { decisionId: string; newStatus: string }; result: void };

  // -- Team Notifications --
  get_team_notifications: { params: { limit?: number; unreadOnly?: boolean }; result: TeamNotification[] };
  get_notification_summary: { params: Record<string, never>; result: NotificationSummary };
  mark_notification_read: { params: { notificationId: string }; result: void };
  mark_all_notifications_read: { params: Record<string, never>; result: void };
  dismiss_notification: { params: { notificationId: string }; result: void };

  // -- Team Shared Sources --
  share_source_with_team: { params: { sourceType: string; configSummary: string; recommendation: string }; result: string };
  get_team_sources: { params: Record<string, never>; result: SharedSource[] };
  upvote_team_source: { params: { sourceId: string }; result: void };
  remove_team_source: { params: { sourceId: string }; result: void };

  // -- Enterprise: Audit Log --
  get_audit_log: { params: { actionFilter?: string; resourceTypeFilter?: string; limit?: number; offset?: number }; result: AuditEntry[] };
  get_audit_summary_cmd: { params: { days?: number }; result: AuditSummary };
  export_audit_csv_cmd: { params: { from: string; to: string }; result: string };

  // -- Enterprise: Webhooks --
  register_webhook_cmd: { params: { name: string; url: string; events: string[] }; result: Webhook };
  list_webhooks_cmd: { params: Record<string, never>; result: Webhook[] };
  delete_webhook_cmd: { params: { webhookId: string }; result: void };
  test_webhook_cmd: { params: { webhookId: string }; result: boolean };
  get_webhook_deliveries_cmd: { params: { webhookId: string; limit?: number }; result: WebhookDelivery[] };

  // -- Enterprise: Organizations --
  get_organization_cmd: { params: Record<string, never>; result: Organization | null };
  get_org_teams_cmd: { params: Record<string, never>; result: OrgTeamSummary[] };
  get_retention_policies_cmd: { params: Record<string, never>; result: RetentionPolicy[] };
  set_retention_policy_cmd: { params: { resourceType: string; days: number }; result: void };
  get_cross_team_signals_cmd: { params: Record<string, never>; result: CrossTeamCorrelation[] };

  // -- Enterprise: Analytics --
  get_org_analytics_cmd: { params: { days?: number }; result: OrgAnalytics };
  export_org_analytics_cmd: { params: { days?: number }; result: string };

  // -- Enterprise: SSO --
  get_sso_config: { params: Record<string, never>; result: SsoConfig | null };
  set_sso_config: { params: { config: SsoConfig }; result: void };
  initiate_sso_login: { params: Record<string, never>; result: string };
  get_sso_session: { params: Record<string, never>; result: SsoSession | null };
  validate_sso_callback: { params: { assertion: string }; result: SsoSession };
  logout_sso: { params: Record<string, never>; result: void };

  // -- Data Export --
  export_all_data: { params: { format: string }; result: ExportManifest };
  export_section: { params: { section: string; format: string }; result: string };
  list_exports: { params: Record<string, never>; result: ExportManifest[] };
  delete_export: { params: { exportId: string }; result: void };

  // -- Dependency Intelligence --
  get_dependency_overview: { params: Record<string, never>; result: DependencyOverview };
  get_project_deps: { params: { projectPath: string }; result: ProjectDepsResult };
  get_dependency_alerts: { params: Record<string, never>; result: DependencyAlertsResult };
  resolve_dependency_alert: { params: { alertId: number }; result: void };
  check_dependency_upgrades: { params: Record<string, never>; result: { checked: number; upgrades_available: number; upgrades: Array<{ package: string; ecosystem: string; current: string; latest: string; is_major_upgrade: boolean; project: string }> } };
  get_license_overview: { params: Record<string, never>; result: { total: number; compatible: number; caution: number; warning: number; unknown: number; issues: Array<{ package: string; license: string; status: string; reason: string }> } };

  // -- Accuracy Tracking (Phase 4.1) --
  get_accuracy_report: { params: { period?: string }; result: AccuracyReport };
  get_intelligence_report: { params: { period?: string }; result: IntelligenceReportData };

  // -- Temporal Graph (Phase 4.5) --
  get_temporal_snapshot: { params: { period?: string }; result: TemporalSnapshot };
  get_adoption_curves: { params: Record<string, never>; result: TechAdoptionCurve[] };
  get_knowledge_decay_report: { params: Record<string, never>; result: KnowledgeDecayEntry[] };

  // -- Tech Convergence (Phase 6.3) --
  get_tech_convergence: { params: Record<string, never>; result: TechConvergenceReport };
  get_project_health_comparison: { params: Record<string, never>; result: ProjectHealthComparison };
  get_cross_project_dependencies: { params: Record<string, never>; result: CrossProjectDep[] };

  // -- AI Cost Tracking (Phase 8.2) --
  get_ai_usage_summary: { params: { period?: string }; result: AiUsageSummary };
  get_ai_cost_estimate: { params: { provider: string; model: string; tokensIn: number; tokensOut: number }; result: AiCostEstimate };
  get_ai_cost_recommendation: { params: Record<string, never>; result: AiCostRecommendation };

  // -- Source Plugin API (Phase 7) --
  list_plugins: { params: Record<string, never>; result: PluginManifest[] };
  fetch_plugin_items: { params: { plugin_name: string }; result: PluginItem[] };
  fetch_all_plugins: { params: Record<string, never>; result: PluginItem[] };

  // -- Waitlist --
  save_waitlist_signup: { params: { tier: string; email: string; name?: string | null; teamSize?: string | null; company?: string | null; role?: string | null }; result: { success: boolean; tier: string; email: string } };
  get_waitlist_signups: { params: Record<string, never>; result: Array<{ id: number; tier: string; email: string; name: string | null; team_size: string | null; company: string | null; role: string | null; source: string; signed_up_at: string }> };

}

// ============================================================================
// Types referenced above but not yet in shared type files
// (These can be moved to src/types/ as they get formalized)
// ============================================================================

/** Content integrity verification report (mirrors Rust IntegrityReport) */
interface ContentIntegrityReport {
  passed: boolean;
  filtered_tech: Array<{ name: string; reason: string; source: string }>;
  phantom_tech: Array<{ name: string; detected_confidence: number; category: string; auto_removed: boolean }>;
  verified_stack: string[];
  auto_corrected: number;
  checked_at: string;
}

/** First-run simulation audit report (mirrors Rust FirstRunAuditReport) */
interface FirstRunAuditReport {
  passed: boolean;
  lessons_audited: number;
  unresolved_templates: Array<{ module_id: string; lesson_idx: number; severity: string; category: string; description: string; fragment: string }>;
  fallback_only_fields: Array<{ module_id: string; lesson_idx: number; severity: string; category: string; description: string; fragment: string }>;
  broken_markers: Array<{ module_id: string; lesson_idx: number; severity: string; category: string; description: string; fragment: string }>;
  total_issues: number;
  critical_issues: number;
  checked_at: string;
}

/** Personalization context summary (mirrors Rust get_personalization_context_summary JSON) */
interface PersonalizationContextSummary {
  profile_completeness: number;
  has_llm: boolean;
  llm_tier: string;
  gpu_tier: string;
  os_family: string;
  stack_count: number;
  interest_count: number;
  completed_modules: string[];
  completed_lessons: number;
  regional_country: string;
  dna_available: boolean;
  context_hash: string;
}

/** Execution log entry (mirrors Rust row_to_json in sovereign_profile.rs) */
interface ExecutionLogEntry {
  id: number;
  module_id: string;
  lesson_idx: number;
  command_id: string;
  command_text: string;
  success: boolean;
  exit_code: number | null;
  stdout: string | null;
  stderr: string | null;
  duration_ms: number | null;
  executed_at: string | null;
}

/** Registered source info (mirrors Rust get_sources JSON) */
interface SourceInfo {
  type: string;
  name: string;
  enabled: boolean;
  max_items: number;
  fetch_interval_secs: number;
  category: string;
  label: string;
  color_hint: string;
  default_content_type: string;
}

/** Electricity cost calculation result (mirrors Rust calculate_electricity_cost JSON) */
interface ElectricityCostResult {
  kwh_per_day: string;
  daily_cost: string;
  monthly_cost: string;
  yearly_cost: string;
  rate_per_kwh: string;
  currency: string;
}

/** Context engine statistics (mirrors Rust get_context_stats JSON) */
interface ContextStats {
  interests: number;
  exclusions: number;
  tech_stack: number;
  domains: number;
  has_role: boolean;
}

/** Game engine state (mirrors Rust ActivitySnapshot) */
interface ActivitySnapshot {
  counters: Array<{ counter_type: string; value: number }>;
  achievements: Array<{
    id: string;
    name: string;
    description: string;
    icon: string;
    counter_type: string;
    threshold: number;
    tier: 'bronze' | 'silver' | 'gold';
    current: number;
    unlocked: boolean;
    unlocked_at: string | null;
  }>;
  streak: number;
  last_active: string | null;
}

/** Achievement unlock event (mirrors Rust AchievementUnlocked) */
interface AchievementUnlocked {
  id: string;
  name: string;
  description: string;
  icon: string;
  tier: 'bronze' | 'silver' | 'gold';
  celebration_intensity: number;
  unlocked_at: string;
}

/** Startup health issue (mirrors Rust HealthIssue) */
interface StartupHealthIssue {
  component: string;
  severity: 'warning' | 'error';
  message: string;
}

/** Diagnostic report for support/troubleshooting (mirrors Rust DiagnosticReport) */
interface DiagnosticReport {
  app_version: string;
  platform: string;
  arch: string;
  data_dir: string;
  db_size_bytes: number;
  settings_exists: boolean;
  disk_available_mb: number;
  health_issues: StartupHealthIssue[];
}


/** Team sync status (mirrors Rust TeamSyncStatus) */
interface TeamSyncStatus {
  enabled: boolean;
  connected: boolean;
  team_id: string | null;
  client_id: string | null;
  display_name: string | null;
  role: string | null;
  member_count: number;
  pending_outbound: number;
  last_sync_at: string | null;
  last_relay_seq: number;
}

/** A registered team member (mirrors Rust TeamMember) */
interface TeamMember {
  client_id: string;
  display_name: string;
  role: string;
  last_seen: string | null;
}

// -- Team Intelligence Types --

interface TeamProfile {
  team_id: string;
  member_count: number;
  collective_stack: TeamTechEntry[];
  stack_coverage: number;
  blind_spots: TeamBlindSpot[];
  overlap_zones: OverlapZone[];
  unique_strengths: UniqueStrength[];
  generated_at: string;
}

interface TeamTechEntry {
  tech: string;
  members: string[];
  team_confidence: number;
}

interface TeamBlindSpot {
  topic: string;
  related_to: string[];
  severity: string;
}

interface OverlapZone {
  topic: string;
  members: string[];
  member_count: number;
}

interface UniqueStrength {
  tech: string;
  sole_expert: string;
  risk_level: string;
}

interface TeamSignalSummary {
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

interface MemberDetection {
  client_id: string;
  display_name: string;
  detected_at: string;
}

// -- Team Monitoring Types --

interface TeamSignal {
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

interface AlertPolicy {
  min_seats_for_alert: number;
  auto_resolve_hours: number;
  notify_on_corroboration: boolean;
}

interface TeamMonitoringSummary {
  active_signals: number;
  resolved_signals: number;
  avg_detection_count: number;
  top_signal_types: [string, number][];
}

// -- Enterprise: Audit Types --

interface AuditEntry {
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

interface AuditSummary {
  total_events: number;
  events_by_action: [string, number][];
  events_by_actor: [string, number][];
  events_by_day: [string, number][];
}

// -- Enterprise: Webhook Types --

interface Webhook {
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

interface WebhookDelivery {
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

interface Organization {
  id: string;
  name: string;
  team_count: number;
  total_seats: number;
  created_at: string;
}

interface OrgTeamSummary {
  team_id: string;
  member_count: number;
  last_active: string | null;
}

interface RetentionPolicy {
  resource_type: string;
  retention_days: number;
}

interface CrossTeamCorrelation {
  correlation_id: string;
  signal_type: string;
  teams_affected: [string, number][];
  org_severity: string;
  first_detected: string;
  recommendation: string;
}

// -- Enterprise: Analytics Types --

interface OrgAnalytics {
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

interface TeamActivity {
  team_id: string;
  active_members: number;
  signals_this_period: number;
  decisions_this_period: number;
  engagement_score: number;
}

// -- Team Decision Types --

interface TeamDecision {
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

interface DecisionVote {
  voter_id: string;
  stance: string;
  rationale: string;
  voted_at: string;
}

interface DecisionDetail {
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

interface TeamNotification {
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

interface NotificationSummary {
  total_unread: number;
  by_type: { notification_type: string; count: number }[];
}

// -- Shared Source Types --

interface SharedSource {
  id: string;
  team_id: string;
  source_type: string;
  config_summary: Record<string, unknown>;
  recommendation: string;
  shared_by: string;
  upvotes: number;
  created_at: string;
}

// -- SSO Types --

interface SsoConfig {
  provider_type: string;
  idp_url: string;
  entity_id: string;
  certificate: string | null;
  client_id: string | null;
  issuer: string | null;
  enabled: boolean;
}

interface SsoSession {
  email: string;
  display_name: string;
  groups: string[];
  authenticated_at: string;
  expires_at: string | null;
  provider_type: string;
}

// -- Data Export Types --

interface ExportManifest {
  export_id: string;
  created_at: string;
  format: string;
  sections: string[];
  total_records: number;
}

/** Developer decision (mirrors Rust DeveloperDecision) */
interface DeveloperDecision {
  id: number;
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string[];
  context_tags: string[];
  confidence: number;
  status: string;
  superseded_by: number | null;
  created_at: string;
  updated_at: string;
}

interface AgentMemoryEntry {
  id: number;
  session_id: string;
  agent_type: string;
  memory_type: string;
  subject: string;
  content: string;
  context_tags: string[];
  expires_at: string | null;
  created_at: string;
}

interface DelegationScoreEntry {
  task_type: string;
  score: number;
  factors: Record<string, number>;
  recommendation: string;
}

interface ProValueReport {
  total_saved_hours: number;
  total_signals_processed: number;
  top_discoveries: Array<{ title: string; impact: string }>;
  period_days: number;
}

interface StreetHealthScore {
  overall: number;
  module_scores: Array<{
    module_id: string;
    module_name: string;
    score: number;
    sun_count: number;
    success_rate: number;
    lessons_completed: number;
    total_lessons: number;
    last_activity: string | null;
  }>;
  trend: string;
  top_action: string;
}

interface SovereignProfileData {
  facts: Record<string, Record<string, string>>;
  completeness: number;
}

interface ProfileCompleteness {
  overall: number;
  by_category: Record<string, number>;
  missing_keys: string[];
}

interface NLQResult {
  query: string;
  results: SourceRelevance[];
  interpretation: string;
}

interface ScoreAutopsyResult {
  item_id: number;
  breakdown: ScoreBreakdown;
  analysis: string;
}

interface EngagementData {
  total_interactions: number;
  saves: number;
  dismissals: number;
  clicks: number;
  engagement_rate: number;
  top_topics: Array<{ topic: string; count: number }>;
}

interface TechRadarData {
  entries: Array<{
    name: string;
    quadrant: string;
    ring: string;
    movement: string;
    description: string;
  }>;
  generated_at: string;
}

interface RadarEntryDetail {
  name: string;
  quadrant: string;
  ring: string;
  movement: string;
  description: string;
  history: Array<{ date: string; ring: string; event: string }>;
  related_decisions: DeveloperDecision[];
}

interface TranslationStatus {
  language: string;
  total_keys: number;
  translated_keys: number;
  override_count: number;
  coverage: number;
}

interface TranslationEntry {
  key: string;
  namespace: string;
  default_value: string;
  translated_value: string | null;
  override_value: string | null;
}

// Content translation (real-time feed/briefing translation)
interface ContentTranslationRequest {
  id: string;
  text: string;
  source_lang?: string;
}

interface ContentTranslationResult {
  id: string;
  original: string;
  translated: string;
  from_cache: boolean;
  provider: string;
}

interface ContentTranslationSettings {
  enabled: boolean;
  provider: string;
  target_lang: string;
}

interface TranslationCacheStats {
  target_lang: string;
  total_entries: number;
  active_entries: number;
  total_lookups: number;
}

export interface TranslationConfig {
  provider: string;
  api_key: string;
  auto_translate: boolean;
  translate_descriptions: boolean;
  cloud_translation_consent: boolean;
}

interface DigestConfig {
  enabled: boolean;
  schedule: string;
  last_sent: string | null;
}

interface ListeningPort {
  port: number;
  pid: number;
  process_name: string;
  protocol: string;
}

interface HttpProbeRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string | null;
}

interface HttpProbeResponse {
  status: number;
  status_text: string;
  headers: Record<string, string>;
  body: string;
  duration_ms: number;
  size_bytes: number;
}

interface HttpHistoryEntry {
  id: number;
  method: string;
  url: string;
  status: number;
  duration_ms: number;
  created_at: string;
}

interface FeedTestItem {
  title: string;
  url: string;
  published_at: string | null;
  content_preview: string;
}

interface FeedTestResult {
  feed_title: string | null;
  format: string;
  item_count: number;
  items: FeedTestItem[];
  fetch_duration_ms: number;
  errors: string[];
}

interface SandboxScoreResult {
  score: number;
  breakdown: ScoreBreakdown;
  explanation: string;
}

interface ExportPackResult {
  file_path: string;
  size_bytes: number;
  items_exported: number;
}

interface EnvSnapshot {
  os: string;
  arch: string;
  node_version: string | null;
  rust_version: string | null;
  tools: Array<{ name: string; version: string | null; available: boolean }>;
  env_vars: Record<string, string>;
}

interface ScanSummary {
  languages: string[];
  frameworks: string[];
  rust_deps: number;
  npm_deps: number;
  python_deps: number;
  other_deps: number;
  total_topics: number;
  topics: string[];
}

interface ScoringStats {
  total_runs: number;
  total_scored: number;
  total_relevant: number;
  lifetime_rejection_rate: number;
  last_run_rejection_rate: number | null;
}

interface DiagnosticsSnapshot {
  memory_bytes: number;
  db_size_bytes: number;
  source_item_count: number;
  context_chunk_count: number;
  feedback_count: number;
  uptime_secs: number;
  source_health: Array<{ source_type: string; status: string }>;
  schema_version: number;
}

interface ComposedStackSummary {
  active: boolean;
  pain_point_count: number;
  ecosystem_shift_count: number;
  keyword_boost_count: number;
  source_preferences: Array<[string, number]>;
  all_tech: string[];
  competing: string[];
}

interface AgentSessionBrief {
  generated_at: string;
  version: string;
  agent_type: string | null;
  active_decisions: Array<{ id: number; subject: string; decision: string; status: string; confidence: number }>;
  ecosystem_changes: Array<{ topic: string; change_type: string; summary: string }>;
  active_concerns: Array<{ concern_type: string; subject: string; severity: string; detail: string }>;
  open_signals: Array<{ chain_id: string; topic: string; signal_count: number; latest_signal: string }>;
  recent_briefing: string | null;
  agent_memories: AgentMemoryEntry[];
}

interface DelegationScoreResult {
  subject: string;
  overall_score: number;
  factors: { complexity: number; reversibility: number; domain_familiarity: number; time_sensitivity: number; strategic_value: number };
  recommendation: { level: string; summary: string };
  caveats: string[];
}

interface IntelligencePulseData {
  items_analyzed_7d: number;
  items_surfaced_7d: number;
  rejection_rate: number;
  calibration_accuracy: number;
  top_calibrations: Array<{ topic: string; delta: number; sample_size: number; confidence: number }>;
  source_quality: Array<{ source_type: string; items_surfaced: number; items_engaged: number; engagement_rate: number }>;
  anti_patterns_detected: number;
  total_cycles: number;
  learning_narratives: string[];
}

/** @deprecated Use PersonalizedLesson from types/personalization.ts instead */
type PersonalizedLesson = PersonalizedLessonType;

interface SovereignDeveloperProfileData {
  generated_at: string;
  identity_summary: string;
  infrastructure: InfrastructureDimension;
  stack: StackDimension;
  skills: SkillsDimension;
  preferences: PreferencesDimension;
  context: ContextDimension;
  intelligence: IntelligenceReport;
  completeness: CompletenessReport;
}

interface RadarEntry {
  name: string;
  ring: string;
  quadrant: string;
  movement: string;
  signals: string[];
  decision_ref: number | null;
  score: number;
}

interface SemanticShift {
  topic: string;
  drift_magnitude: number;
  direction: string;
  representative_items: number[];
  period: string;
  detected_at: string;
}

interface RegionalData {
  country: string;
  currency: string;
  currency_symbol: string;
  electricity_kwh: number;
  internet_typical_monthly: number;
  business_registration_cost: number;
  business_entity_type: string;
  tax_note: string;
  payment_processors: string[];
  bank_recommendation: string;
  isp_note: string;
}

interface ChannelSummary {
  id: number;
  slug: string;
  title: string;
  description: string;
  source_count: number;
  render_count: number;
  freshness: string;
  last_rendered_at: string | null;
}

interface Channel {
  id: number;
  slug: string;
  title: string;
  description: string;
  topic_query: string[];
  status: string;
  source_count: number;
  render_count: number;
  last_rendered_at: string | null;
  created_at: string;
  updated_at: string;
}

interface ChannelRender {
  id: number;
  channel_id: number;
  version: number;
  content_markdown: string;
  content_hash: string;
  source_item_ids: number[];
  model: string | null;
  tokens_used: number | null;
  latency_ms: number | null;
  rendered_at: string;
}

interface RenderProvenance {
  render_id: number;
  claim_index: number;
  claim_text: string;
  source_item_ids: number[];
  source_titles: string[];
  source_urls: string[];
}

interface ChannelChangelog {
  channel_id: number;
  from_version: number;
  to_version: number;
  summary: string;
  added_lines: string[];
  removed_lines: string[];
  changed_at: string;
}

interface ChannelSourceMatch {
  channel_id: number;
  source_item_id: number;
  title: string;
  url: string | null;
  source_type: string;
  match_score: number;
  matched_at: string;
}

interface SynthesisResponse {
  text: string;
  sources: Array<{ title: string; url: string | null; source_type: string }>;
  grounding_count: number;
  total_sources: number;
}

interface WeeklyDigest {
  generated_at: string;
  period_start: string;
  period_end: string;
  highlights: Array<{ title: string; source_type: string; score: number }>;
  top_topics: Array<{ topic: string; count: number }>;
  active_signals: Array<{ chain_id: string; topic: string }>;
  knowledge_gaps: Array<{ topic: string; gap_type: string }>;
  stats: { items_analyzed: number; items_surfaced: number; sources_active: number };
}

interface DecisionSignals {
  decision_id: number;
  subject: string;
  decision: string;
  supporting: Array<{ item_id: number; title: string; source_type: string; url: string | null; relevance: number; reason: string }>;
  challenging: Array<{ item_id: number; title: string; source_type: string; url: string | null; relevance: number; reason: string }>;
}

interface DecisionHealthReport {
  decision_id: number;
  subject: string;
  decision: string;
  created_at: string;
  days_since: number;
  supporting_count: number;
  challenging_count: number;
  volatility: number;
  status: 'confident' | 'challenged' | 'stale' | 'needs_review';
  latest_evidence: Array<{ item_id: number; title: string; source_type: string; url: string | null; relevance: number; reason: string; discovered_at: string }>;
}

interface StandingQuery {
  id: number;
  query_text: string;
  keywords: string[];
  created_at: string;
  last_run: string | null;
  total_matches: number;
  new_matches: number;
  active: boolean;
}

interface StandingQueryMatch {
  item_id: number;
  title: string;
  source_type: string;
  url: string | null;
  discovered_at: string | null;
}

interface StandingQuerySuggestion {
  topic: string;
  reason: string;
  engagement_count: number;
  query_type: string;
}

interface IntelligencePack {
  id: string;
  name: string;
  description: string;
  icon: string;
  concepts: Array<{ name: string; keywords: string[]; importance: number }>;
  default_watches: string[];
  active: boolean;
  activated_at: string | null;
}

interface PackSuggestion {
  pack_id: string;
  reason: string;
}

interface IntelligenceGrowthData {
  snapshots: Array<{ recorded_at: string; accuracy: number; topics_learned: number; items_analyzed: number; relevant_found: number }>;
  current_accuracy: number;
  total_topics: number;
  total_analyzed: number;
  total_relevant: number;
}

interface CommunityStatus {
  enabled: boolean;
  frequency: string;
  last_contributed: string | null;
  anonymous_id_preview: string | null;
}

interface StackHealthData {
  technologies: Array<{ name: string; signal_count: number; trend: string; health: string }>;
  stack_score: number;
  signals_this_week: number;
  suggested_queries: string[];
  missed_signals: MissedIntelligence;
}

interface MissedIntelligence {
  total_count: number;
  critical_count: number;
  high_count: number;
  example_titles: string[];
}

interface UsageReport {
  period_days: number;
  total_events: number;
  sessions: number;
  view_usage: Array<{ view_id: string; count: number }>;
  search_count: number;
  synthesis_count: number;
  ghost_preview_impressions: number;
  ghost_preview_clicks: number;
  ghost_click_rate: number;
  avg_session_views: number;
}

// -- Dependency Intelligence Types --

interface DependencyOverview {
  total_dependencies: number;
  total_projects: number;
  direct_dependencies: number;
  dev_dependencies: number;
  ecosystems: Array<{ ecosystem: string; count: number }>;
  projects: Array<{ name: string; path: string; dependency_count: number; alert_count: number }>;
  alerts: { total: number; critical: number; high: number; medium: number; low: number };
  cross_project_packages: number;
  cross_project_top: Array<{ package_name: string; ecosystem: string; project_count: number }>;
}

interface ProjectDepsResult {
  project_name: string;
  project_path: string;
  dependencies: Array<{
    name: string;
    version: string | null;
    ecosystem: string;
    is_dev: boolean;
    is_direct: boolean;
    detected_at: string;
    last_seen_at: string;
    alerts: Array<{ id: number; severity: string; title: string; alert_type: string }>;
  }>;
  total: number;
}

interface DependencyAlertsResult {
  alerts: Array<{
    id: number;
    package_name: string;
    ecosystem: string;
    alert_type: string;
    severity: string;
    title: string;
    description: string | null;
    affected_versions: string | null;
    source_url: string | null;
    detected_at: string;
  }>;
  total: number;
}

// -- Accuracy Tracking Types --

interface AccuracyReport {
  id: number;
  period: string;
  total_scored: number;
  total_relevant: number;
  user_confirmed: number;
  user_rejected: number;
  accuracy_pct: number;
  created_at: string;
}

interface IntelligenceReportData {
  period: string;
  accuracy_current: number;
  accuracy_previous: number;
  accuracy_delta: number;
  topics_tracked: number;
  topics_added: number;
  noise_rejected: number;
  noise_rejection_pct: number;
  time_saved_hours: number;
  security_alerts: number;
  security_acted_on: number;
  decisions_recorded: number;
  feedback_signals: number;
}

// -- Temporal Graph Types --

interface TemporalSnapshot {
  period: string;
  tech_snapshot: Array<{ name: string; confidence: number; engagement_score: number }>;
  interest_snapshot: Array<{ topic: string; score: number }>;
  decision_count: number;
  feedback_count: number;
  created_at?: string;
}

interface TechAdoptionCurve {
  tech_name: string;
  first_seen: string;
  weeks_active: number;
  current_confidence: number;
  stage: string;
  engagement_history: number[];
}

interface KnowledgeDecayEntry {
  tech_name: string;
  last_engagement: string;
  weeks_since_engagement: number;
  risk_level: string;
  recommendation: string;
}

// -- Tech Convergence Types --

interface TechConvergenceReport {
  total_projects: number;
  shared_technologies: Array<{ name: string; category: string; project_count: number; adoption_pct: number }>;
  unique_technologies: Array<{ name: string; category: string; project_path: string; bus_factor_risk: string }>;
  convergence_score: number;
}

interface ProjectHealthComparison {
  projects: Array<{
    project_path: string;
    project_name: string;
    dependency_count: number;
    dev_dependency_count: number;
    freshness_score: number;
    vulnerability_count: number;
    ecosystems: string[];
  }>;
}

interface CrossProjectDep {
  name: string;
  ecosystem: string;
  projects: string[];
  project_count: number;
}

// -- AI Cost Tracking Types --

interface AiUsageSummary {
  period: string;
  total_cost_usd: number;
  total_tokens_in: number;
  total_tokens_out: number;
  by_provider: Array<{ provider: string; model: string; cost_usd: number; request_count: number }>;
  by_task: Array<{ task_type: string; cost_usd: number; request_count: number; avg_tokens: number }>;
  recommendation: AiCostRecommendation | null;
}

interface AiCostEstimate {
  provider: string;
  model: string;
  tokens_in: number;
  tokens_out: number;
  estimated_cost_usd: number;
}

interface AiCostRecommendation {
  current_provider: string;
  current_model: string;
  recommended_provider: string;
  recommended_model: string;
  estimated_savings_usd: number;
  quality_match_pct: number;
  reason: string;
}

/** Plugin manifest — metadata about an installed source plugin (mirrors Rust PluginManifest) */
interface PluginManifest {
  name: string;
  version: string;
  description: string;
  author: string | null;
  binary: string;
  poll_interval_secs: number;
  max_items: number;
}

/** Item returned by a source plugin (mirrors Rust PluginItem) */
interface PluginItem {
  title: string;
  url: string | null;
  content: string;
  source_type: string;
  author: string | null;
  published_at: string | null;
}


// ============================================================================
// Typed invoke — the core wrapper
// ============================================================================

/** Command names that require no parameters. */
type NoParamCommands = {
  [K in keyof CommandMap]: CommandMap[K]['params'] extends Record<string, never> ? K : never;
}[keyof CommandMap];

/** Command names that require parameters. */
type ParamCommands = Exclude<keyof CommandMap, NoParamCommands>;

/**
 * Type-safe Tauri invoke wrapper.
 *
 * @example
 * // No-param command:
 * const status = await cmd('get_analysis_status');
 *
 * // With params:
 * const result = await cmd('ace_record_interaction', {
 *   item_id: 42,
 *   action_type: 'save',
 *   action_data: null,
 *   item_topics: ['rust'],
 *   item_source: 'hn',
 * });
 */
export function cmd<K extends NoParamCommands>(
  command: K,
): Promise<CommandMap[K]['result']>;
// eslint-disable-next-line no-redeclare
export function cmd<K extends ParamCommands>(
  command: K,
  params: CommandMap[K]['params'],
): Promise<CommandMap[K]['result']>;
// eslint-disable-next-line no-redeclare
export function cmd<K extends keyof CommandMap>(
  command: K,
  params?: CommandMap[K]['params'],
): Promise<CommandMap[K]['result']> {
  const timeoutMs = LONG_RUNNING_COMMANDS.has(command) ? LONG_TIMEOUT_MS : DEFAULT_TIMEOUT_MS;
  return withTimeout(invoke<CommandMap[K]['result']>(command, params ?? {}), timeoutMs, command);
}

// ============================================================================
// IPC Timeout — prevents hanging if a Rust command deadlocks or stalls
// ============================================================================

const DEFAULT_TIMEOUT_MS = 30_000; // 30s for most commands
const LONG_TIMEOUT_MS = 120_000; // 120s for analysis, indexing, LLM calls

/** Commands that legitimately need extended timeouts */
const LONG_RUNNING_COMMANDS = new Set<string>([
  'run_cached_analysis',
  'index_context',
  'ace_full_scan',
  'pull_ollama_model',
  'natural_language_query',
  'synthesize_search',
  'generate_briefing',
  'setup_and_verify_ollama',
  'translate_content',
  'translate_content_batch',
  'translate_playbook_module',
  'get_embedding_model_info',
]);

class CommandTimeoutError extends Error {
  constructor(public readonly command: string, ms: number) {
    super(`Command '${command}' timed out after ${ms / 1000}s`);
    this.name = 'CommandTimeoutError';
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number, command?: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([
    promise,
    new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new CommandTimeoutError(command ?? 'unknown', ms)), ms);
    }),
  ]).finally(() => clearTimeout(timer));
}

export { CommandTimeoutError };

// ============================================================================
// Re-export types used by command consumers
// ============================================================================

export type { CommandMap };
export type CommandName = keyof CommandMap;
export type CommandParams<K extends CommandName> = CommandMap[K]['params'];
export type CommandResult<K extends CommandName> = CommandMap[K]['result'];

// Re-export locally-defined types for consumers that need them
export type {
  DeveloperDecision,
  AgentMemoryEntry,
  AgentSessionBrief,
  DelegationScoreEntry,
  DelegationScoreResult,
  ProValueReport,
  StreetHealthScore,
  SovereignProfileData,
  SovereignDeveloperProfileData,
  ProfileCompleteness,
  NLQResult,
  ScoreAutopsyResult,
  EngagementData,
  TechRadarData,
  RadarEntry,
  RadarEntryDetail,
  TranslationStatus,
  TranslationEntry,
  ContentTranslationRequest,
  ContentTranslationResult,
  ContentTranslationSettings,
  TranslationCacheStats,
  DigestConfig,
  ListeningPort,
  HttpProbeRequest,
  HttpProbeResponse,
  HttpHistoryEntry,
  FeedTestResult,
  SandboxScoreResult,
  ExportPackResult,
  EnvSnapshot,
  ScanSummary,
  ScoringStats,
  DiagnosticsSnapshot,
  ComposedStackSummary,
  IntelligencePulseData,
  PersonalizedLesson,
  SemanticShift,
  RegionalData,
  ChannelSummary,
  Channel,
  ChannelRender,
  RenderProvenance,
  ChannelChangelog,
  ChannelSourceMatch,
  SynthesisResponse,
  WeeklyDigest,
  DecisionSignals,
  DecisionHealthReport,
  StandingQuery,
  StandingQueryMatch,
  StandingQuerySuggestion,
  IntelligencePack,
  PackSuggestion,
  IntelligenceGrowthData,
  CommunityStatus,
  StackHealthData,
  MissedIntelligence,
  UsageReport,
  TeamSyncStatus,
  TeamMember,
  // Team Intelligence
  TeamProfile,
  TeamTechEntry,
  TeamBlindSpot,
  OverlapZone,
  UniqueStrength,
  TeamSignalSummary,
  MemberDetection,
  // Team Monitoring
  TeamSignal,
  AlertPolicy,
  TeamMonitoringSummary,
  // Enterprise: Audit
  AuditEntry,
  AuditSummary,
  // Enterprise: Webhooks
  Webhook,
  WebhookDelivery,
  // Enterprise: Organizations
  Organization,
  OrgTeamSummary,
  RetentionPolicy,
  CrossTeamCorrelation,
  // Enterprise: Analytics
  OrgAnalytics,
  TeamActivity,
  // Dependency Intelligence
  DependencyOverview,
  ProjectDepsResult,
  DependencyAlertsResult,
  // Accuracy & Temporal
  AccuracyReport,
  IntelligenceReportData,
  TemporalSnapshot,
  TechAdoptionCurve,
  KnowledgeDecayEntry,
  // Convergence & AI Costs
  TechConvergenceReport,
  ProjectHealthComparison,
  CrossProjectDep,
  AiUsageSummary,
  AiCostEstimate,
  AiCostRecommendation,
  // IPC type safety additions
  PersonalizationContextSummary,
  ExecutionLogEntry,
  SourceInfo,
  ElectricityCostResult,
  ContextStats,
  ActivitySnapshot,
  AchievementUnlocked,
  StartupHealthIssue,
  // Source Plugin API (Phase 7)
  PluginManifest,
  PluginItem,
};
