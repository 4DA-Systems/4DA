/**
 * Type-safe Tauri IPC command layer.
 *
 * Every invoke() call in the app should use `commands.commandName(params)`
 * instead of raw `invoke('command_name', { ... })`.
 *
 * Benefits:
 * - Compile-time checking of command names, parameter types, and return types
 * - IDE autocomplete for all 107 commands
 * - Single source of truth for the IPC contract
 *
 * Generated from Rust #[tauri::command] signatures + frontend usage analysis.
 */

import { invoke } from '@tauri-apps/api/core';

import type {
  SourceRelevance,
  ScoreBreakdown,
} from '../types/analysis';
import type { CalibrationResult, TasteTestStepResult, TasteProfileSummary } from '../types/calibration';
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
  SavedItem,
  ItemContent,
  ItemSummary,
} from '../types/sources';
import type {
  PredictedContext,
  KnowledgeGap,
  SignalChain,
  ProjectHealth,
  AttentionReport,
  AudioBriefingStatus,
  ContextPacket,
  DeveloperDna,
} from '../types/innovation';
import type {
  AutophagyCycleResult,
  AutophagyStatus,
  DecisionWindow,
  CompoundAdvantageScore,
} from '../types/autophagy';
import type {
  CoachSession,
  CoachMessage,
  EngineRecommendation,
  LaunchReviewResult,
  CoachNudge,
  CoachTemplate,
  VideoLesson,
  VideoCurriculumStatus,
  CoachSessionType,
} from '../types/coach';
import type {
  PlaybookModule,
  PlaybookContent,
  PlaybookProgress,
} from '../types/playbook';
import type { ParsedCommand, CommandExecutionResult } from '../types/streets';
import type {
  CommandOutput,
  GitDeckStatus,
  CommitSummary,
  SuggestedCommitMessage,
  CommandHistoryEntry,
  RepoInfo,
} from '../types/command-deck';
import type { StackProfileSummary, StackDetection } from '../types/stacks';


// ============================================================================
// Command Map — maps every command name to { params, result }
// ============================================================================

/** Full IPC contract: command name → parameter type & return type. */
interface CommandMap {
  // -- Analysis & Core --
  get_analysis_status: { params: Record<string, never>; result: { running: boolean; completed: boolean; error: string | null; results: SourceRelevance[] | null; started_at: number | null; last_completed_at: string | null } };
  run_cached_analysis: { params: Record<string, never>; result: void };
  cancel_analysis: { params: Record<string, never>; result: void };
  get_context_files: { params: Record<string, never>; result: ContextFile[] };
  clear_context: { params: Record<string, never>; result: string };
  index_context: { params: Record<string, never>; result: string };
  index_project_readmes: { params: Record<string, never>; result: string };
  export_results: { params: { format: string }; result: string };

  // -- Settings & Configuration --
  get_settings: { params: Record<string, never>; result: Settings };
  set_llm_provider: { params: { provider: string; apiKey: string; model: string; baseUrl: string | null; openaiApiKey?: string | null }; result: void };
  set_rerank_config: { params: { enabled: boolean; maxItems: number; minScore: number; dailyTokenLimit: number; dailyCostLimit: number }; result: void };
  test_llm_connection: { params: Record<string, never>; result: { success: boolean; message: string } };
  check_ollama_status: { params: { baseUrl: string | null }; result: { operational: boolean; models: string[]; error: string | null } };
  mark_onboarding_complete: { params: Record<string, never>; result: void };
  pull_ollama_model: { params: { model: string; baseUrl: string | null }; result: void };
  run_calibration: { params: Record<string, never>; result: CalibrationResult };
  set_close_to_tray: { params: { enabled: boolean }; result: void };

  // -- Taste Test Calibration --
  taste_test_start: { params: Record<string, never>; result: TasteTestStepResult };
  taste_test_respond: { params: { itemSlot: number; response: string }; result: TasteTestStepResult };
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
  set_selected_stacks: { params: { profileIds: string[] }; result: void };
  get_stack_profiles: { params: Record<string, never>; result: StackProfileSummary[] };
  detect_stack_profiles: { params: Record<string, never>; result: StackDetection[] };

  // -- ACE (Autonomous Context Engine) --
  ace_record_interaction: { params: { item_id: number; action_type: string; action_data: string | null; item_topics: string[]; item_source: string }; result: void };
  ace_record_accuracy_feedback: { params: { item_id: number; predicted_score: number; feedback_type: string }; result: void };
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

  // -- Context Directories --
  get_context_dirs: { params: Record<string, never>; result: string[] };
  set_context_dirs: { params: { dirs: string[] }; result: void };

  // -- Monitoring --
  get_monitoring_status: { params: Record<string, never>; result: MonitoringStatus };
  set_monitoring_enabled: { params: { enabled: boolean }; result: void };
  set_monitoring_interval: { params: { minutes: number }; result: void };
  set_notification_threshold: { params: { threshold: string }; result: void };
  trigger_notification_test: { params: Record<string, never>; result: void };

  // -- Briefing --
  get_latest_briefing: { params: Record<string, never>; result: { content: string; model: string | null; item_count: number; created_at: string } | null };
  generate_ai_briefing: { params: Record<string, never>; result: { success: boolean; briefing: string | null; error?: string; model?: string; item_count?: number; latency_ms?: number } };
  generate_free_briefing: { params: Record<string, never>; result: { content: string; item_count: number; created_at: string } };
  get_source_health_status: { params: Record<string, never>; result: SourceHealthStatus[] };

  // -- Decisions --
  get_decisions: { params: { limit?: number; decisionType?: string; status?: string }; result: DeveloperDecision[] };
  record_developer_decision: { params: { decisionType: string; subject: string; decision: string; rationale: string | null; alternativesRejected: string[]; contextTags: string[]; confidence: number }; result: DeveloperDecision };
  update_developer_decision: { params: { id: number; decision: string | null; rationale: string | null; status: string | null; confidence: number | null }; result: DeveloperDecision };

  // -- Agent Memory --
  recall_agent_memories: { params: { subject: string; limit: number }; result: AgentMemoryEntry[] };
  get_all_delegation_scores: { params: Record<string, never>; result: DelegationScoreEntry[] };
  promote_memory_to_decision: { params: { memoryId: number }; result: void };

  // -- Decision Advantage --
  get_decision_windows: { params: Record<string, never>; result: DecisionWindow[] };
  act_on_decision_window: { params: { windowId: number; outcome: string | null }; result: void };
  close_decision_window: { params: { windowId: number }; result: void };
  get_compound_advantage: { params: Record<string, never>; result: CompoundAdvantageScore };

  // -- Autophagy --
  get_autophagy_status: { params: Record<string, never>; result: AutophagyStatus };
  get_autophagy_history: { params: { limit: number }; result: AutophagyCycleResult[] };

  // -- License & Trial --
  get_license_tier: { params: Record<string, never>; result: { tier: string; has_key: boolean; activated_at: string | null; expires_at: string | null; days_remaining: number; expired: boolean } };
  activate_license: { params: { licenseKey: string }; result: { success: boolean; tier: string; expires_at?: string } };
  get_trial_status: { params: Record<string, never>; result: { active: boolean; days_remaining: number; started_at: string | null } };
  start_trial: { params: Record<string, never>; result: { success: boolean; days_remaining?: number } };
  get_pro_value_report: { params: Record<string, never>; result: ProValueReport };

  // -- STREETS Coach --
  get_streets_tier: { params: Record<string, never>; result: { tier: string; expired?: boolean } };
  activate_streets_license: { params: { licenseKey: string }; result: { success: boolean; streets_tier: string; tier: string } };
  coach_list_sessions: { params: Record<string, never>; result: CoachSession[] };
  coach_create_session: { params: { sessionType: CoachSessionType; title?: string }; result: CoachSession };
  coach_delete_session: { params: { sessionId: string }; result: void };
  coach_send_message: { params: { sessionId: string; content: string }; result: CoachMessage };
  coach_get_history: { params: { sessionId: string }; result: CoachMessage[] };
  coach_recommend_engines: { params: Record<string, never>; result: EngineRecommendation };
  coach_generate_strategy: { params: Record<string, never>; result: string };
  coach_launch_review: { params: { projectDescription: string }; result: LaunchReviewResult };
  coach_progress_check_in: { params: Record<string, never>; result: string };
  get_coach_nudges: { params: Record<string, never>; result: CoachNudge[] };
  dismiss_coach_nudge: { params: { nudgeId: number }; result: void };
  get_templates: { params: Record<string, never>; result: CoachTemplate[] };
  get_video_curriculum: { params: Record<string, never>; result: [VideoLesson[], VideoCurriculumStatus] };

  // -- Playbook (STREETS) --
  get_playbook_modules: { params: Record<string, never>; result: PlaybookModule[] };
  get_playbook_content: { params: { moduleId: string }; result: PlaybookContent };
  get_playbook_progress: { params: Record<string, never>; result: PlaybookProgress };
  mark_lesson_complete: { params: { moduleId: string; lessonIdx: number }; result: void };
  parse_lesson_commands: { params: { moduleId: string; lessonIdx: number }; result: ParsedCommand[] };
  execute_streets_command: { params: { commandId: string; command: string; riskLevel: string }; result: CommandExecutionResult };

  // -- Suns (Autonomous Engines) --
  get_sun_statuses: { params: Record<string, never>; result: SunStatus[] };
  get_sun_alerts: { params: Record<string, never>; result: SunAlert[] };
  toggle_sun: { params: { sunId: string; enabled: boolean }; result: void };
  acknowledge_sun_alert: { params: { alertId: number }; result: void };
  trigger_sun_manually: { params: { sunId: string }; result: SunRunResult };
  get_street_health: { params: Record<string, never>; result: StreetHealthScore };

  // -- Sovereign Profile --
  get_sovereign_profile: { params: Record<string, never>; result: SovereignProfileData };
  get_sovereign_profile_completeness: { params: Record<string, never>; result: ProfileCompleteness };
  save_sovereign_fact: { params: { category: string; key: string; value: string }; result: void };
  generate_sovereign_stack_document: { params: Record<string, never>; result: string };

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

  // -- Knowledge Gaps & Predictions --
  get_knowledge_gaps: { params: Record<string, never>; result: KnowledgeGap[] };
  get_predicted_context: { params: Record<string, never>; result: PredictedContext };

  // -- Signal Chains --
  get_signal_chains: { params: Record<string, never>; result: SignalChain[] };
  resolve_signal_chain: { params: { chainId: string; resolution: string }; result: void };

  // -- Score Autopsy --
  mcp_score_autopsy: { params: { itemId: number; sourceType: string; synthesize: boolean; compact: boolean }; result: ScoreAutopsyResult };

  // -- Audio & Context Handoff --
  get_audio_briefing_status: { params: Record<string, never>; result: AudioBriefingStatus };
  generate_audio_briefing: { params: Record<string, never>; result: string };
  generate_context_packet: { params: Record<string, never>; result: ContextPacket };

  // -- Engagement & Attention --
  get_engagement_summary: { params: Record<string, never>; result: EngagementData };
  get_attention_report: { params: { periodDays: number }; result: AttentionReport };

  // -- Developer DNA --
  get_developer_dna: { params: Record<string, never>; result: DeveloperDna };
  export_developer_dna_markdown: { params: Record<string, never>; result: string };
  export_developer_dna_svg: { params: Record<string, never>; result: string };

  // -- Tech Radar --
  get_tech_radar: { params: Record<string, never>; result: TechRadarData };
  get_radar_at_snapshot: { params: { snapshotDate: string }; result: TechRadarData };
  get_radar_entry_detail: { params: { name: string }; result: RadarEntryDetail };
  get_radar_snapshots: { params: Record<string, never>; result: Array<{ date: string }> };

  // -- Project Health --
  get_project_health: { params: Record<string, never>; result: ProjectHealth[] };

  // -- Void Engine --
  get_void_signal: { params: Record<string, never>; result: VoidSignal };

  // -- Source Configuration --
  get_rss_feeds: { params: Record<string, never>; result: { feeds: string[]; count: number } };
  set_rss_feeds: { params: { feeds: string[] }; result: void };
  get_youtube_channels: { params: Record<string, never>; result: { channels: string[]; count: number } };
  set_youtube_channels: { params: { channels: string[] }; result: void };
  get_twitter_handles: { params: Record<string, never>; result: { handles: string[]; count: number } };
  set_twitter_handles: { params: { handles: string[] }; result: void };
  get_x_api_key: { params: Record<string, never>; result: string };
  set_x_api_key: { params: { key: string }; result: void };
  get_github_languages: { params: Record<string, never>; result: { languages: string[]; count: number } };
  set_github_languages: { params: { languages: string[] }; result: void };
  get_sources: { params: Record<string, never>; result: unknown };

  // -- Locale & i18n --
  get_locale: { params: Record<string, never>; result: { country: string; language: string; currency: string } };
  set_locale: { params: { country: string; language: string; currency: string }; result: void };
  format_currency: { params: { amount: number }; result: string };
  get_translation_status: { params: { lang: string }; result: TranslationStatus };
  get_all_translations: { params: { lang: string }; result: Record<string, TranslationEntry> };
  save_translation_override: { params: { lang: string; namespace: string; key: string; value: string }; result: void };
  trigger_translation: { params: { lang: string }; result: void };

  // -- Digest --
  get_digest_config: { params: Record<string, never>; result: DigestConfig };
  set_digest_config: { params: { enabled: boolean }; result: void };

  // -- Command Deck (Git) --
  git_deck_list_repos: { params: Record<string, never>; result: RepoInfo[] };
  git_deck_status: { params: { repoPath: string }; result: GitDeckStatus };
  git_deck_stage: { params: { repoPath: string; paths: string[] }; result: void };
  git_deck_unstage: { params: { repoPath: string; paths: string[] }; result: void };
  git_deck_commit: { params: { repoPath: string; message: string }; result: { short_hash: string; files_changed: number } };
  git_deck_push: { params: { repoPath: string; branch: string | null }; result: void };
  git_deck_suggest_commit: { params: { repoPath: string }; result: SuggestedCommitMessage };
  git_deck_log: { params: { repoPath: string; count: number }; result: CommitSummary[] };
  git_deck_diff_stat: { params: { repoPath: string; staged: boolean }; result: string };
  run_shell_command: { params: { command: string; workingDir: string | null }; result: CommandOutput };
  get_command_history: { params: { limit: number }; result: CommandHistoryEntry[] };

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
  auto_render_all_channels: { params: Record<string, never>; result: void };
  create_custom_channel: { params: { slug: string; title: string; description: string; topicQuery: string[] }; result: number };
  delete_channel: { params: { channelId: number }; result: void };

  // -- Splash Probes --
  get_context_stats: { params: Record<string, never>; result: unknown };
}


// ============================================================================
// Types referenced above but not yet in shared type files
// (These can be moved to src/types/ as they get formalized)
// ============================================================================

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

interface SunStatus {
  id: string;
  name: string;
  enabled: boolean;
  last_run: string | null;
  next_run: string | null;
  status: string;
}

interface SunAlert {
  id: number;
  sun_id: string;
  alert_type: string;
  message: string;
  severity: string;
  acknowledged: boolean;
  created_at: string;
}

interface SunRunResult {
  success: boolean;
  items_processed: number;
  duration_ms: number;
  message: string;
}

interface StreetHealthScore {
  overall: number;
  dimensions: Record<string, number>;
  recommendations: string[];
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

interface FeedTestResult {
  valid: boolean;
  title: string | null;
  item_count: number;
  error: string | null;
  sample_titles: string[];
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
  return invoke<CommandMap[K]['result']>(command, params ?? {});
}


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
  DelegationScoreEntry,
  ProValueReport,
  SunStatus,
  SunAlert,
  SunRunResult,
  StreetHealthScore,
  SovereignProfileData,
  ProfileCompleteness,
  NLQResult,
  ScoreAutopsyResult,
  EngagementData,
  TechRadarData,
  RadarEntryDetail,
  TranslationStatus,
  TranslationEntry,
  DigestConfig,
  ListeningPort,
  HttpProbeRequest,
  HttpProbeResponse,
  HttpHistoryEntry,
  FeedTestResult,
  SandboxScoreResult,
  ExportPackResult,
  EnvSnapshot,
};
