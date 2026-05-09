// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// ============================================================================
// Sovereign Developer Profile — TypeScript types mirroring Rust structs
// ============================================================================

export interface SovereignDeveloperProfile {
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

// ---- Dimension 1: Infrastructure ----

export interface InfrastructureDimension {
  cpu: Record<string, string>;
  ram: Record<string, string>;
  gpu: Record<string, string>;
  storage: Record<string, string>;
  network: Record<string, string>;
  os: Record<string, string>;
  llm: Record<string, string>;
  legal: Record<string, string>;
  budget: Record<string, string>;
  gpu_tier: string;
  llm_tier: string;
}

// ---- Dimension 2: Stack ----

export interface StackDimension {
  primary_stack: string[];
  adjacent_tech: string[];
  detected_tech: DetectedTechEntry[];
  dependencies: string[];
  selected_profiles: string[];
}

interface DetectedTechEntry {
  name: string;
  confidence: number;
}

// ---- Dimension 3: Skills ----

export interface SkillsDimension {
  top_affinities: AffinityEntry[];
  playbook_progress: PlaybookProgressSummary;
  engagement_sources: SourceEngagementEntry[];
}

interface AffinityEntry {
  topic: string;
  score: number;
}

interface PlaybookProgressSummary {
  completed_lessons: number;
  total_lessons: number;
  completed_modules: string[];
}

interface SourceEngagementEntry {
  source_type: string;
  items_seen: number;
  items_saved: number;
}

// ---- Dimension 4: Preferences ----

export interface PreferencesDimension {
  interests: string[];
  exclusions: string[];
  active_decisions: ProfileDecisionEntry[];
  tech_radar: TechRadarSummary;
}

interface ProfileDecisionEntry {
  subject: string;
  decision: string;
}

interface TechRadarSummary {
  adopt: string[];
  trial: string[];
  assess: string[];
  hold: string[];
}

// ---- Dimension 5: Context ----

export interface ContextDimension {
  active_topics: string[];
  scan_directories: string[];
  projects_monitored: number;
}

// ---- Intelligence ----

export interface IntelligenceReport {
  skill_gaps: SkillGap[];
  optimization_opportunities: OptimizationOpportunity[];
  infrastructure_mismatches: InfrastructureMismatch[];
  ecosystem_alerts: EcosystemAlert[];
}

interface SkillGap {
  dependency: string;
  reason: string;
}

interface OptimizationOpportunity {
  area: string;
  suggestion: string;
  severity: number;
}

interface InfrastructureMismatch {
  category: string;
  issue: string;
}

interface EcosystemAlert {
  from_tech: string;
  to_tech: string;
  description: string;
}

// ---- Completeness ----

export interface CompletenessReport {
  overall_percentage: number;
  dimensions: DimensionCompleteness[];
}

interface DimensionCompleteness {
  name: string;
  depth: string;
  fact_count: number;
  percentage: number;
}
